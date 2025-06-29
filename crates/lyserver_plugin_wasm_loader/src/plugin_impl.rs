use std::sync::{Arc};

use lyserver_messaging_shared::LYServerMessageEvent;
use lyserver_plugin_common::{LYServerPlugin, LYServerPluginMetadata};
use lyserver_plugin_shared_data::LYServerPluginSharedData;
use lyserver_shared_data::LYServerSharedData;
use serde_json::Value;
use tokio::sync::Mutex;
use wasmtime::WasmParams;
use wasmtime_wasi::{p2::WasiCtxBuilder, preview1::WasiP1Ctx};

use crate::{LYServerWASMLinkerState, LYSERVER_PLUGIN_ABI_ALLOC_METHOD, LYSERVER_PLUGIN_ABI_DESTROY_METHOD, LYSERVER_PLUGIN_ABI_HANDLE_MESSAGE_EVENT_METHOD, LYSERVER_PLUGIN_ABI_INIT_METHOD};

pub struct LYServerWASMPlugin {
    metadata: LYServerPluginMetadata,

    instance: wasmtime::Instance,
    store: Arc<Mutex<wasmtime::Store<LYServerWASMLinkerState>>>,

    plugin_shared_data: Arc<LYServerPluginSharedData>,

    init: Arc<wasmtime::TypedFunc<(), ()>>,
    destroy: Arc<wasmtime::TypedFunc<(), ()>>,
    handle_message_event: Arc<wasmtime::TypedFunc<(i32, i32), ()>>,
}

impl LYServerWASMPlugin {
    pub async fn new(
        metadata: LYServerPluginMetadata,
        engine: &wasmtime::Engine,
        linker: &mut wasmtime::Linker<LYServerWASMLinkerState>,
        module: &wasmtime::Module,
        plugin_shared_data: Arc<LYServerPluginSharedData>,
    ) -> anyhow::Result<Self> {
        let wasi_ctx = WasiCtxBuilder::new()
            .inherit_stdio()
            .build_p1();

        let mut store = wasmtime::Store::new(engine, LYServerWASMLinkerState {
            wasi_ctx: wasi_ctx
        });

        let instance = linker.instantiate_async(&mut store, &module)
            .await
            .map_err(|e| anyhow::Error::msg(format!("Failed to instantiate plugin '{}': {}", metadata.id, e)))?;

        let init_fn = instance
            .get_typed_func::<(), ()>(&mut store, LYSERVER_PLUGIN_ABI_INIT_METHOD)
            .map_err(|e| anyhow::Error::msg(format!("No method named '{}' in plugin '{}': {}", LYSERVER_PLUGIN_ABI_INIT_METHOD, metadata.id, e)))?;

        let destroy_fn = instance
            .get_typed_func::<(), ()>(&mut store, LYSERVER_PLUGIN_ABI_DESTROY_METHOD)
            .map_err(|e| anyhow::Error::msg(format!("No method named '{}' in plugin '{}': {}", LYSERVER_PLUGIN_ABI_DESTROY_METHOD, metadata.id, e)))?;

        let handle_message_event_fn = instance
            .get_typed_func::<(i32, i32), ()>(&mut store, LYSERVER_PLUGIN_ABI_HANDLE_MESSAGE_EVENT_METHOD)
            .map_err(|e| anyhow::Error::msg(format!("No method named '{}' in plugin '{}': {}", LYSERVER_PLUGIN_ABI_HANDLE_MESSAGE_EVENT_METHOD, metadata.id, e)))?;

        Ok(Self {
            metadata,

            instance,
            store: Arc::new(Mutex::new(store)),

            plugin_shared_data,

            init: init_fn.into(),
            destroy: destroy_fn.into(),
            handle_message_event: handle_message_event_fn.into(),
        })
    }
}

#[async_trait::async_trait]
impl LYServerPlugin for LYServerWASMPlugin {
    fn metadata(&self) -> LYServerPluginMetadata {
        self.metadata.clone()
    }

