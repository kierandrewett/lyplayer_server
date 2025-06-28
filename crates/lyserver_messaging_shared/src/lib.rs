use anyhow::bail;
use serde::{de::Error, Deserialize, Serialize};
use serde_cbor::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LYServerMessageEvent {
    pub event_id: String,
    pub event_type: String,
    pub event_target: LYServerMessageEventTarget,
    pub event_sender: LYServerMessageEventTarget,
    pub data: Vec<u8>,
}

impl LYServerMessageEvent {
    pub fn data(&self) -> Vec<u8> {
        self.data.clone()
    }

    pub fn data_as<T: for<'de> serde::Deserialize<'de>>(&self) -> Result<T, serde_cbor::Error> {
        let value = serde_cbor::from_slice::<serde_cbor::value::Value>(&self.data)?;
        serde_cbor::value::from_value(value)
    }

    pub fn reply<T: serde::Serialize>(&self, event_type: impl Into<String>, sender: LYServerMessageEventTarget, data: T) -> Result<Self, serde_cbor::Error> {
        Ok(Self {
            event_id: self.event_id.clone(),
            event_type: event_type.into(),
            event_target: self.event_sender.clone(),
            event_sender: sender,
            data: serde_cbor::to_vec(&data)?,
        })
    }
}

impl TryFrom<Value> for LYServerMessageEvent {
    type Error = anyhow::Error;

    fn try_from(value: Value) -> anyhow::Result<Self> {
        let arr = match value {
            Value::Array(arr) => arr,
            _ => bail!("Expected CBOR array with 4 elements"),
        };

        let data = match &arr[0] {
            Value::Bytes(b) => b.clone(),
            _ => bail!("Expected event_data as bytes"),
        };

        let event_id = match &arr[1] {
            Value::Text(s) => s.clone(),
            _ => bail!("Expected event_id as text"),
        };

        let event_type = match &arr[2] {
            Value::Text(s) => s.clone(),
            _ => bail!("Expected event_type as text"),
        };

        let event_sender = match &arr[3] {
            Value::Text(s) => s.clone(),
            _ => bail!("Expected event_sender as text"),
        };

        let event_target = match &arr[4] {
            Value::Text(s) => s.clone(),
            _ => bail!("Expected event_target as text"),
        };

        Ok(LYServerMessageEvent {
            event_id,
            event_type,
            event_sender: LYServerMessageEventTarget::from(event_sender),
            event_target: LYServerMessageEventTarget::from(event_target),
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
    pub fn new<T: Into<String>, U: Into<LYServerMessageEventTarget>, V: serde::Serialize>(
        event_type: T,
        target: U,
        sender: U,
        data: V,
    ) -> Self {
        let event_id = lyserver_random_id::generate();

        let event = Self {
            event_id,
            event_type: event_type.into(),
            event_target: target.into(),
            event_sender: sender.into(),
            data: serde_cbor::to_vec(&data).expect("Failed to serialize event data"),
        };

        event
    }
}