use data::{ClientReport, MessageLog, MessageType};

use dispatch::SamDispatchClient;

mod data;
mod denim_client;
mod dispatch;

#[tokio::main]
async fn main() {
    let dispatch = SamDispatchClient::new("127.0.0.1:8080".to_string()).expect("can create client");

    let client = dispatch.get_client().await.expect("can get client");
    let start = dispatch.wait_for_start().await.expect("can wait for start");
    dispatch
        .upload_results(
            ClientReport::builder()
                .websocket_port(44444)
                .messages(vec![
                    MessageLog::builder()
                        .from("me".to_string())
                        .to("you".to_string())
                        .size(500)
                        .timestamp(12)
                        .r#type(MessageType::Regular)
                        .build(),
                ])
                .build(),
        )
        .await
        .expect("can upload report");

    println!("{:?}", client);
    println!("{:?}", start);
}
