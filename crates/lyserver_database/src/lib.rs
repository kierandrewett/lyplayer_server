mod database;
mod databases;

use std::{collections::{HashMap, HashSet}, sync::Arc, time::Duration};

use futures::future::BoxFuture;
use lyserver_plugin_common::{LYServerPlugin, LYServerPluginMetadata};
use lyserver_plugin_shared_data::LYServerPluginSharedData;
use lyserver_shared_data::LYServerSharedData;
use serde_json::Value;
use sqlx::{sqlite::SqlitePoolOptions, Pool, Sqlite, Row as _, Column as _, ValueRef as _, TypeInfo as _};
use tokio::sync::{Mutex, RwLock};

use crate::{database::{LYServerDatabase, LYServerDatabaseConnection, LYServerDatabaseLifecycle as _}, databases::preferences::LYServerPreferencesDatabase};

pub struct LYServerDatabasePlugin {
    plugin_shared_data: Arc<LYServerPluginSharedData>,
    preferences: Arc<RwLock<LYServerPreferencesDatabase>>,
}

impl LYServerDatabasePlugin {
    pub fn new(plugin_shared_data: Arc<LYServerPluginSharedData>) -> Arc<Self> {
        let shared_data = plugin_shared_data.app_shared_data.clone();

        let preferences = LYServerPreferencesDatabase::new(shared_data.clone());
        let preferences = Arc::new(RwLock::new(preferences));

        Arc::new(Self {
            plugin_shared_data,
            preferences,
        })
    }

    pub async fn with_db_connection<F, T>(
        &self,
        database: Arc<Pool<Sqlite>>,
        f: F,
    ) -> anyhow::Result<T>
    where
        F: FnOnce(&Pool<Sqlite>) -> BoxFuture<'static, anyhow::Result<T>>,
    {
        f(&database).await
    }

}

#[async_trait::async_trait]
impl LYServerPlugin for LYServerDatabasePlugin {
    fn metadata(&self) -> lyserver_plugin_common::LYServerPluginMetadata {
        LYServerPluginMetadata::builder()
            .id("database@lyserver.local")
            .name("LYPlayerDatabasePlugin")
            .description("Database plugin for LYPlayer")
            .version(env!("CARGO_PKG_VERSION").to_string())
            .author("LYPlayer")
            .build()
    }

    async fn init(&self) -> anyhow::Result<()> {
        self.preferences.write().await.connect().await?;

        self.plugin_shared_data.dispatch_init_event().await?;

        loop {
            let _ = self.preferences.write().await.health_check().await;

            tokio::time::sleep(Duration::from_secs(60)).await;
        }
    }

    async fn destroy(&self) -> anyhow::Result<()> {
        self.preferences.write().await.disconnect().await?;

        Ok(())
    }

    async fn invoke(&self, method: &str, args: Vec<String>) -> anyhow::Result<Value> {
        match method {
            "query" => {
                let database = args.get(0).cloned().ok_or_else(|| {
                    anyhow::anyhow!("Missing database argument for 'query' method")
                })?;
                let query = args.get(1).cloned().ok_or_else(|| {
                    anyhow::anyhow!("Missing query argument for 'query' method")
                })?;
                let query_args = args.get(2..).map_or_else(
                    || vec![],
                    |args| args.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
                );

                match database.as_str() {
                    "preferences" => {
                        let db_ref = self.preferences.read().await.get_pool().await?;
                        
                        self.with_db_connection(db_ref, |pool| {
                            let pool = pool.clone();

                            Box::pin(async move {
                                let mut query_obj = sqlx::query(&query);

                                for (i, arg) in query_args.iter().enumerate() {
                                    if let Err(e) = query_obj.try_bind(arg) {
                                        return Err(anyhow::anyhow!("Failed to bind argument {}: {}", i, e));
                                    }
                                }

                                let rows = query_obj.fetch_all(&pool).await?;

                                log::info!("Executing query on {} database: {:#?}", database, query);

                                let data = rows.iter()
                                    .map(|row| {
                                        let mut row_map = HashMap::new();
                                        for (i, column) in row.columns().iter().enumerate() {
                                            let column_name = column.name().to_string();
                                            let value_ref = row.try_get_raw(i).ok();

                                            let value = match value_ref {
                                                Some(val) if !val.is_null() => {
                                                    match val.type_info().name() {
                                                        "INTEGER" | "INT" => row.try_get::<i64, _>(i).ok().map(|v| v.to_string()),
                                                        "REAL" => row.try_get::<f64, _>(i).ok().map(|v| v.to_string()),
                                                        "TEXT" => row.try_get::<String, _>(i).ok(),
                                                        "BOOLEAN" => row.try_get::<bool, _>(i).ok().map(|v| v.to_string()),
                                                        _ => row.try_get::<String, _>(i).ok(),
                                                    }
                                                },
                                                _ => None,
                                            };

                                            row_map.insert(column_name, value);
                                        }
                                        row_map
                                    })
                                    .collect::<Vec<_>>();

                                Ok(serde_json::to_value(&data)?)
                            }) as BoxFuture<'static, anyhow::Result<Value>>
                        }).await
                    },
                    _ => Err(anyhow::anyhow!("Unknown database: {}", database)),
                }
            },
            _ => Err(anyhow::anyhow!("Unknown method: {}", method)),
        }
    }

    async fn receive(&self, method: &str, args: Vec<String>) -> anyhow::Result<Value> {
        Ok("Received method not implemented".to_string().into())
    }
}