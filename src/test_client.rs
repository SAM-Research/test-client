use bon::bon;
use denim_sam_client::{
    DenimClient, DenimClientError, client::SqliteDenimClientType,
    message::queue::InMemoryMessageQueueConfig, protocol::DenimProtocolClientConfig,
    store::sqlite::SqliteDeniableStoreConfig,
};
use denim_sam_common::{
    DenimBufferError,
    buffers::{InMemoryReceivingBuffer, InMemorySendingBuffer},
};
use derive_more::{Display, Error, From};
use rustls::ClientConfig;
use sam_client::{
    Client, ClientError,
    client::SqliteClientType,
    encryption::DecryptedEnvelope,
    net::{HttpClientConfig, protocol::WebSocketProtocolClientConfig},
    storage::{SqliteStoreConfig, error::DatabaseError, sqlite::sqlite_connector::SqliteConnector},
};
use sam_common::AccountId;
use tokio::sync::broadcast::Receiver;

#[derive(Debug, Display, Error, From)]
pub enum TestClientError {
    Sam(ClientError),
    Denim(DenimClientError),
}

#[derive(Debug, Display, Error, From)]
pub enum TestClientCreationError {
    Sam(SamClientCreationError),
    Denim(DenimClientCreationError),
}

#[derive(Debug, Display, Error, From)]
pub enum SamClientCreationError {
    Database(DatabaseError),
    Client(ClientError),
}

#[derive(Debug, Display, Error, From)]
pub enum DenimClientCreationError {
    Database(DatabaseError),
    Buffer(DenimBufferError),
    Client(DenimClientError),
}

pub enum TestClient {
    Sam(Client<SqliteClientType>),
    Denim(DenimClient<SqliteDenimClientType>),
}

#[bon]
impl TestClient {
    #[builder]
    pub async fn new_sam(
        address: String,
        username: String,
        buffer_size: usize,
        tls: Option<ClientConfig>,
        upload_count: usize,
        inmemory: bool,
    ) -> Result<Self, TestClientCreationError> {
        let store_url = if inmemory {
            "sqlite::memory:".to_string()
        } else {
            format!("sqlite://{}_sam.sql?mode=rwc", username)
        };
        let sam_conn = SqliteConnector::migrate(&store_url)
            .await
            .map_err(SamClientCreationError::Database)?;

        let store = SqliteStoreConfig::new(sam_conn, buffer_size);

        let (http, ws) = match &tls {
            Some(config) => (
                HttpClientConfig::new_with_tls(address.clone(), config.clone()),
                WebSocketProtocolClientConfig::new_with_tls(address, config.clone(), buffer_size),
            ),
            None => (
                HttpClientConfig::new(address.clone()),
                WebSocketProtocolClientConfig::new(address, buffer_size),
            ),
        };

        Ok(Self::Sam(
            Client::from_registration()
                .username(&username)
                .device_name(&format!("{}#device", username))
                .store_config(store)
                .api_client_config(http)
                .protocol_config(ws)
                .upload_prekey_count(upload_count)
                .call()
                .await
                .map_err(SamClientCreationError::Client)?,
        ))
    }

    #[builder]
    pub async fn new_denim(
        address: String,
        username: String,
        buffer_size: usize,
        tls: Option<ClientConfig>,
        upload_count: usize,
        inmemory: bool,
    ) -> Result<Self, TestClientCreationError> {
        let (store_url, denim_store_url) = if inmemory {
            ("sqlite::memory:".to_string(), "sqlite::memory:".to_string())
        } else {
            (
                format!("sqlite://{}_sam.sql?mode=rwc", username),
                format!("sqlite://{}_denim.sql?mode=rwc", username),
            )
        };

        let sam_conn = SqliteConnector::migrate(&store_url)
            .await
            .map_err(DenimClientCreationError::Database)?;
        let denim_conn = SqliteConnector::migrate(&denim_store_url)
            .await
            .map_err(DenimClientCreationError::Database)?;

        let store = SqliteStoreConfig::new(sam_conn, buffer_size);
        let denim_store = SqliteDeniableStoreConfig::new(denim_conn, buffer_size);

        let send_buffer =
            InMemorySendingBuffer::new(0.0).map_err(DenimClientCreationError::Buffer)?;
        let recv_buffer = InMemoryReceivingBuffer::default();
        let (http, ws) = match &tls {
            Some(config) => (
                HttpClientConfig::new_with_tls(address.clone(), config.clone()),
                DenimProtocolClientConfig::new(address, tls, buffer_size, send_buffer, recv_buffer),
            ),
            None => (
                HttpClientConfig::new(address.clone()),
                DenimProtocolClientConfig::new(address, tls, buffer_size, send_buffer, recv_buffer),
            ),
        };

        Ok(Self::Denim(
            DenimClient::from_registration()
                .username(&username)
                .device_name(&format!("{}#device", username))
                .store_config(store)
                .deniable_store_config(denim_store)
                .api_client_config(http)
                .protocol_config(ws)
                .message_queue_config(InMemoryMessageQueueConfig::default())
                .upload_prekey_count(upload_count)
                .call()
                .await
                .map_err(DenimClientCreationError::Client)?,
        ))
    }

    pub fn is_denim(&self) -> bool {
        matches!(self, TestClient::Denim(_))
    }

    pub fn account_id(&self) -> AccountId {
        match self {
            TestClient::Sam(client) => client.account_id(),
            TestClient::Denim(denim_client) => denim_client.account_id(),
        }
    }

    pub fn regular_subscribe(&self) -> Receiver<DecryptedEnvelope> {
        match self {
            TestClient::Sam(client) => client.subscribe(),
            TestClient::Denim(denim_client) => denim_client.regular_subscribe(),
        }
    }
    pub fn deniable_subscribe(&self) -> Receiver<DecryptedEnvelope> {
        match self {
            TestClient::Sam(client) => client.subscribe(),
            TestClient::Denim(denim_client) => denim_client.deniable_subscribe(),
        }
    }

    pub async fn process_messages(&mut self) -> Result<(), TestClientError> {
        Ok(match self {
            TestClient::Sam(client) => client.process_messages().await?,
            TestClient::Denim(denim_client) => denim_client.process_messages().await?,
        })
    }

    pub async fn enqueue_message(
        &mut self,
        account_id: AccountId,
        msg: Vec<u8>,
    ) -> Result<(), TestClientError> {
        Ok(match self {
            TestClient::Sam(client) => client.send_message(account_id, msg).await?,
            TestClient::Denim(denim_client) => {
                denim_client.enqueue_message(account_id, msg).await?
            }
        })
    }

    pub async fn send_message(
        &mut self,
        account_id: AccountId,
        msg: Vec<u8>,
    ) -> Result<(), TestClientError> {
        Ok(match self {
            TestClient::Sam(client) => client.send_message(account_id, msg).await?,
            TestClient::Denim(denim_client) => denim_client.send_message(account_id, msg).await?,
        })
    }

    pub async fn disconnect(&mut self) -> Result<(), TestClientError> {
        Ok(match self {
            TestClient::Sam(client) => client.disconnect().await?,
            TestClient::Denim(denim_client) => denim_client.disconnect().await?,
        })
    }
}
