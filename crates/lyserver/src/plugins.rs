use core::panic;
use std::{fs, path::Path, sync::{atomic::{AtomicBool, Ordering}, Arc}, time::Duration};
use lyserver_plugin_shared_data::LYServerPluginSharedData;
use lyserver_shared_data::{LYServerPluginInstance, LYServerSharedData, LYServerSharedDataDirectories, LYServerSharedDataMessaging};
use tokio::{sync::{Mutex, RwLock}, task::JoinHandle};
use futures::future::try_join_all;

use lyserver_plugin_common::{LYServerPlugin, LYServerPluginMetadata};
use lyserver_plugin_wasm_loader::LYServerWASMLoader;
use tokio_util::sync::CancellationToken;

pub struct LYServerPluginManager {
    wasm_loader: Arc<LYServerWASMLoader>,

    plugin_tasks: Vec<JoinHandle<anyhow::Result<()>>>,

    shared_data: Arc<LYServerSharedData>,

    messaging_loop_running: Arc<AtomicBool>,
}

impl LYServerPluginManager {
    pub fn new(shared_data: Arc<LYServerSharedData>) -> Self {
        Self {
            wasm_loader: LYServerWASMLoader::new(shared_data.clone()).into(),

            plugin_tasks: Vec::new(),

            shared_data,

            messaging_loop_running: AtomicBool::new(false).into(),
        }
    }

    pub async fn init_messaging_loop(&mut self) -> anyhow::Result<()> {
        log::info!("PluginManager: Initializing global messaging loop...");

        let shared_data_clone = Arc::clone(&self.shared_data);
        let messaging_loop_running = Arc::clone(&self.messaging_loop_running);

        let handle = tokio::spawn(async move {
            tokio::select! {
                _ = async {
                    while !shared_data_clone.shutdown_flag.load(Ordering::Relaxed) {
                        tokio::time::sleep(Duration::from_millis(200)).await;
                    }
                } => {}
                
                _ = async {
                    messaging_loop_running.store(true, Ordering::Relaxed);

                    log::info!("Starting plugin global messaging loop...");

                    while let Some(event) = shared_data_clone.receive_event().await {
                        log::debug!("!!!!!!!!!!!!!!!!!!!! Received event: {}", event.event_type);

                        let shared_data_clone = Arc::clone(&shared_data_clone);

                        tokio::spawn(async move {
                            log::debug!("got event: {}", event.event_type);
                            let plugin_receivers = shared_data_clone.messaging_plugin_tx.read().await
                                .iter()
                                .filter_map(|(plugin_id, tx)| {
                                    if event.event_target.is_all() || event.event_target.plugin_id() == Some(plugin_id.to_string()) {
                                        Some(tx.clone())
                                    } else {
                                        None
                                    }
                                })
                                .collect::<Vec<_>>();
                            log::debug!("got plugin receivers: {}", plugin_receivers.len());
    
                            let plugin_receiver_count = plugin_receivers.len();
    
                            log::debug!("[Plugin Messaging] Broadcasting event to {} plugins '{}' ({}): {} -> {}", plugin_receiver_count, event.event_type, event.event_id, event.event_sender.to_string(), event.event_target.to_string());
    
                            let event_clone = event.clone();
                            for tx in plugin_receivers.iter() {
                                log::debug!("iter plugin tx");
    
                                let tx_clone = Arc::clone(tx);
                                let event_clone = event_clone.clone();
    
                                tokio::spawn(async move {
                                    log::debug!("pub plugin event");
    
                                    let tx = tx_clone.write().await;
                
                                    let _ = tx.send(event_clone.clone());
                                });
                            }
                        });
                    }

                    return Ok::<(), anyhow::Error>(());
                } => {}
            }

            messaging_loop_running.store(false, Ordering::Relaxed);
            
            log::error!("Plugin global messaging loop has exited unexpectedly.");

            Ok(())
        });

        self.plugin_tasks.push(handle);

        Ok(())
    }

