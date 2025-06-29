pub mod plugin_impl;

use std::{path::Path, sync::{Arc, Mutex}};

use lyserver_plugin_common::LYServerPluginMetadata;
use lyserver_plugin_shared_data::LYServerPluginSharedData;
use lyserver_shared_data::LYServerSharedData;
use wasmtime::{Caller, Module};
use wasmtime_wasi::preview1::{WasiP1Ctx};

use crate::plugin_impl::LYServerWASMPlugin;

pub const LYSERVER_PLUGIN_ABI_INIT_METHOD: &str = "lyserver_plugin_init";
pub const LYSERVER_PLUGIN_ABI_DESTROY_METHOD: &str = "lyserver_plugin_destroy";
pub const LYSERVER_PLUGIN_ABI_HANDLE_MESSAGE_EVENT_METHOD: &str = "lyserver_plugin_handle_message_event";
pub const LYSERVER_PLUGIN_ABI_STDOUT_WRITE_METHOD: &str = "lyserver_plugin_stdout_write";
pub const LYSERVER_PLUGIN_ABI_LOG_INFO_METHOD: &str = "lyserver_plugin_log_info";
pub const LYSERVER_PLUGIN_ABI_LOG_WARN_METHOD: &str = "lyserver_plugin_log_warn";
pub const LYSERVER_PLUGIN_ABI_LOG_ERROR_METHOD: &str = "lyserver_plugin_log_error";
pub const LYSERVER_PLUGIN_ABI_LOG_DEBUG_METHOD: &str = "lyserver_plugin_log_debug";
pub const LYSERVER_PLUGIN_ABI_RECEIVE_MESSAGE_METHOD: &str = "lyserver_plugin_receive_message";
pub const LYSERVER_PLUGIN_ABI_SEND_MESSAGE_METHOD: &str = "lyserver_plugin_send_message";
pub const LYSERVER_PLUGIN_ABI_ALLOC_METHOD: &str = "lyserver_plugin_alloc";

macro_rules! add_linker_func {
    ($linker:expr, $name:expr, $handler:expr) => {
        $linker
            .func_wrap_async("env", $name, $handler)
            .expect(&format!("Failed to register plugin function: {}", $name));
    };
}

