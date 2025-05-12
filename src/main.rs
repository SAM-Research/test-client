use std::{io::BufReader, time::Duration};

use clap::{Arg, Command};
use config::DenimClientConfig;
use data::{AccountInfo, DispatchData};
use derive_more::{Display, Error, From};
use dispatch::{SamDispatchClient, SamDispatchError};
use health::HealthClient;
use log::{error, info};
use sam_net::{error::ClientTlsError, tls::create_tls_client_config};
use scenario::ScenarioRunner;
use test_client::{TestClient, TestClientCreationError};

mod config;
mod data;
mod dispatch;
mod health;
mod scenario;
mod test_client;
mod timer;
mod utils;

#[derive(Debug, Display, Error, From)]
pub enum CliError {
    NoConfig,
    Serde(serde_json::Error),
    Io(std::io::Error),
    Dispatch(SamDispatchError),
    ArgumentError(#[error(not(source))] String),
    Tls(ClientTlsError),
    Creation(TestClientCreationError),
    Reqwest(reqwest::Error),
    UnknownClientType,
}

const DEFAULT_CHANNEL_BUFFER_SIZE: usize = 10;

async fn cli() -> Result<(), CliError> {
    let matches = Command::new("denim_client")
        .arg(Arg::new("config").required(true).help("Client config"))
        .get_matches();

    let config_path = matches
        .get_one::<String>("config")
        .ok_or(CliError::NoConfig)?;

    let file = std::fs::File::open(config_path)?;
    let reader = BufReader::new(file);
    let config: DenimClientConfig = serde_json::from_reader(reader)?;

    if let Some(filter) = &config.logging {
        env_logger::builder().parse_filters(filter).init();
    } else {
        env_logger::init();
    }

    let tls = if let Some(tls_path) = &config.certificate_path {
        let _ = rustls::crypto::ring::default_provider().install_default();
        Some(create_tls_client_config(&tls_path, None)?)
    } else {
        None
    };

    let dispatch = SamDispatchClient::new(config.dispatch_address)?;

    while !dispatch.health().await {
        info!("Dispatcher unavailable, trying again in 200ms...");
        tokio::time::sleep(Duration::from_millis(200)).await
    }
    info!("Dispatcher ready!");
    let client_info = dispatch.get_client().await?;

    let health = HealthClient::new(config.address.clone(), tls.clone())?;

    loop {
        let check = match health.health().await {
            Ok(check) => check,
            Err(x) => {
                error!("{}", x);
                info!("Health unavailable, trying again in 200ms...");
                tokio::time::sleep(Duration::from_millis(200)).await;
                continue;
            }
        };
        if check.is_ok() {
            break;
        }
        info!("SAM Health: {}", check.sam);
        if let Some(denim) = check.denim {
            info!("DenIM Proxy Health: {}", denim);
            info!("Database Health: {}", check.database);
            info!("Trying again in 200ms...");
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    }

    info!("SAM ready!");

    let client = match client_info.client_type {
        data::ClientType::Denim => {
            TestClient::new_denim()
                .address(config.address)
                .buffer_size(
                    config
                        .channel_buffer_size
                        .unwrap_or(DEFAULT_CHANNEL_BUFFER_SIZE),
                )
                .maybe_tls(tls)
                .username(client_info.username.clone())
                .upload_count(client_info.friends.len() + 1)
                .call()
                .await?
        }
        data::ClientType::Sam => {
            TestClient::new_sam()
                .address(config.address)
                .buffer_size(
                    config
                        .channel_buffer_size
                        .unwrap_or(DEFAULT_CHANNEL_BUFFER_SIZE),
                )
                .maybe_tls(tls)
                .username(client_info.username.clone())
                .upload_count(client_info.friends.len() + 1)
                .call()
                .await?
        }
        data::ClientType::Other => Err(CliError::UnknownClientType)?,
    };

    dispatch
        .upload_account_id(
            AccountInfo::builder()
                .account_id(client.account_id())
                .build(),
        )
        .await?;

    let start_info = dispatch.sync().await?;
    let dispatch_data = DispatchData::new(client_info, start_info);

    let runner = ScenarioRunner::new(dispatch_data, client);
    info!("Starting Scenario...");
    let report = runner.start().await;

    dispatch.upload_results(report).await?;

    Ok(())
}

#[tokio::main]
async fn main() {
    let res = cli().await;
    let _ = env_logger::try_init();
    match res {
        Ok(_) => info!("Goodbye!"),
        Err(e) => error!("Fatal Client Error: {}", e),
    }
}
