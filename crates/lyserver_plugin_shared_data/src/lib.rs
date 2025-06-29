use std::{clone, collections::HashSet, sync::Arc, time::Duration};

use lyserver_messaging_shared::{LYServerMessageEvent, LYServerMessageEventTarget};
use lyserver_shared_data::{LYServerSharedData, LYServerSharedDataMessaging as _};
use tokio::sync::RwLock;

pub struct LYServerPluginSharedData {
    pub app_shared_data: Arc<LYServerSharedData>,

    pub plugin_id: Option<String>,

    tx: Arc<RwLock<tokio::sync::broadcast::Sender<LYServerMessageEvent>>>,
    rx: Arc<RwLock<Option<tokio::sync::broadcast::Receiver<LYServerMessageEvent>>>>,
    rx_sync: Arc<std::sync::Mutex<Option<tokio::sync::broadcast::Receiver<LYServerMessageEvent>>>>,
}

impl LYServerPluginSharedData {
    pub fn new(shared_data: Arc<LYServerSharedData>) -> Self {
        let (tx, _) = tokio::sync::broadcast::channel::<LYServerMessageEvent>(512);

        Self {
            app_shared_data: shared_data,

            plugin_id: None,

            tx: Arc::new(RwLock::new(tx)),
            rx: Arc::new(RwLock::new(None)),
            rx_sync: Arc::new(std::sync::Mutex::new(None)),
        }
    }

    pub async fn register_plugin_messaging(
        &mut self,
        plugin_id: String,
    ) -> anyhow::Result<()> {
        self.plugin_id = Some(plugin_id.clone());

        let rx = self.tx.read().await.subscribe();
        *self.rx.write().await = Some(rx);

        let rx_sync = self.tx.read().await.subscribe();
        *self.rx_sync.lock().unwrap() = Some(rx_sync);

        self.app_shared_data
            .register_plugin_messaging(plugin_id, self.tx.clone())
            .await
    }

    pub fn dispatch_event(&self, event: LYServerMessageEvent) -> anyhow::Result<()> {
        log::debug!("[Plugin Messaging] Dispatching event '{}' ({}): {} -> {}",
            event.event_type, event.event_id,
            event.event_sender.to_string(), event.event_target.to_string()
        );

        self.app_shared_data.dispatch_event(event)
    }

    pub fn dispatch_raw_event(
        &self,
        event: Vec<u8>
    ) -> anyhow::Result<()> {
        let event_obj = serde_cbor::from_slice::<LYServerMessageEvent>(&event)
            .map_err(|e| anyhow::anyhow!("Failed to parse event CBOR: {}", e))?;

        self.dispatch_event(event_obj)
    }

    pub async fn create_event(
        &self,
        event_type: &str,
        target: LYServerMessageEventTarget,
        data: impl serde::Serialize,
    ) -> anyhow::Result<LYServerMessageEvent> {
        let plugin_id = self.plugin_id
            .clone()
            .ok_or_else(|| anyhow::anyhow!("Plugin ID is not set"))?
            .clone();

        Ok(LYServerMessageEvent::new(event_type, target, LYServerMessageEventTarget::Plugin(plugin_id), data))
    }

    pub fn receive_event_sync(&self) -> Option<LYServerMessageEvent> {
        let mut rx = self.rx_sync.lock().unwrap();
        
        rx.as_mut().unwrap().try_recv().ok()
    }
    
    pub async fn receive_event(&self) -> Option<LYServerMessageEvent> {
        let mut rx = {
            let lock = self.tx.read().await;
            lock.subscribe()
        };
    
        match rx.recv().await {
            Ok(event) => Some(event),
            Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                log::warn!("Missed {} messages in plugin event stream", n);
                None
            }
            Err(_) => None,
        }
    }

    pub async fn reply_event<T: serde::Serialize>(
        &self,
        event_type: &str,
        original_event: LYServerMessageEvent,
        data: T,
    ) -> anyhow::Result<()> {
        let plugin_id = self.plugin_id
            .clone()
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
        predicate: impl Fn(&LYServerMessageEvent) -> bool + Send + 'static,
        timeout: Duration,
    ) -> Option<LYServerMessageEvent> {
        let deadline = tokio::time::Instant::now() + timeout;
    
        let mut rx = {
            let tx_clone = Arc::clone(&self.tx);
            let rx_lock = tx_clone.read().await;
            rx_lock.subscribe()
        };

        tokio::spawn(async move {
            loop {
                let now = tokio::time::Instant::now();
                if now >= deadline {
                    return None;
                }
        
                let remaining = deadline - now;
        
                let maybe_event = rx.try_recv();

                match maybe_event {
                    Ok(event) => {
                        if predicate(&event) {
                            return Some(event);
                        }
                    }
                    Err(tokio::sync::broadcast::error::TryRecvError::Lagged(n)) => {
                        log::warn!("Missed {} messages in plugin stream", n);
                    }
                    Err(_) => {}
                }

                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            }
        }).await.expect("Failed to wait for event")
    }    

    pub async fn dispatch_init_event(&self) -> anyhow::Result<()> {
        let plugin_id = self.plugin_id
            .clone()
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Plugin ID is not set"))?
            .clone();

        let init_event = LYServerMessageEvent::new(
            "plugin_init",
            LYServerMessageEventTarget::All,
            LYServerMessageEventTarget::Plugin(plugin_id),
            serde_cbor::Value::Null,
        );

        self.dispatch_event(init_event)
    }
}