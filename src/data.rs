use sam_common::AccountId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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
    pub reply_rate: u32,
    pub tick_millis: u32,
    pub duration_ticks: u32,
    pub denim_probability: f32,
    pub reply_probability: f32,
    pub stale_reply: u32,
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
    pub start_time: u128,
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

#[derive(Serialize, Deserialize, Clone, bon::Builder, Debug)]
#[serde(rename_all = "camelCase")]
pub struct HealthCheck {
    pub sam: String,
    pub denim: Option<String>,
    pub database: String,
}

impl HealthCheck {
    pub fn is_ok(&self) -> bool {
        let status = vec![&self.sam, &self.database];
        let is_ok = status.iter().all(|f| *f == "OK");
        let denim = self.denim.as_ref().is_some_and(|x| x == "OK") || self.denim.as_ref().is_none();
        is_ok && denim
    }
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
