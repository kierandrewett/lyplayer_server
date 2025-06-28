use std::{
    path::{Path, PathBuf},
    time::Duration,
};

use lyserver_plugin_common::LYServerPluginMetadata;
use serde::{Deserialize, Serialize};

use crate::LYServerSharedData;

#[derive(Serialize, Deserialize, Debug)]
pub struct LYServerSharedDataStatusData {
    pub data_dir: String,
    pub version: String,
    pub uptime: u128,
    pub start_time: u128,
    pub loaded_plugins: Vec<LYServerPluginMetadata>,
}

pub trait LYServerSharedDataStatus {
    fn get_server_status(&self) -> std::pin::Pin<Box<dyn std::future::Future<Output = LYServerSharedDataStatusData> + Send>>;
}

impl LYServerSharedDataStatus for LYServerSharedData {
    fn get_server_status(&self) -> std::pin::Pin<Box<dyn std::future::Future<Output = LYServerSharedDataStatusData> + Send>> {
        let maybe_canonicalized_data_dir = self
            .data_dir
            .canonicalize()
            .map(|p| p.display().to_string())
            .unwrap_or(self.data_dir.display().to_string());

        let version = self.version;
        let start_ts = self.start_ts.clone();
        let loaded_plugins = self.loaded_plugins.clone();

        Box::pin(async move {
            let loaded_plugins = loaded_plugins.read().await
                .iter()
                .map(|(_, metadata, _)| metadata.clone())
                .collect::<Vec<LYServerPluginMetadata>>();

            LYServerSharedDataStatusData {
                data_dir: maybe_canonicalized_data_dir,
                version: version.to_string(),
                uptime: start_ts.elapsed().unwrap_or_default().as_millis(),
                start_time: start_ts
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or(Duration::from_secs(0))
                    .as_secs() as u128,
                loaded_plugins,
            }
        })
    }
}