    pub async fn init(&mut self) {
        log::info!("PluginManager: Initializing server plugins...");

        let cwd = std::env::current_dir()
            .expect("Failed to get current directory");

        let plugins_path = self.shared_data.resolve_data_path(Path::new("plugins"));

        if plugins_path.exists() {
            let available_plugins = fs::read_dir(&plugins_path)
                .expect("Failed to read plugins directory")
                .filter_map(|entry| entry.ok())
                .map(|entry| entry.path())
                .collect::<Vec<_>>();

            log::debug!("Found {} plugin(s) in '{}'", available_plugins.len(), plugins_path.display());

            for plugin in available_plugins {
                log::info!("Loading plugin from path: '{}'", plugin.display());

                if let Err(e) = self.load_plugin_from_path(&plugin).await {
                    log::error!("Failed to load plugin '{}': {}", plugin.display(), e);
                }
            }
        } else {
            log::debug!("No plugins directory found at '{}'.", plugins_path.display());

            fs::create_dir(plugins_path.clone())
                .expect("Failed to create plugins directory");
        }

        log::info!("PluginManager: Loaded {} plugin(s)!", self.shared_data.loaded_plugins.read().await.len());

        for (_, metadata, _) in self.shared_data.loaded_plugins.read().await.iter() {
            log::info!("    {} (v{}) - {}", 
                metadata.name,  
                metadata.version,
                metadata.id);
        }
    }

    pub async fn load_plugin_from_path(&mut self, plugin_path: &Path) -> anyhow::Result<LYServerPluginInstance, String> {
        let plugin_path_display = plugin_path.display();

        if !plugin_path.exists() {
            return Err(format!("Plugin at '{}' does not exist", plugin_path_display));
        }

        if !plugin_path.is_dir() {
            return Err(format!("Plugin path '{}' is not a directory", plugin_path_display));
        }

        let plugin_manifest_file_path = fs::read_dir(plugin_path)
            .map_err(|e| format!("Failed to read plugin dir '{}': {}", plugin_path_display, e))?
            .find_map(|entry| {
                entry.ok().and_then(|e| {
                    if e.file_name().to_ascii_lowercase() == "manifest.toml" {
                        Some(e.path())
                    } else {
                        None
                    }
                })
            })
            .ok_or_else(|| format!("manifest.toml not found in '{}'", plugin_path_display))?;

        let plugin_manifest_content = fs::read_to_string(&plugin_manifest_file_path)
            .map_err(|e| format!("Failed to read manifest '{}': {}", plugin_manifest_file_path.display(), e))?;

        let plugin_metadata: LYServerPluginMetadata = toml::from_str(&plugin_manifest_content)
            .map_err(|e| format!("Failed to parse manifest '{}': {}", plugin_manifest_file_path.display(), e))?;

        let wasm_path = plugin_metadata.wasm_entry_point
            .as_deref()
            .ok_or_else(|| format!("Plugin '{}' missing wasm_entry_point in manifest", plugin_metadata.id))?;

        let wasm_full_path = plugin_path.join(wasm_path);

        if !wasm_full_path.exists() {
            return Err(format!("Plugin '{}' wasm entry '{}' not found", plugin_metadata.id, wasm_full_path.display()));
        }

        let wasm_loader_consumer = Arc::clone(&self.wasm_loader);

        self
            .load_plugin(plugin_metadata.id.clone(), |plugin_shared_data| async move {
                let wasm_plugin = wasm_loader_consumer
                    .create_wasm_plugin_instance(&plugin_metadata, &wasm_full_path, plugin_shared_data)
                    .await
                    .map_err(|e| {
                        format!(
                            "Failed to create WASM plugin '{}': {}",
                            plugin_metadata.id, e
                        )
                    })
                    .unwrap();
        
                let thread_safe_plugin: LYServerPluginInstance = Arc::new(wasm_plugin);
                Box::new(thread_safe_plugin)
            })
            .await
    }

