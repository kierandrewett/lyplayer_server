use lyserver_messaging_shared::LYServerMessageEvent;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::Mutex;

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct LYServerPluginMetadata {
    pub id: String,
    pub name: String,
    pub description: String,
    pub version: String,
    pub author: String,
    pub wasm_entry_point: Option<String>,
}

impl LYServerPluginMetadata {
    pub fn builder() -> LYServerPluginMetadataBuilder {
        LYServerPluginMetadataBuilder::default()
    }   
}

pub struct LYServerPluginMetadataBuilder {
    metadata: LYServerPluginMetadata,
}

impl Default for LYServerPluginMetadataBuilder {
    fn default() -> Self {
        Self {
            metadata: LYServerPluginMetadata::default(),
        }
    }
}

impl LYServerPluginMetadataBuilder {
    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.metadata.id = id.into();
        self
    }

    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.metadata.name = name.into();
        self
    }

    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.metadata.description = description.into();
        self
    }

    pub fn version(mut self, version: impl Into<String>) -> Self {
        self.metadata.version = version.into();
        self
    }

    pub fn author(mut self, author: impl Into<String>) -> Self {
        self.metadata.author = author.into();
        self
    }

    pub fn wasm_entry_point(mut self, entry_point: impl Into<String>) -> Self {
        self.metadata.wasm_entry_point = Some(entry_point.into());
        self
    }

    pub fn build(self) -> LYServerPluginMetadata {
        self.metadata
    }
}

#[async_trait::async_trait]
pub trait LYServerPlugin: Send + Sync {
    fn metadata(&self) -> LYServerPluginMetadata;

    async fn init(&self) -> anyhow::Result<()>;
    async fn destroy(&self) -> anyhow::Result<()>;
    async fn invoke(&self, method: &str, args: Vec<String>) -> anyhow::Result<Value>;
    async fn receive(&self, method: &str, args: Vec<String>) -> anyhow::Result<Value>;
    async fn handle_message_event(&self, _: LYServerMessageEvent) -> anyhow::Result<()> {
        Ok(())
    }
}
