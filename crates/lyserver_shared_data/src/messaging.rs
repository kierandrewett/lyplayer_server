use std::sync::Arc;

use lyserver_messaging_shared::LYServerMessageEvent;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::{broadcast::Sender, mpsc::{UnboundedReceiver, UnboundedSender}, RwLock};

use crate::LYServerSharedData;

#[async_trait::async_trait]
pub trait LYServerSharedDataMessaging {
    fn dispatch_event(&self, event: LYServerMessageEvent) -> anyhow::Result<()>;
    async fn register_plugin_messaging(
        &self,
        plugin_id: String,
        tx: Arc<RwLock<Sender<LYServerMessageEvent>>>,
    ) -> anyhow::Result<()>;
    async fn receive_event(&self) -> Option<LYServerMessageEvent>;
}

#[async_trait::async_trait]
impl LYServerSharedDataMessaging for LYServerSharedData {
    fn dispatch_event(&self, event: LYServerMessageEvent) -> anyhow::Result<()> {
        self.messaging_global_tx
            .send(event)
            .map_err(|e| anyhow::anyhow!("Failed to send global event: {}", e))?;

        Ok(())
    }

    async fn register_plugin_messaging(
        &self,
        plugin_id: String,
        tx: Arc<RwLock<Sender<LYServerMessageEvent>>>,
    ) -> anyhow::Result<()> {
        log::info!("Registering plugin messaging for '{}'", plugin_id);

        if self.messaging_plugin_tx.read().await.contains_key(&plugin_id) {
            return Err(anyhow::anyhow!(
                "Plugin '{}' is already registered for messaging",
                plugin_id
            ));
        }

        self.messaging_plugin_tx
            .write()
            .await
            .insert(plugin_id, tx);

        Ok(())
    }

    async fn receive_event(&self) -> Option<LYServerMessageEvent> {
        let mut rx = self.messaging_global_tx.subscribe();

        rx.recv().await.ok()
    }
}