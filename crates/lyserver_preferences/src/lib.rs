use std::{collections::{HashMap, HashSet}, sync::Arc, time::Duration};

use chrono::{DateTime, Utc};
use lyserver_http_shared::{router::LYServerHTTPRouter, LYServerHTTPRequest};
use lyserver_messaging_shared::LYServerMessageEvent;
use lyserver_plugin_common::{LYServerPlugin, LYServerPluginMetadata};
use lyserver_plugin_shared_data::LYServerPluginSharedData;
use lyserver_shared_data::{LYServerSharedData, LYServerSharedDataDatabase, LYServerSharedDataPlugins};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::api::{LYServerPreferenceType, LYServerPreferenceTyped, LYServerPreferencesAPI};

mod api;

pub struct LYServerPreferencesPlugin {
    plugin_shared_data: Arc<LYServerPluginSharedData>,

    api: Arc<LYServerPreferencesAPI>,
}

impl LYServerPreferencesPlugin {
    pub fn new(plugin_shared_data: Arc<LYServerPluginSharedData>) -> Arc<Self> {
        let plugin_shared_data_clone = Arc::clone(&plugin_shared_data);

        Arc::new(Self {
            plugin_shared_data,
            api: Arc::new(LYServerPreferencesAPI::new(plugin_shared_data_clone)),
        })
    }
}

#[async_trait::async_trait]
impl LYServerPlugin for LYServerPreferencesPlugin {
    fn metadata(&self) -> LYServerPluginMetadata {
        LYServerPluginMetadata::builder()
            .id("preferences@lyserver.local")
            .name("LYServerPreferencesPlugin")
            .description("Preferences plugin for LYServer")
            .version(env!("CARGO_PKG_VERSION").to_string())
            .author("LYServer")
            .build()
    }

    async fn init(&self) -> anyhow::Result<()> {
        self.api.set_server_version_preference().await?;

        self.plugin_shared_data.dispatch_init_event().await?;

        Ok(())
    }

    async fn handle_message_event(
        &self,
        event: LYServerMessageEvent,
    ) -> anyhow::Result<()> {
        if event.event_type == "http_request" {
            let request = event.data_as::<LYServerHTTPRequest>().expect("Failed to deserialize LYServerHTTPRequest");

            let mut router = LYServerHTTPRouter::new();

            let api_clone = Arc::clone(&self.api);
            router.add_matcher("GET", "/preferences", move |route| {
                let api_clone = Arc::clone(&api_clone);

                async move {
                    let all_preferences = api_clone.get_all_preferences().await?;
                
                    let response = route.request.build_response()
                        .json(json!({
                            "ok": true,
                            "data": all_preferences
                        }))
                        .build();
            
                    Ok(response)
                }
            });

            let api_clone = Arc::clone(&self.api);
            router.add_matcher("GET", "/preferences/:key", move |route| {
                let api_clone = Arc::clone(&api_clone);

                async move {
                    let key = route.params.get("key")
                        .ok_or_else(|| anyhow::anyhow!("Missing 'key' parameter in request"))?
                        .to_string();

                    if let Ok(preference) = api_clone.get_preference_by_id(&key).await {
                        let response = route.request.build_response()
                            .json(json!({
                                "ok": true,
                                "data": preference
                            }))
                            .build();

                        Ok(response)
                    } else {
                        let error_response = route.request.not_found_response();
                        Ok(error_response)
                    }
                }
            });

            let api_clone = Arc::clone(&self.api);
            router.add_matcher("PUT", "/preferences", move |route| {
                let api_clone = Arc::clone(&api_clone);

                async move {
                    #[derive(Deserialize)]
                    struct PutPreferenceRequest {
                        key: String,
                        value: Value,
                    }

                    let body = route.request.body_json::<PutPreferenceRequest>()?;

                    let new_native_type = match &body.value {
                        Value::Null => LYServerPreferenceType::Null,
                        Value::Bool(_) => LYServerPreferenceType::Boolean,
                        Value::Number(num) => {
                            if num.is_f64() {
                                LYServerPreferenceType::F32
                            } else if num.is_u64() {
                                LYServerPreferenceType::U32
                            } else if num.is_i64() {
                                LYServerPreferenceType::I32
                            } else {
                                return Err(anyhow::anyhow!("Invalid number type in preference value"));
                            }
                        },
                        Value::String(_) => LYServerPreferenceType::String,
                        Value::Array(_) => LYServerPreferenceType::JSON,
                        Value::Object(_) => LYServerPreferenceType::JSON,
                    };

                    let pref_exists = api_clone.does_preference_exist(&body.key).await;

                    api_clone.set_preference(
                        body.key.clone(), 
                        body.value, 
                        new_native_type
                    ).await?;

                    let new_preference = api_clone.get_preference_by_id(&body.key).await?;

                    let response = route.request.build_response()
                        .status_code(if pref_exists { 200 } else { 201 })
                        .json(json!({
                            "ok": true,
                            "data": new_preference
                        }))
                        .build();

                    Ok(response)
                }
            });

            let api_clone = Arc::clone(&self.api);
            router.add_matcher("DELETE", "/preferences", move |route| {
                let api_clone = Arc::clone(&api_clone);

                async move {
                    #[derive(Deserialize)]
                    struct DeletePreferenceRequest {
                        key: String,
                    }

                    let body = route.request.body_json::<DeletePreferenceRequest>()?;

                    api_clone.delete_preference_by_id(&body.key).await?;

                    let response = route.request.build_response()
                        .status_code(204)
                        .build();
                    Ok(response)
                }
            });

            if let Some(response) = router.respond(request).await {
                self.plugin_shared_data.reply_event("http_response", event, response).await?;
            }
        }

        Ok(())
    }

    async fn destroy(&self) -> anyhow::Result<()> {
        Ok(())
    }

    async fn invoke(&self, method: &str, args: Vec<String>) -> anyhow::Result<Value> {
        match method {
            "get" => {
                if args.len() != 1 {
                    return Err(anyhow::anyhow!("Invalid arguments for 'get' method"));
                }

                let pref_name = args.get(0)
                    .cloned()
                    .ok_or_else(|| anyhow::anyhow!("Missing preference name argument for get method."))?;

                self.api.get_preference_by_id(&pref_name).await
                    .and_then(|pref| {
                        serde_json::to_value(pref)
                            .map_err(|e| anyhow::anyhow!("Failed to serialize preference: {}", e))
                    })
            },
            "get_all" => {
                self.api.get_all_preferences().await
                    .and_then(|prefs| {
                        serde_json::to_value(prefs)
                            .map_err(|e| anyhow::anyhow!("Failed to serialize preferences: {}", e))
                    })
            }
            _ => Err(anyhow::anyhow!("Unknown method: {}", method)),
        }
    }

    async fn receive(&self, method: &str, args: Vec<String>) -> anyhow::Result<Value> {
        Ok("Received method not implemented".to_string().into())
    }
}