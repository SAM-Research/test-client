use denim_client::denim_client;
use sam_net::tls::create_tls_client_config;
mod data;
mod denim_client;
mod dispatch;

#[tokio::main]
async fn main() {
    env_logger::init();
    let _ = rustls::crypto::ring::default_provider().install_default();
    let tls = create_tls_client_config("./root.crt", None).expect("can create tls");
    let client = denim_client()
        .address("127.0.0.1:4443".to_string())
        .buffer_size(10)
        .tls(tls)
        .username("magnus".to_string())
        .upload_count(5)
        .call()
        .await
        .expect("can create client");
}
