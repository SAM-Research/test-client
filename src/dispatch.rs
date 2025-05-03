use derive_more::{Display, Error, From};

use crate::data::{Client, ClientReport, StartInfo};

pub struct SamDispatchClient {
    url: String,
    client: reqwest::Client,
}

#[derive(Debug, Display, Error, From)]
pub enum SamDispatchError {
    Json(serde_json::Error),
    Reqwest(reqwest::Error),
    Unauthorized,
}

impl SamDispatchClient {
    pub fn new(url: String) -> Result<Self, reqwest::Error> {
        Ok(Self {
            url: format!("http://{}", url),
            client: reqwest::Client::builder().cookie_store(true).build()?,
        })
    }

    pub async fn get_client(&self) -> Result<Client, reqwest::Error> {
        let res = self
            .client
            .get(format!("{}/client", self.url))
            .send()
            .await?;
        res.json().await
    }

    pub async fn wait_for_start(&self) -> Result<StartInfo, SamDispatchError> {
        let res = self
            .client
            .get(format!("{}/start", self.url))
            .send()
            .await?;
        if res.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(SamDispatchError::Unauthorized);
        }
        Ok(res.json().await?)
    }

    pub async fn upload_results(&self, report: ClientReport) -> Result<(), SamDispatchError> {
        let json_val = serde_json::to_string(&report)?;
        let res = self
            .client
            .post(format!("{}/upload", self.url))
            .body(json_val)
            .send()
            .await?;
        if res.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(SamDispatchError::Unauthorized);
        }
        Ok(())
    }
}
