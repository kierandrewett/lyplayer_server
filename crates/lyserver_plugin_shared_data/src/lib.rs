use std::{clone, collections::HashSet, sync::Arc, time::Duration};

use lyserver_messaging_shared::{LYServerMessageEvent, LYServerMessageEventTarget};
use lyserver_shared_data::{LYServerSharedData, LYServerSharedDataMessaging as _};
use tokio::sync::RwLock;

pub struct LYServerPluginSharedData {
    pub app_shared_data: Arc<LYServerSharedData>,

    pub plugin_id: Option<String>,

    tx: tokio::sync::broadcast::Sender<LYServerMessageEvent>,
    rx: Arc<RwLock<Option<tokio::sync::broadcast::Receiver<LYServerMessageEvent>>>>,
    rx_sync: Arc<std::sync::Mutex<Option<tokio::sync::broadcast::Receiver<LYServerMessageEvent>>>>,

    tx_2: Arc<tokio::sync::mpsc::UnboundedSender<LYServerMessageEvent>>,
    rx_2: Arc<RwLock<tokio::sync::mpsc::UnboundedReceiver<LYServerMessageEvent>>>,
}

impl LYServerPluginSharedData {
    pub fn new(shared_data: Arc<LYServerSharedData>) -> Self {
        let (tx, _) = tokio::sync::broadcast::channel::<LYServerMessageEvent>(1024);
        let (tx_2, rx_2) = tokio::sync::mpsc::unbounded_channel::<LYServerMessageEvent>();

        Self {
            app_shared_data: shared_data,

            plugin_id: None,

            tx,
            rx: Arc::new(RwLock::new(None)),
            rx_sync: Arc::new(std::sync::Mutex::new(None)),

            tx_2: Arc::new(tx_2),
            rx_2: Arc::new(RwLock::new(rx_2)),
        }
    }

    pub async fn register_plugin_messaging(
        &mut self,
        plugin_id: String,
    ) -> anyhow::Result<()> {
        self.plugin_id = Some(plugin_id.clone());

        let rx = self.tx.subscribe();
        *self.rx.write().await = Some(rx);

        let rx_sync = self.tx.subscribe();
        *self.rx_sync.lock().unwrap() = Some(rx_sync);

        self.app_shared_data
            .register_plugin_messaging(plugin_id.clone(), self.tx.clone().into())
            .await?;

        let plugin_id_clone = plugin_id.clone();
        let mut rx_clone = self.tx.subscribe();

        let tx_2_clone = Arc::clone(&self.tx_2);

        tokio::spawn(async move {
            log::error!("SPAWNED MESSAGING TOKIO TASK FOR PLUGIN: {}", plugin_id_clone);

            loop {
                if let Ok(event) = rx_clone.recv().await {
                    log::debug!("[Plugin Messaging SHARED DATA RUNIME: {}] Received event '{}' ({}): {} -> {}",
                        plugin_id_clone,
                        event.event_type, event.event_id,
                        event.event_sender.to_string(), event.event_target.to_string()
                    );

                    let _ = tx_2_clone.send(event.clone());
                } else {
                    log::warn!("[Plugin Messaging SHARED DATA RUNIME: {}] Receiver closed, stopping event loop", plugin_id_clone);
                }
            }
        });

        Ok(())
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
            self.tx.subscribe()
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
        // let tx_read = self.rx_2.clone();

        let mut rx = {
            let rx = self.tx.subscribe();
            rx
        };
    
        let fut = async {
            loop {
                match rx.recv().await {
                    Ok(event) => {
                        if predicate(&event) {
                            return Some(event);
                        }
                    }
                    _ => {
                        log::error!("Broadcast channel closed");
                    }
                }
            }
        };
    
        tokio::time::timeout(timeout, fut).await.ok().flatten()
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