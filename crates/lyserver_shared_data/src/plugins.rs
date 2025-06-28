use std::{path::{Path, PathBuf}, sync::Arc};

use lyserver_plugin_common::{LYServerPlugin, LYServerPluginMetadata};
use tokio::sync::{Mutex, RwLock};

use crate::LYServerSharedData;

pub type LYServerPluginInstance = Arc<dyn LYServerPlugin + Send + Sync>;

#[async_trait::async_trait]
pub trait LYServerSharedDataPlugins {
    async fn get_plugin_by_id(&self, id: &str) -> Option<LYServerPluginInstance>;
    async fn get_plugin_metadata_by_id(&self, id: &str) -> Option<LYServerPluginMetadata>;
}

#[async_trait::async_trait]
impl LYServerSharedDataPlugins for LYServerSharedData {
    async fn get_plugin_by_id(&self, id: &str) -> Option<LYServerPluginInstance> {
        let loaded_plugins = self.loaded_plugins.read().await;
        let loaded_plugins = loaded_plugins.iter()
            .map(|(plugin, metadata, _)| {
                (plugin.clone(), metadata.clone())
            })
            .collect::<Vec<(LYServerPluginInstance, LYServerPluginMetadata)>>();

        loaded_plugins
            .clone()
            .into_iter()
            .find(|(_, metadata)| metadata.id == id)
            .map(|(plugin, _)| plugin.clone())
    }

    async fn get_plugin_metadata_by_id(&self, id: &str) -> Option<LYServerPluginMetadata> {
        let loaded_plugins = self.loaded_plugins.read().await;
        let loaded_plugins = loaded_plugins.iter()
            .map(|(_, metadata, _)| metadata.clone())
            .collect::<Vec<LYServerPluginMetadata>>();

        loaded_plugins
            .into_iter()
            .find(|metadata| metadata.id == id)
    }
}