    async fn init(&self) -> anyhow::Result<()> {
        let init_fn = Arc::clone(&self.init);
        let metadata = self.metadata.clone();
        let store = Arc::clone(&self.store);

        let result = tokio::task::spawn(async move {
            let mut store = store.lock().await;

            init_fn.call_async(&mut *store, ())
                .await
                .map_err(|e| anyhow::Error::msg(format!("Failed to call init for '{}': {}", metadata.id, e)))
        }).await;

        match result {
            Ok(res) => res,
            Err(e) => Err(anyhow::Error::msg(format!("Failed to spawn init task for '{}': {}", self.metadata.id, e))),
        }
    }

    async fn destroy(&self) -> anyhow::Result<()> {
        let destroy_fn = Arc::clone(&self.destroy);
        let metadata = self.metadata.clone();
        let store = Arc::clone(&self.store);

        let result = tokio::task::spawn(async move {
            let mut store = store.lock().await;

            destroy_fn.call_async(&mut *store, ())
                .await
                .map_err(|e| anyhow::Error::msg(format!("Failed to call destroy for '{}': {}", metadata.id, e)))
        }).await;

        match result {
            Ok(res) => res,
            Err(e) => Err(anyhow::Error::msg(format!("Failed to spawn destroy task for '{}': {}", self.metadata.id, e))),
        }
    }

    async fn handle_message_event(&self, event: LYServerMessageEvent) -> anyhow::Result<()> {
        let handle_message_event_fn = Arc::clone(&self.handle_message_event);
        let metadata = self.metadata.clone();
        let store = Arc::clone(&self.store);
    
        // serialize event to CBOR
        let serialized = serde_cbor::to_vec(&event)
            .map_err(|e| anyhow::anyhow!("Failed to serialize event for '{}': {}", metadata.id, e))?;
    
        let message_len = serialized.len() as i32;

        let wasm_message_ptr = {
            let mut store = store.lock().await;

            let alloc_func = self.instance
                .get_export(&mut *store, LYSERVER_PLUGIN_ABI_ALLOC_METHOD)
                .and_then(|e| e.into_func())
                .ok_or_else(|| anyhow::anyhow!(
                    "Plugin {} does not export '{}'",
                    metadata.id,
                    LYSERVER_PLUGIN_ABI_ALLOC_METHOD,
                ))?;

            // --- get memory ---
            let memory = self.instance
                .get_export(&mut *store, "memory")
                .and_then(|e| e.into_memory())
                .ok_or_else(|| anyhow::anyhow!("Plugin {} does not export memory", metadata.id))?;

            // --- call alloc inside typed closure ---
            let wasm_ptr = {
                let mut wasm_ptr: Option<i32> = None;
                alloc_func
                    .typed::<i32, i32>(&*store)?
                    .call_async(&mut *store, message_len)
                    .await
                    .map(|ptr| wasm_ptr = Some(ptr))
                    .map_err(|e| anyhow::anyhow!(
                        "alloc call failed in '{}': {}",
                        metadata.id, e
                    ))?;
                wasm_ptr.ok_or_else(|| anyhow::anyhow!("alloc didn't return a pointer"))?
            };

            // write to memory
            memory.write(&mut *store, wasm_ptr as usize, &serialized)
                .map_err(|e| anyhow::anyhow!(
                    "Failed to write message to wasm memory for '{}': {}",
                    metadata.id, e
                ))?;

            wasm_ptr
        };
    
        let result = tokio::task::spawn(async move {
            let mut store = store.lock().await;
    
            handle_message_event_fn
                .call_async(&mut *store, (wasm_message_ptr, message_len))
                .await
                .map_err(|e| anyhow::anyhow!(
                    "Failed to call handle_message_event for '{}': {}",
                    metadata.id, e
                ))
        }).await;
    
        match result {
            Ok(res) => res,
            Err(e) => Err(anyhow::anyhow!(
                "Failed to spawn handle_message_event task for '{}': {}",
                self.metadata.id, e
            )),
        }
    }

    async fn invoke(&self, method: &str, args: Vec<String>) -> anyhow::Result<Value> {
        Err(anyhow::anyhow!("Method 'invoke' is not implemented for plugin '{}'", self.metadata.id))
    }

    async fn receive(&self, method: &str, args: Vec<String>) -> anyhow::Result<Value> {
        // This method is not implemented in the original code, so we can leave it empty or handle it as needed.
        Err(anyhow::anyhow!("Method 'receive' is not implemented for plugin '{}'", self.metadata.id))
    }
}
