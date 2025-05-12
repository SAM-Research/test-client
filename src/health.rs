use rustls::ClientConfig;

use crate::data::HealthCheck;

pub struct HealthClient {
    url: String,
    client: reqwest::Client,
}

impl HealthClient {
    pub fn new(url: String, tls: Option<ClientConfig>) -> Result<Self, reqwest::Error> {
        let (scheme, client) = if let Some(cfg) = tls {
            (
                "https",
                reqwest::Client::builder()
                    .use_preconfigured_tls(cfg)
                    .build()?,
            )
        } else {
            ("http", reqwest::Client::new())
        };

        Ok(Self {
            url: format!("{scheme}://{}", url),
            client: client,
        })
    }

    pub async fn health(&self) -> Result<HealthCheck, reqwest::Error> {
        let res = self
            .client
            .get(format!("{}/health", self.url))
            .send()
            .await?;
        res.json().await
    }
}
