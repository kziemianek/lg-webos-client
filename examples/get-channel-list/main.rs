use lg_webos_client::client::WebosClient;
use lg_webos_client::command::Command;

#[tokio::main]
async fn main() {
    env_logger::init();
    let mut client = WebosClient::new("ws://192.168.1.62:3000/").await.unwrap();
    let resp = client.send_command(Command::GetChannelList).await.unwrap();
    println!("Got response {:?}", resp.payload);
}
