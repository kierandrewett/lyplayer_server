use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LYServerMessageEvent {
    pub event_id: String,
    pub event_type: String,
    pub event_target: LYServerMessageEventTarget,
    pub event_sender: LYServerMessageEventTarget,
    pub data: Value,
}

impl LYServerMessageEvent {
    pub fn data(&self) -> Value {
        self.data.clone()
    }

    pub fn data_as<T: for<'de> serde::Deserialize<'de>>(&self) -> Result<T, serde_json::Error> {
        serde_json::from_value(self.data.clone())
    }

    pub fn reply<T: serde::Serialize>(&self, event_type: impl Into<String>, sender: LYServerMessageEventTarget, data: T) -> Result<Self, serde_json::Error> {
        Ok(Self {
            event_id: self.event_id.clone(),
            event_type: event_type.into(),
            event_target: self.event_sender.clone(),
            event_sender: sender,
            data: serde_json::to_value(data)?,
        })
    }
}

impl TryFrom<Value> for LYServerMessageEvent {
    type Error = anyhow::Error;

    fn try_from(value: Value) -> anyhow::Result<Self, Self::Error> {
        let event_id = value.get("event_id")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::Error::msg("Missing event_id"))?;

        let event_type = value.get("event_type")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::Error::msg("Missing event_type"))?;

        let event_target = value.get("event_target")
            .and_then(Value::as_str)
            .map(|s| LYServerMessageEventTarget::from(s.to_string()))
            .unwrap_or(LYServerMessageEventTarget::All);

        let event_sender = value.get("event_sender")
            .and_then(Value::as_str)
            .map(|s| LYServerMessageEventTarget::from(s.to_string()))
            .unwrap_or(LYServerMessageEventTarget::All);

        let data = value.get("data").cloned().unwrap_or(Value::Null);

        Ok(Self {
            event_id: event_id.to_string(),
            event_type: event_type.to_string(),
            event_target,
            event_sender,
            data,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LYServerMessageEventTarget {
    All,
    Plugin(String),
}

impl ToString for LYServerMessageEventTarget {
    fn to_string(&self) -> String {
        match self {
            LYServerMessageEventTarget::All => "all".to_string(),
            LYServerMessageEventTarget::Plugin(id) => id.clone(),
        }
    }
}

impl From<LYServerMessageEventTarget> for String {
    fn from(target: LYServerMessageEventTarget) -> Self {
        target.to_string()
    }
}

impl From<String> for LYServerMessageEventTarget {
    fn from(s: String) -> Self {
        LYServerMessageEventTarget::Plugin(s)
    }
}

impl From<&str> for LYServerMessageEventTarget {
    fn from(s: &str) -> Self {
        LYServerMessageEventTarget::Plugin(s.to_string())
    }
}

impl LYServerMessageEvent {
    pub fn new<T: Into<String>, U: Into<LYServerMessageEventTarget>, V: Into<Value>>(
        event_type: T,
        target: U,
        sender: U,
        data: V,
    ) -> Self {
        let event_id = lyserver_random_id::generate();

        Self {
            event_id,
            event_type: event_type.into(),
            event_target: target.into(),
            event_sender: sender.into(),
            data: data.into(),
        }
    }
}