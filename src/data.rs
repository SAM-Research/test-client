use std::collections::HashMap;

use sam_common::AccountId;
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
    pub client_type: ClientType,
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
pub enum ClientType {
    Denim,
    Sam,
    #[serde(other)]
    Other,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "lowercase")]
pub enum MessageType {
    Denim,
    Regular,
    #[serde(other)]
    Other,
}

#[derive(Serialize, Deserialize, Clone, bon::Builder, Debug)]
#[serde(rename_all = "camelCase")]
pub struct MessageLog {
    #[serde(rename = "type")]
    pub r#type: MessageType,
    pub from: String,
    pub to: String,
    pub size: usize,
    pub tick: u32,
}

#[derive(Serialize, Deserialize, Clone, bon::Builder, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ClientReport {
    pub start_time: u64,
    pub messages: Vec<MessageLog>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct StartInfo {
    pub friends: HashMap<String, AccountId>,
}

#[derive(Serialize, Deserialize, Clone, bon::Builder, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AccountInfo {
    pub account_id: AccountId,
}

pub struct DispatchData {
    pub client: ClientInfo,
    pub start: StartInfo,
}

impl DispatchData {
    pub fn new(client: ClientInfo, start: StartInfo) -> Self {
        Self { client, start }
    }
}
