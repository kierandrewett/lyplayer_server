use std::{collections::{HashMap, HashSet}, sync::Arc, time::Duration};

use chrono::{DateTime, Utc};
use lyserver_http_shared::LYServerHTTPRequest;
use lyserver_plugin_common::{LYServerPlugin, LYServerPluginMetadata};
use lyserver_plugin_shared_data::LYServerPluginSharedData;
use lyserver_shared_data::{LYServerSharedData, LYServerSharedDataDatabase, LYServerSharedDataPlugins};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Clone)]
pub enum LYServerPreference {
    Null(LYServerPreferenceTyped<Option<()>>),
    I32(LYServerPreferenceTyped<i32>),
    F32(LYServerPreferenceTyped<f32>),
    U32(LYServerPreferenceTyped<u32>),
    Boolean(LYServerPreferenceTyped<bool>),
    String(LYServerPreferenceTyped<String>),
    JSON(LYServerPreferenceTyped<Value>),
}

impl Serialize for LYServerPreference {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            LYServerPreference::Null(pref) => pref.serialize(serializer),
            LYServerPreference::I32(pref) => pref.serialize(serializer),
            LYServerPreference::F32(pref) => pref.serialize(serializer),
            LYServerPreference::U32(pref) => pref.serialize(serializer),
            LYServerPreference::Boolean(pref) => pref.serialize(serializer),
            LYServerPreference::String(pref) => pref.serialize(serializer),
            LYServerPreference::JSON(pref) => pref.serialize(serializer),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LYServerPreferenceTyped<T> {
    pub key: String,
    pub value: T,
    pub native_type: u32,
    pub is_locked: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub struct LYServerPreferencesPlugin {
    plugin_shared_data: Arc<LYServerPluginSharedData>,
}

const SELECT_PREFERENCES_QUERY: &'static str = "select p.key, p.value, p.native_type_id, p.is_locked, p.created_at, p.updated_at from preferences p";
const SELECT_PREFERENCES_QUERY_WITH_KEY: &'static str = "select p.key, p.value, p.native_type_id, p.is_locked, p.created_at, p.updated_at from preferences where p.key = ?";

impl LYServerPreferencesPlugin {
    pub fn new(plugin_shared_data: Arc<LYServerPluginSharedData>) -> Arc<Self> {
        Arc::new(Self {
            plugin_shared_data
        })
    }

    pub async fn get_all_preferences(&self) -> anyhow::Result<Vec<LYServerPreference>> {
        self.plugin_shared_data.app_shared_data.query("preferences".to_string(), SELECT_PREFERENCES_QUERY.to_string(), vec![]).await
            .and_then(|result| {
                result.as_array()
                    .ok_or_else(|| anyhow::anyhow!("Expected an array of preferences"))
                    .and_then(|items| {
                        items.iter()
                            .map(Self::deserialize_preference)
                            .collect::<Result<Vec<_>, _>>()
                    })
            })
    }

    pub async fn get_preference_by_id(&self, pref_name: &str) -> anyhow::Result<LYServerPreference> {
        self.plugin_shared_data.app_shared_data.query("preferences".to_string(), SELECT_PREFERENCES_QUERY_WITH_KEY.to_string(), vec![pref_name.to_string()]).await
            .and_then(|result| {
                result.as_array()
                    .ok_or_else(|| anyhow::anyhow!("Expected an array of preferences"))
                    .and_then(|items| {
                        items.first()
                            .ok_or_else(|| anyhow::anyhow!("Preference not found"))
                            .and_then(Self::deserialize_preference)
                    })
            })
    }    

    pub fn deserialize_preference(result: &Value) -> anyhow::Result<LYServerPreference> {
        log::debug!("Deserializing preference: {:?}", result);

        let native_type_id = result.get("native_type_id").cloned()
            .ok_or_else(|| anyhow::anyhow!("Preference native_type_id not found"))?;

        let native_type_id = native_type_id
            .as_str();

        log::debug!("Deserializing preference with native_type_id: {:?}", native_type_id);

        let native_type_id = native_type_id.and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(0)
            as u32;

        let key = result.get("key").cloned()
            .ok_or_else(|| anyhow::anyhow!("Preference key not found"))?
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Preference key is not a valid string"))?
            .to_string();

        let value = result.get("value").cloned()
            .ok_or_else(|| anyhow::anyhow!("Preference value not found"))?
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Preference value is not a valid string"))?
            .to_string();

        let is_locked = result.get("is_locked").cloned()
            .ok_or_else(|| anyhow::anyhow!("Preference is_locked not found"))?
            .as_bool()
            .unwrap_or(false);

        let created_at = result.get("created_at").cloned()
            .ok_or_else(|| anyhow::anyhow!("Preference created_at not found"))?
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Preference created_at is not a valid string"))?
            .to_string();

        let created_at = chrono::NaiveDateTime::parse_from_str(&created_at, "%Y-%m-%d %H:%M:%S")?
            .and_utc();

        let updated_at = result.get("updated_at").cloned()
            .ok_or_else(|| anyhow::anyhow!("Preference updated_at not found"))?
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Preference created_at is not a valid string"))?
            .to_string();

        let updated_at = chrono::NaiveDateTime::parse_from_str(&updated_at, "%Y-%m-%d %H:%M:%S")?
            .and_utc();

        let native_preference_type = match native_type_id {
            1 => LYServerPreference::I32(LYServerPreferenceTyped {
                key,
                value: value.parse::<i32>().map_err(|e| anyhow::anyhow!("Failed to parse i32: {}", e))?,
                native_type: native_type_id,
                is_locked,
                created_at,
                updated_at,
            }),
            2 => LYServerPreference::F32(LYServerPreferenceTyped {
                key,
                value: value.parse::<f32>().map_err(|e| anyhow::anyhow!("Failed to parse f32: {}", e))?,
                native_type: native_type_id,
                is_locked,
                created_at,
                updated_at,
            }),
            3 => LYServerPreference::U32(LYServerPreferenceTyped {
                key,
                value: value.parse::<u32>().map_err(|e| anyhow::anyhow!("Failed to parse u32: {}", e))?,
                native_type: native_type_id,
                is_locked,
                created_at,
                updated_at,
            }),
            4 => LYServerPreference::Boolean(LYServerPreferenceTyped {
                key,
                value: value.parse::<bool>().map_err(|e| anyhow::anyhow!("Failed to parse bool: {}", e))?,
                native_type: native_type_id,
                is_locked,
                created_at,
                updated_at,
            }),
            5 => LYServerPreference::String(LYServerPreferenceTyped {
                key,
                value,
                native_type: native_type_id,
                is_locked,
                created_at,
                updated_at,
            }),
            6 => LYServerPreference::JSON(LYServerPreferenceTyped {
                key,
                value: serde_json::from_str(&value).map_err(|e| anyhow::anyhow!("Failed to parse JSON: {}", e))?,
                native_type: native_type_id,
                is_locked,
                created_at,
                updated_at,
            }),
            _ => LYServerPreference::Null(LYServerPreferenceTyped {
                key,
                value: None,
                native_type: native_type_id,
                is_locked,
                created_at,
                updated_at,
            }),
        };

        Ok(native_preference_type)
    }
}

#[async_trait::async_trait]
impl LYServerPlugin for LYServerPreferencesPlugin {
    fn metadata(&self) -> LYServerPluginMetadata {
        LYServerPluginMetadata::builder()
            .id("preferences@lyserver.local")
            .name("LYServerPreferencesPlugin")
            .description("Preferences plugin for LYPlayer")
            .version(env!("CARGO_PKG_VERSION").to_string())
            .author("LYPlayer")
            .build()
    }

    async fn init(&self) -> anyhow::Result<()> {
        self.plugin_shared_data.dispatch_init_event().await?;

        while let Some(event) = self.plugin_shared_data.receive_event().await {
            if event.event_type == "http_request" {
                let request = event.data_as::<LYServerHTTPRequest>().expect("Failed to deserialize LYServerHTTPRequest");

                if request.match_request("GET", "/preferences") {
                    let response = request.build_response()
                        .json(self.get_all_preferences().await?)
                        .build();

                    self.plugin_shared_data.reply_event("http_response", event, response).await?;
                }
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

                self.get_preference_by_id(&pref_name).await
                    .and_then(|pref| {
                        serde_json::to_value(pref)
                            .map_err(|e| anyhow::anyhow!("Failed to serialize preference: {}", e))
                    })
            },
            "get_all" => {
                self.get_all_preferences().await
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