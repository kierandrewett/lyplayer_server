mod api;

use std::{net::SocketAddr, sync::{Arc}};

use actix_web::{middleware::Logger, web, App, HttpServer};

use lyserver_http_shared::{router::LYServerHTTPRouter, LYServerHTTPRequest};
use lyserver_messaging_shared::LYServerMessageEvent;
use lyserver_plugin_common::{LYServerPlugin, LYServerPluginMetadata};
use lyserver_plugin_shared_data::LYServerPluginSharedData;
use lyserver_shared_data::{LYServerSharedData, LYServerSharedDataStatus as _};
use serde_json::Value;
use tokio::sync::{Mutex, RwLock};

pub struct LYServerHTTPServerPlugin {
    plugin_shared_data: Arc<LYServerPluginSharedData>,
}

impl LYServerHTTPServerPlugin {
    pub fn new(plugin_shared_data: Arc<LYServerPluginSharedData>) -> Arc<Self> {
        Arc::new(Self {
            plugin_shared_data
        })
    }

    pub async fn handle_http_request(plugin_shared_data: Arc<LYServerPluginSharedData>, event: LYServerMessageEvent) -> anyhow::Result<()> {
        let request = event.data_as::<LYServerHTTPRequest>().expect("Failed to deserialize LYServerHTTPRequest");

        let mut router = LYServerHTTPRouter::new();

        router.add_matcher("GET", "/", |route| {
            async move {
                let response = route.request.build_response()
                    .body("Welcome to LYServer")
                    .build();

                Ok(response)
            }
        });

        let plugin_shared_data_clone = Arc::clone(&plugin_shared_data);
        router.add_matcher("GET", "/status", move |route| {
            let plugin_shared_data_clone = Arc::clone(&plugin_shared_data_clone);

            async move {
                let server_status_data = plugin_shared_data_clone.app_shared_data.get_server_status().await;

                let response = route.request.build_response()
                    .json(server_status_data)
                    .build();
                
                Ok(response)
            }
        });

        router.add_matcher("GET", "/favicon.ico", |route| {
            async move {
                let response = route.request.not_found_response();
                Ok(response)
            }
        });

        if let Some(response) = router.respond(request).await {
            plugin_shared_data.reply_event("http_response", event, response).await?;
        }

        Ok(())
    }
}

#[async_trait::async_trait]
impl LYServerPlugin for LYServerHTTPServerPlugin {
    fn metadata(&self) -> LYServerPluginMetadata {
        LYServerPluginMetadata::builder()
            .id("http@lyserver.local")
            .name("LYServerHTTPServerPlugin")
            .description("HTTP server plugin for LYServer")
            .version(env!("CARGO_PKG_VERSION").to_string())
            .author("LYServer")
            .build()
    }

    async fn init(&self) -> anyhow::Result<()> {
        let bind_addr = self.plugin_shared_data.app_shared_data.bind_address;

        let shared_plugin_data_clone = Arc::clone(&self.plugin_shared_data);
        self.plugin_shared_data.dispatch_init_event().await?;

        let server = HttpServer::new(move || {
            App::new()
                .app_data(web::Data::from(Arc::clone(&shared_plugin_data_clone)))
                .service(crate::api::router())
        })
        .bind(bind_addr)?;

        log::info!("LYServer is started at http://{}.", bind_addr);
    
        tokio::select! {
            res = server.run() => {
                return res.map_err(|e| anyhow::anyhow!("Failed to start HTTP server: {}", e));
            },

            _ = async {
                while let Some(event) = self.plugin_shared_data.receive_event().await {
                    if event.event_type == "http_request" {
                        let plugin_shared_data_clone = Arc::clone(&self.plugin_shared_data);

                        tokio::spawn(async {
                            if let Err(e) = Self::handle_http_request(plugin_shared_data_clone, event).await {
                                log::error!("Error handling HTTP event: {}", e);
                            }
                        });
                    }
                }
            } => {
                return Ok(());
            }
        }
    }

    async fn destroy(&self) -> anyhow::Result<()> {
        // Cleanup logic for the HTTP server plugin

        Ok(())
    }

    async fn invoke(&self, method: &str, args: Vec<String>) -> anyhow::Result<Value> {
        // Handle incoming requests to the HTTP server plugin
        // This is a placeholder implementation
        Ok(format!("Invoked method: {}, args: {:?}", method, args).into())
    }

    async fn receive(&self, method: &str, args: Vec<String>) -> anyhow::Result<Value> {
        // Handle incoming requests to the HTTP server plugin
        // This is a placeholder implementation
        Ok(format!("Received method: {}, args: {:?}", method, args).into())
    }
}