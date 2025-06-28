use std::sync::{Arc};

use lyserver_plugin_common::{LYServerPlugin, LYServerPluginMetadata};
use lyserver_plugin_shared_data::LYServerPluginSharedData;
use lyserver_shared_data::LYServerSharedData;
use serde_json::Value;
use tokio::sync::Mutex;
use wasmtime_wasi::{p2::WasiCtxBuilder, preview1::WasiP1Ctx};

use crate::{LYServerWASMLinkerState, LYSERVER_PLUGIN_ABI_DESTROY_METHOD, LYSERVER_PLUGIN_ABI_INIT_METHOD};

pub struct LYServerWASMPlugin {
    metadata: LYServerPluginMetadata,
    instance: wasmtime::Instance,
    store: Arc<Mutex<wasmtime::Store<LYServerWASMLinkerState>>>,
    init: Arc<wasmtime::TypedFunc<(), ()>>,
    destroy: Arc<wasmtime::TypedFunc<(), ()>>,
    plugin_shared_data: Arc<LYServerPluginSharedData>,
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

        Ok(Self {
            metadata,
            instance,
            init: init_fn.into(),
            destroy: destroy_fn.into(),
            store: Arc::new(Mutex::new(store)),
            plugin_shared_data,
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

    async fn invoke(&self, method: &str, args: Vec<String>) -> anyhow::Result<Value> {
        Err(anyhow::anyhow!("Method 'invoke' is not implemented for plugin '{}'", self.metadata.id))
    }

    async fn receive(&self, method: &str, args: Vec<String>) -> anyhow::Result<Value> {
        // This method is not implemented in the original code, so we can leave it empty or handle it as needed.
        Err(anyhow::anyhow!("Method 'receive' is not implemented for plugin '{}'", self.metadata.id))
    }
}
