use std::{path::{Path, PathBuf}, sync::Arc};

use lyserver_plugin_common::{LYServerPlugin, LYServerPluginMetadata};
use tokio::sync::{Mutex, RwLock};

use crate::LYServerSharedData;

pub type LYServerPluginInstance = Arc<dyn LYServerPlugin + Send + Sync>;

#[async_trait::async_trait]
pub trait LYServerSharedDataPlugins {
    async fn get_plugin_by_id(&self, id: &str) -> Option<LYServerPluginInstance>;
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

        log::info!("Searching for plugin: {:?}", loaded_plugins.clone().into_iter().map(|(_, metadata)| metadata.id).collect::<Vec<String>>());

        loaded_plugins
            .clone()
            .into_iter()
            .find(|(_, metadata)| metadata.id == id)
            .map(|(plugin, _)| plugin.clone())
    }
}