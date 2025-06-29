use std::{sync::Arc, time::Duration};

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
        tx: Arc<Sender<LYServerMessageEvent>>,
    ) -> anyhow::Result<()>;
    async fn receive_event(&self) -> Option<LYServerMessageEvent>;
}

#[async_trait::async_trait]
impl LYServerSharedDataMessaging for LYServerSharedData {
    fn dispatch_event(&self, event: LYServerMessageEvent) -> anyhow::Result<()> {
        let messaging_global_tx_clone = Arc::clone(&self.messaging_global_tx);

        messaging_global_tx_clone
            .send(event)
            .map_err(|e| anyhow::anyhow!("Failed to send global event: {}", e))?;
        
        Ok(())
    }

    async fn register_plugin_messaging(
        &self,
        plugin_id: String,
        tx: Arc<Sender<LYServerMessageEvent>>,
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
            .insert(plugin_id.clone(), tx);

        Ok(())
    }

    async fn receive_event(&self) -> Option<LYServerMessageEvent> {
        let mut rx = self.messaging_global_tx.subscribe();

        match rx.recv().await {
            Ok(event) => Some(event),
            Err(e) => {
                log::warn!("Failed to receive event: {}", e);
                None
            }
        }
    }
}