pub fn mutate_linker<'a>(
    linker: &'a mut wasmtime::Linker<LYServerWASMLinkerState>,
    metadata: &'a LYServerPluginMetadata,
    plugin_shared_data: Arc<LYServerPluginSharedData>,
) -> &'a mut wasmtime::Linker<LYServerWASMLinkerState> {
    let plugin_id = Arc::new(metadata.id.clone());

    add_linker_func!(
        linker,
        LYSERVER_PLUGIN_ABI_STDOUT_WRITE_METHOD,
        |mut caller: Caller<'_, LYServerWASMLinkerState>, (ptr, len): (i32, i32)| Box::new(async move {
            let mem = caller.get_export("memory").unwrap().into_memory().unwrap();
            let start = ptr as usize;
            let end = start + len as usize;
            let data = mem.data(&caller)[start..end].to_vec();
            let msg = String::from_utf8_lossy(&data);
            println!("{}", msg);
            Ok(())
        })
    );

    let plugin_id_clone = plugin_id.clone();
    add_linker_func!(
        linker,
        LYSERVER_PLUGIN_ABI_LOG_INFO_METHOD,
        move |mut caller: Caller<'_, LYServerWASMLinkerState>, (ptr, len): (i32, i32)| Box::new({
            let plugin_id_clone = plugin_id_clone.clone();

            async move {
                let mem = caller.get_export("memory").unwrap().into_memory().unwrap();
                let start = ptr as usize;
                let end = start + len as usize;
                let data = mem.data(&caller)[start..end].to_vec();
                let msg = String::from_utf8_lossy(&data);
                log::info!("[Plugin {}]: {}", &plugin_id_clone, msg);
                Ok(())
            }
        })
    );

    let plugin_id_clone = plugin_id.clone();
    add_linker_func!(
        linker,
        LYSERVER_PLUGIN_ABI_LOG_WARN_METHOD,
        move |mut caller: Caller<'_, LYServerWASMLinkerState>, (ptr, len): (i32, i32)| Box::new({
            let plugin_id_clone = plugin_id_clone.clone();

            async move {
                let mem = caller.get_export("memory").unwrap().into_memory().unwrap();
                let start = ptr as usize;
                let end = start + len as usize;
                let data = mem.data(&caller)[start..end].to_vec();
                let msg = String::from_utf8_lossy(&data);
                log::warn!("[Plugin {}]: {}", &plugin_id_clone, msg);
                Ok(())
            }
        })
    );

    let plugin_id_clone = plugin_id.clone();
    add_linker_func!(
        linker,
        LYSERVER_PLUGIN_ABI_LOG_ERROR_METHOD,
        move |mut caller: Caller<'_, LYServerWASMLinkerState>, (ptr, len): (i32, i32)| Box::new({
            let plugin_id_clone = plugin_id_clone.clone();

            async move {
                let mem = caller.get_export("memory").unwrap().into_memory().unwrap();
                let start = ptr as usize;
                let end = start + len as usize;
                let data = mem.data(&caller)[start..end].to_vec();
                let msg = String::from_utf8_lossy(&data);
                log::error!("[Plugin {}]: {}", &plugin_id_clone, msg);
                Ok(())
            }
        })
    );

    let plugin_id_clone = plugin_id.clone();
    add_linker_func!(
        linker,
        LYSERVER_PLUGIN_ABI_LOG_DEBUG_METHOD,
        move |mut caller: Caller<'_, LYServerWASMLinkerState>, (ptr, len): (i32, i32)| Box::new({
            let plugin_id_clone = plugin_id_clone.clone();

            async move {
                let mem = caller.get_export("memory").unwrap().into_memory().unwrap();
                let start = ptr as usize;
                let end = start + len as usize;
                let data = mem.data(&caller)[start..end].to_vec();
                let msg = String::from_utf8_lossy(&data);
                log::debug!("[Plugin {}]: {}", &plugin_id_clone, msg);
                Ok(())
            }
        })
    );

    let plugin_shared_data_clone = plugin_shared_data.clone();
    add_linker_func!(
        linker,
        LYSERVER_PLUGIN_ABI_RECEIVE_MESSAGE_METHOD,
        move |mut caller: Caller<'_, LYServerWASMLinkerState>, (ret_ptr_ptr, ret_len_ptr): (i32, i32)| Box::new({
            let plugin_id_clone = plugin_id.clone();
            let plugin_shared_data_clone = plugin_shared_data_clone.clone();

            async move {
                let memory = caller
                    .get_export("memory")
                    .and_then(|e| e.into_memory())
                    .expect("memory export not found");

                let message = plugin_shared_data_clone.receive_event().await;

                if let Some(response) = message {
                    let serialized = serde_cbor::to_vec(&response)
                        .expect("Failed to serialize message");

                    let response_len = serialized.len();

                    let alloc = caller
                        .get_export(LYSERVER_PLUGIN_ABI_ALLOC_METHOD)
                        .and_then(|e| e.into_func())
                        .expect(format!(
                            "Plugin {} does not export '{}'",
                            &plugin_id_clone,
                            LYSERVER_PLUGIN_ABI_ALLOC_METHOD
                        ).as_str());

                    let ptr = alloc
                        .typed::<i32, i32>(&caller)?
                        .call_async(&mut caller, response_len as i32)
                        .await?;

                    memory.write(&mut caller, ptr as usize, &serialized)?;
                    memory.write(&mut caller, ret_ptr_ptr as usize, &(ptr as i32).to_le_bytes())?;
                    memory.write(&mut caller, ret_len_ptr.try_into().unwrap(), &(response_len as i32).to_le_bytes())?;
                } else {
                    memory.write(&mut caller, ret_ptr_ptr as usize, &(0i32).to_le_bytes())?;
                    memory.write(&mut caller, ret_len_ptr.try_into().unwrap(), &(0i32).to_le_bytes())?;
                }

                Ok(())
            }
        })
    );

    let plugin_shared_data_clone = plugin_shared_data.clone();
    add_linker_func!(
        linker,
        LYSERVER_PLUGIN_ABI_SEND_MESSAGE_METHOD,
        move |mut caller: Caller<'_, LYServerWASMLinkerState>, (ptr, len, ret_ptr): (i32, i32, i32)| Box::new({
            let plugin_shared_data_clone = plugin_shared_data_clone.clone();
    
            async move {
                let memory = caller
                    .get_export("memory")
                    .and_then(|e| e.into_memory())
                    .expect("memory export not found");
    
                let start = ptr as usize;
                let end = start + len as usize;
                let data = memory.data(&caller)[start..end].to_vec();

                let result_code: u32 = tokio::task::spawn_blocking(move || {
                    let result_code: u32 = match plugin_shared_data_clone.dispatch_raw_event(data) {
                        Ok(_) => 0,
                        Err(_) => 1,
                    };

                    return result_code;
                }).await?;

                let ret_ptr = ret_ptr as usize;
                let ret_bytes = result_code.to_le_bytes();
                memory.data_mut(&mut caller)[ret_ptr..ret_ptr + 4]
                    .copy_from_slice(&ret_bytes);
    
                Ok(())
            }
        })
    );
    
    linker
}

pub struct LYServerWASMLinkerState {
    wasi_ctx: WasiP1Ctx,
}


pub struct LYServerWASMLoader {
    engine: wasmtime::Engine,
    shared_data: Arc<LYServerSharedData>,
}

impl LYServerWASMLoader {
    pub fn new(shared_data: Arc<LYServerSharedData>) -> Self {
        let mut config = wasmtime::Config::new();
        config.async_support(true);
        let engine = wasmtime::Engine::new(&config)
            .expect("Failed to create Wasmtime engine");

        Self { engine, shared_data }
    }

    pub async fn create_wasm_plugin_instance(&self, metadata: &LYServerPluginMetadata, wasm_entry_point_path: &Path, plugin_shared_data: Arc<LYServerPluginSharedData>) -> anyhow::Result<plugin_impl::LYServerWASMPlugin> {
        let mut linker: wasmtime::Linker<LYServerWASMLinkerState> = wasmtime::Linker::new(&self.engine);
        let linker = mutate_linker(&mut linker, &metadata, plugin_shared_data.clone());
        wasmtime_wasi::preview1::add_to_linker_async(linker, |c| &mut c.wasi_ctx)?;

        let module = Module::from_file(&self.engine, wasm_entry_point_path)
            .map_err(|e| anyhow::Error::msg(format!("Failed to load module from file '{}': {}", wasm_entry_point_path.display(), e)))?;

        let wasm_plugin = LYServerWASMPlugin::new(metadata.clone(), &self.engine, linker, &module, plugin_shared_data)
            .await
            .map_err(|e| anyhow::Error::msg(format!("Failed to create WASM plugin instance from '{}': {}", wasm_entry_point_path.display(), e)))?;

        Ok(wasm_plugin)
    }
}
