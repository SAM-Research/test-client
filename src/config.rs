use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DenimClientConfig {
    pub address: String,
    pub dispatch_address: String,
    pub certificate_path: Option<String>,

    pub channel_buffer_size: Option<usize>,

    pub logging: Option<String>,
}
