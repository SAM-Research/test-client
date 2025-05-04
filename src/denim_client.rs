use bon::builder;
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
    net::HttpClientConfig,
    storage::{SqliteStoreConfig, error::DatabaseError, sqlite::sqlite_connector::SqliteConnector},
};

#[derive(Debug, Display, Error, From)]
pub enum DenimClientCreationError {
    Database(DatabaseError),
    Buffer(DenimBufferError),
    Client(DenimClientError),
}

#[builder]
pub async fn denim_client(
    address: String,
    username: String,
    buffer_size: usize,
    tls: Option<ClientConfig>,
    upload_count: usize,
) -> Result<DenimClient<SqliteDenimClientType>, DenimClientCreationError> {
    let store_url = format!("sqlite://{}_sam.sql?mode=rwc", username);
    let denim_store_url = format!("sqlite://{}_denim.sql?mode=rwc", username);

    let sam_conn = SqliteConnector::migrate(&store_url).await?;
    let denim_conn = SqliteConnector::migrate(&denim_store_url).await?;

    let store = SqliteStoreConfig::new(sam_conn, buffer_size);
    let denim_store = SqliteDeniableStoreConfig::new(denim_conn, buffer_size);

    let send_buffer = InMemorySendingBuffer::new(0.0)?;
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

    Ok(DenimClient::from_registration()
        .username(&username)
        .device_name(&format!("{}#device", username))
        .store_config(store)
        .deniable_store_config(denim_store)
        .api_client_config(http)
        .protocol_config(ws)
        .message_queue_config(InMemoryMessageQueueConfig::default())
        .upload_prekey_count(upload_count)
        .call()
        .await?)
}