    pub async fn load_plugin<F, Fut, T: Into<String>>(
        &mut self,
        plugin_id: T,
        constructor: F,
    ) -> anyhow::Result<LYServerPluginInstance, String>
    where
        F: FnOnce(Arc<LYServerPluginSharedData>) -> Fut,
        Fut: Future<Output = Box<LYServerPluginInstance>>
    {
        let shared_data = self.shared_data.clone();
        let mut plugin_shared_data = LYServerPluginSharedData::new(shared_data.clone());

        let plugin_id = plugin_id.into();

        plugin_shared_data.register_plugin_messaging(plugin_id.clone())
            .await
            .map_err(|e| format!("Failed to register plugin messaging for '{}': {}", plugin_id, e))?;

        let plugin_shared_data = Arc::new(plugin_shared_data);
        let plugin = constructor(plugin_shared_data.clone()).await;
        let plugin_metadata = plugin.metadata().clone();

        let plugin_token = CancellationToken::new();
        let plugin_child_token = plugin_token.child_token();

        self.shared_data.loaded_plugins.write().await.push((*plugin.clone(), plugin_metadata, plugin_token));

        let plugin_clone = Arc::clone(&plugin);
        let loaded_plugins_clone = Arc::clone(&self.shared_data.loaded_plugins);

        let handle = tokio::spawn(async move {
            let unload_plugin = async || {
                let mut loaded_plugins = loaded_plugins_clone.write().await;
                if let Some(pos) = loaded_plugins.iter().position(|(p, _, _)| Arc::ptr_eq(p, &plugin_clone)) {
                    loaded_plugins.remove(pos);
                }
            };

            let plugin = plugin_clone.clone();

            tokio::select! {
                res = plugin.init() => {
                    if let Err(e) = res {
                        log::error!("PluginManager: Failed to init plugin '{}': {}", plugin.metadata().id, e);
                        unload_plugin().await;
        
                        panic!("PluginManager: Failed to init plugin '{}': {}", plugin.metadata().id, e);
                    }
                }
                _ = plugin_child_token.cancelled() => {
                    log::info!("Plugin got shutdown signal, destroying plugin '{}'...", plugin.metadata().id);

                    if let Err(e) = plugin.destroy().await {
                        log::error!("PluginManager: Failed to destroy plugin '{}': {}", plugin.metadata().id, e);
                    } else {
                        log::warn!("PluginManager: Plugin '{}' runtime has completed, closing...", plugin.metadata().id);
                    }

                    unload_plugin().await;
                }
            }

            Ok(())
        });

        self.plugin_tasks.push(handle);

        let timeout = Duration::from_secs(10);
        log::info!("Waiting for plugin '{}' to initialize... (timeout in {}s)", plugin.metadata().id, timeout.as_secs());

        if !self.messaging_loop_running.load(Ordering::Relaxed) {
            panic!("PluginManager: Plugin global messaging loop is not running, cannot wait for plugin initialization.");   
        }

        if let None = plugin_shared_data.wait_until_event(|e| e.event_type == "plugin_init", Duration::from_secs(10)).await {
            log::error!("PluginManager: Plugin '{}' did not initialize within {}s, unloading plugin...", plugin.metadata().id, timeout.as_secs());

            return Err(format!("Plugin '{}' did not initialize within {}s", plugin.metadata().id, timeout.as_secs()));
        }

        Ok(*plugin.clone())
    }

    pub async fn wait_for_all_plugins(&mut self) -> anyhow::Result<()> {
        let plugin_results = try_join_all(self.plugin_tasks.drain(..)).await;

        match plugin_results {
            Ok(_) => Ok(()),
            Err(e) => Err(anyhow::anyhow!("Plugin thread panicked! {:#?}", e)),
        }
    }

    pub async fn destroy(&mut self) -> anyhow::Result<()> {
        log::info!("PluginManager: Destroying all plugins...");

        let loaded_plugins = &self.shared_data.loaded_plugins.write().await;
        for (_, metadata, token) in loaded_plugins.iter() {
            log::info!("Unloading plugin '{}'...", metadata.id);
            token.cancel();
        }

        Ok(())
    }
}
