use lg_webos_client::client::*;
use lg_webos_client::command::Command;

#[tokio::main]
async fn main() {
    env_logger::init();
    // Note: We must specify the ws protocol, and if we do not have the key, we just specify None.
    let config = WebOsClientConfig::new("ws://192.168.1.62:3000/", None);
    let client = WebosClient::new(config).await.unwrap();
    println!(
        "The key for next time you build WebOsClientConfig: {:?}",
        client.key
    );
    let resp = client.send_command(Command::GetChannelList).await.unwrap();
    println!("Got response {:?}", resp.payload);
}
