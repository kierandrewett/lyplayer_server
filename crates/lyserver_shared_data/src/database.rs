use std::{any, path::{Path, PathBuf}};

use serde_json::Value;

use crate::{LYServerSharedData, LYServerSharedDataPlugins};

#[async_trait::async_trait]
pub trait LYServerSharedDataDatabase {
    async fn query(&self, database: String, query: String, args: Vec<String>) -> anyhow::Result<Value>;
}

#[async_trait::async_trait]
impl LYServerSharedDataDatabase for LYServerSharedData {
    async fn query(&self, database: String, query: String, args: Vec<String>) -> anyhow::Result<Value> {
        if let Some(db) = self.get_plugin_by_id("database@lyserver.local").await {
            let mut query_params = vec![database.to_string(), query.to_string()];
            query_params.extend(args.into_iter().map(|arg| arg.into()));

            let result = db.invoke("query", query_params).await?;

            Ok(result)
        } else {
            Err(anyhow::anyhow!("Database plugin not found"))
        }
    }
}