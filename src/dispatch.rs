use crate::data::{AccountInfo, ClientInfo, ClientReport, StartInfo};
use derive_more::{Display, Error, From};

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
    pub fn new(url: String) -> Result<Self, SamDispatchError> {
        Ok(Self {
            url: format!("http://{}", url),
            client: reqwest::Client::builder().cookie_store(true).build()?,
        })
    }

    pub async fn health(&self) -> bool {
        let res = self.client.get(format!("{}/health", self.url)).send().await;
        match res {
            Ok(r) => r.status().is_success(),
            Err(_) => false,
        }
    }

    pub async fn get_client(&self) -> Result<ClientInfo, SamDispatchError> {
        let res = self
            .client
            .get(format!("{}/client", self.url))
            .send()
            .await?;

        Ok(res.json().await?)
    }

    pub async fn sync(&self) -> Result<StartInfo, SamDispatchError> {
        let res = self.client.get(format!("{}/sync", self.url)).send().await?;
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

    pub async fn upload_account_id(&self, account_id: AccountInfo) -> Result<(), SamDispatchError> {
        let json_val = serde_json::to_string(&account_id)?;
        let res = self
            .client
            .post(format!("{}/id", self.url))
            .body(json_val)
            .send()
            .await?;
        if res.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(SamDispatchError::Unauthorized);
        }
        Ok(())
    }
}
