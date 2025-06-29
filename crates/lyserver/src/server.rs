use std::{net::SocketAddr, sync::{atomic::Ordering, Arc}};

use actix_web::{middleware::Logger, web, App, HttpServer};

use lyserver_database::LYServerDatabasePlugin;
use lyserver_http::LYServerHTTPServerPlugin;
use lyserver_plugin_common::LYServerPlugin;
use lyserver_preferences::LYServerPreferencesPlugin;
use lyserver_shared_data::LYServerSharedData;
use tokio::sync::Mutex;

use crate::plugins::LYServerPluginManager;

pub struct LYServer {
    shared_data: Arc<LYServerSharedData>,

    plugin_manager: Arc<Mutex<LYServerPluginManager>>,
}

impl LYServer {
    pub fn new(shared_data: LYServerSharedData) -> Self {
        let shared_data = Arc::new(shared_data);

        let plugin_manager = LYServerPluginManager::new(shared_data.clone());
        let plugin_manager = Arc::new(Mutex::new(plugin_manager));

        let shared_data_clone = Arc::clone(&shared_data);
        ctrlc::set_handler(move || {
            log::info!("Received Ctrl+C, shutting down LYServer...");
            shared_data_clone.shutdown_flag.store(true, Ordering::SeqCst);
        }).expect("Failed to set Ctrl+C handler");

        Self { 
            shared_data,

            plugin_manager,
        }
    }

    pub async fn run(&mut self) -> anyhow::Result<()> {
        self.plugin_manager.lock().await.init_messaging_loop().await?;

        let shared_data_clone = Arc::clone(&self.shared_data);
        let plugin_manager_clone = Arc::clone(&self.plugin_manager);

        tokio::select! {
            _ = async {
                while !shared_data_clone.shutdown_flag.load(Ordering::Relaxed) {
                    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
                }
            } => {
                log::info!("Shutdown flag set, destroying plugin manager...");
                if let Err(e) = plugin_manager_clone.lock().await.destroy().await {
                    log::error!("Failed to destroy plugin manager: {}", e);
                }
            },

            res = async {
                let mut locked_plugin_manager = plugin_manager_clone.lock().await;

                if let Err(e) = locked_plugin_manager.load_plugin("database@lyserver.local", |plugin_shared_data| async move {
                    Box::new(LYServerDatabasePlugin::new(plugin_shared_data) as Arc<dyn LYServerPlugin + Send + Sync>)
                }).await {
                    log::error!("Failed to load database plugin: {}", e);
                    return Err(anyhow::anyhow!("Failed to load database plugin"));
                }
    
                if let Err(e) = locked_plugin_manager.load_plugin("preferences@lyserver.local", |plugin_shared_data| async move {
                    Box::new(LYServerPreferencesPlugin::new(plugin_shared_data) as Arc<dyn LYServerPlugin + Send + Sync>)
                }).await {
                    log::error!("Failed to load preferences plugin: {}", e);
                    return Err(anyhow::anyhow!("Failed to load preferences plugin"));
                }
    
                if let Err(e) = locked_plugin_manager.load_plugin("http@lyserver.local", |plugin_shared_data| async move {
                    Box::new(LYServerHTTPServerPlugin::new(plugin_shared_data) as Arc<dyn LYServerPlugin + Send + Sync>)
                }).await {
                    log::error!("Failed to load HTTP server plugin: {}", e);
                    return Err(anyhow::anyhow!("Failed to load HTTP server plugin"));
                }

                locked_plugin_manager.init().await;

                log::info!("----------------------------------");
                log::info!("");
                log::info!("LYServer has started and is available at:");
                log::info!("     http://{}/", shared_data_clone.bind_address);
                log::info!("");
                log::info!("----------------------------------");

                if let Err(e) = locked_plugin_manager.wait_for_all_plugins().await {
                    log::error!("Unrecoverable error occurred in plugins: {}", e);
                }

                Ok(())
            } => {
                return res;
            },
        }

        let _ = plugin_manager_clone.lock().await.wait_for_all_plugins().await;

        log::info!("Goodbye");

        Ok(())
    }
}