use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Friend {
    pub username: String,
    pub frequency: f64,
    pub denim: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ClientInfo {
    pub username: String,
    pub message_size_range: (u32, u32),
    pub send_rate: u32,
    pub tick_millis: u32,
    pub duration_ticks: u32,
    pub denim_probability: f32,
    pub friends: HashMap<String, Friend>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "lowercase")]
pub enum MessageType {
    Denim,
    Regular,
    Status,
    #[serde(other)]
    Other,
}

#[derive(Serialize, Deserialize, Clone, bon::Builder, Debug)]
#[serde(rename_all = "camelCase")]
pub struct MessageLog {
    #[serde(rename = "type")]
    pub r#type: MessageType,
    pub to: String,
    pub from: String,
    pub size: u32,
    pub timestamp: u64,
}

#[derive(Serialize, Deserialize, Clone, bon::Builder, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ClientReport {
    pub websocket_port: u16,
    pub messages: Vec<MessageLog>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct StartInfo {
    pub epoch: u64,
}
