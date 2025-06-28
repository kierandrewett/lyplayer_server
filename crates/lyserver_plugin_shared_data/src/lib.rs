use std::{collections::HashSet, sync::Arc, time::Duration};

use lyserver_messaging_shared::{LYServerMessageEvent, LYServerMessageEventTarget};
use lyserver_shared_data::{LYServerSharedData, LYServerSharedDataMessaging as _};
use tokio::sync::RwLock;

pub struct LYServerPluginSharedData {
    pub app_shared_data: Arc<LYServerSharedData>,

    plugin_id: Arc<RwLock<Option<String>>>,

    tx: Arc<RwLock<tokio::sync::broadcast::Sender<LYServerMessageEvent>>>,
}

impl LYServerPluginSharedData {
    pub fn new(shared_data: Arc<LYServerSharedData>) -> Self {
        let (tx, _) = tokio::sync::broadcast::channel::<LYServerMessageEvent>(512);

        Self {
            app_shared_data: shared_data,

            plugin_id: Arc::new(RwLock::new(None)),

            tx: Arc::new(RwLock::new(tx)),
        }
    }

    pub async fn register_plugin_messaging(
        &self,
        plugin_id: String,
    ) -> anyhow::Result<()> {
        self.plugin_id.write().await.replace(plugin_id.clone());

        self.app_shared_data
            .register_plugin_messaging(plugin_id, self.tx.clone())
            .await
    }

    pub fn dispatch_event(&self, event: LYServerMessageEvent) -> anyhow::Result<()> {
        self.app_shared_data.dispatch_event(event)
    }

    pub fn dispatch_raw_event(
        &self,
        event: String
    ) -> anyhow::Result<()> {
        let event_obj = serde_json::from_str::<LYServerMessageEvent>(&event)
            .map_err(|e| anyhow::anyhow!("Failed to parse event JSON: {}", e))?;

        self.dispatch_event(event_obj)
    }

    pub async fn create_event(
        &self,
        event_type: &str,
        target: LYServerMessageEventTarget,
        data: impl serde::Serialize,
    ) -> anyhow::Result<LYServerMessageEvent> {
        let plugin_id = self.plugin_id
            .read()
            .await
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Plugin ID is not set"))?
            .clone();

        let data = serde_json::to_value(data)
            .map_err(|e| anyhow::anyhow!("Failed to serialize event data: {}", e))?;

        Ok(LYServerMessageEvent::new(event_type, target, LYServerMessageEventTarget::Plugin(plugin_id), data))
    }

    pub fn receive_event_sync(&self) -> Option<LYServerMessageEvent> {
        let mut rx = self.tx.blocking_read().subscribe();

        rx.try_recv().ok()
    }

    pub async fn receive_event(&self) -> Option<LYServerMessageEvent> {
        let mut rx = self.tx.read().await.subscribe();

        rx.recv().await.ok()
    }

    pub async fn reply_event<T: serde::Serialize>(
        &self,
        event_type: &str,
        original_event: LYServerMessageEvent,
        data: T,
    ) -> anyhow::Result<()> {
        let plugin_id = self.plugin_id
            .read()
            .await
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Plugin ID is not set"))?
            .clone();

        let reply_event = original_event.reply(
            event_type,
            LYServerMessageEventTarget::Plugin(plugin_id),
            data
        )?;

        self.dispatch_event(reply_event)
    }

    pub async fn wait_until_event(
        &self,
        predicate: impl Fn(&LYServerMessageEvent) -> bool,
        timeout: Duration,
    ) -> Option<LYServerMessageEvent> {
        let deadline = tokio::time::Instant::now() + timeout;

        loop {
            let now = tokio::time::Instant::now();
            if now >= deadline {
                return None;
            }

            let remaining = deadline - now;

            let result = tokio::select! {
                event = self.receive_event() => event,
                _ = tokio::time::sleep(remaining) => return None,
            };

            if let Some(event) = result {
                if predicate(&event) {
                    return Some(event);
                }
            }
        }
    }

    pub async fn dispatch_init_event(&self) -> anyhow::Result<()> {
        let plugin_id = self.plugin_id
            .read()
            .await
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Plugin ID is not set"))?
            .clone();

        let init_event = LYServerMessageEvent::new(
            "plugin_init",
            LYServerMessageEventTarget::All,
            LYServerMessageEventTarget::Plugin(plugin_id),
            serde_json::Value::Null,
        );

        self.dispatch_event(init_event)
    }
}