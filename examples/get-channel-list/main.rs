use lg_webos_client::{Command, WebosClient};
use std::time::Duration;

#[tokio::main]
async fn main() {
    env_logger::init();
    let mut client = WebosClient::new("ws://192.168.1.62:3000/").await.unwrap();

    // wait for registration...
    std::thread::sleep(Duration::from_millis(3000));
    let resp = client
        .send_command(Command::GetChannelList)
        .await
        .unwrap()
        .await;
    println!("Got response {:?}", resp.payload);
}
