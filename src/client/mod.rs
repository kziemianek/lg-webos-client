use futures::{
    stream::{SplitSink, SplitStream},
    Stream, StreamExt,
};
use futures_util::{
    future::{join_all, ready},
    SinkExt,
};
use log::debug;
use pinky_swear::{Pinky, PinkySwear};
use serde_json::Value;
use std::{
    cell::RefCell,
    collections::HashMap,
    sync::{Arc, Mutex},
};
use tokio::net::TcpStream;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use tokio_tungstenite::{tungstenite::Error, MaybeTlsStream, WebSocketStream};

use super::command::{create_command, Command, CommandResponse};
use crate::command::CommandRequest;

use serde::{Deserialize, Serialize};

/// Client for interacting with TV
pub struct WebosClient {
    write: RefCell<SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>>,
    next_command_id: Arc<Mutex<u8>>,
    ongoing_requests: Arc<Mutex<HashMap<u8, Pinky<CommandResponse>>>>,
    pub key: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct WebOsClientConfig {
    pub address: String,
    pub key: Option<String>,
}

impl ::std::default::Default for WebOsClientConfig {
    fn default() -> Self {
        WebOsClientConfig::new("ws://lgwebostv:3000/", None)
    }
}

impl WebOsClientConfig {
    /// Creates a new client configuration
    pub fn new(addr: &str, key: Option<String>) -> WebOsClientConfig {
        let address = String::from(addr);
        WebOsClientConfig { address, key }
    }
}

impl Clone for WebOsClientConfig {
    fn clone(&self) -> Self {
        let addr = self.address.clone();
        let key = self.key.clone();
        WebOsClientConfig { address: addr, key }
    }
}

impl WebosClient {
    /// Creates client connected to device with given address
    pub async fn new(config: WebOsClientConfig) -> Result<WebosClient, String> {
        let url = url::Url::parse(&config.address).expect("Could not parse given address");
        let (ws_stream, _) = connect_async(url).await.expect("Failed to connect");
        debug!("WebSocket handshake has been successfully completed");
        let (write, read) = ws_stream.split();
        WebosClient::from_stream_and_sink(read, write, config).await
    }

    /// Creates client using provided stream and sink
    pub async fn from_stream_and_sink(
        stream: SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>,
        mut sink: SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>,
        config: WebOsClientConfig,
    ) -> Result<WebosClient, String> {
        let next_command_id = Arc::from(Mutex::from(0));
        let ongoing_requests = Arc::from(Mutex::from(HashMap::new()));
        let requests_to_process = ongoing_requests.clone();
        let (registration_promise, registration_pinky) = PinkySwear::<Option<String>>::new();
        tokio::spawn(async move {
            process_messages_from_server(stream, requests_to_process, registration_pinky).await
        });

        let mut handshake = get_handshake();
        // Check to see if the config has a key, if it does, add it to the handshake.
        if let Some(key) = config.key {
            handshake["payload"]["client-key"] = Value::from(key);
        }
        let formatted_handshake = format!("{}", handshake);
        sink.send(Message::text(formatted_handshake)).await.unwrap();
        let key = registration_promise.await;
        Ok(WebosClient {
            write: RefCell::new(sink),
            next_command_id,
            ongoing_requests,
            key,
        })
    }
    /// Sends single command and waits for response
    pub async fn send_command(self, cmd: Command) -> Result<CommandResponse, String> {
        let (message, promise) = self.prepare_command_to_send(&cmd);
        self.write.borrow_mut().send(message).await.unwrap();
        Ok(promise.await)
    }

    /// Sends mutliple commands and waits for responses
    pub async fn send_all_commands(
        &mut self,
        cmds: Vec<Command>,
    ) -> Result<Vec<CommandResponse>, String> {
        let mut promises: Vec<PinkySwear<CommandResponse>> = vec![];
        let messages: Vec<Result<Message, tokio_tungstenite::tungstenite::Error>> = cmds
            .iter()
            .map(|cmd| {
                let (message, promise) = self.prepare_command_to_send(cmd);
                promises.push(promise);
                Result::Ok(message)
            })
            .collect();

        let mut iter = futures_util::stream::iter(messages);
        self.write.borrow_mut().send_all(&mut iter).await.unwrap();
        Result::Ok(join_all(promises).await)
    }

    fn prepare_command_to_send(&self, cmd: &Command) -> (Message, PinkySwear<CommandResponse>) {
        let next_command_id = self.generate_next_id();
        let (promise, pinky) = PinkySwear::<CommandResponse>::new();

        self.ongoing_requests
            .lock()
            .unwrap()
            .insert(next_command_id, pinky);
        let message = Message::from(&create_command(next_command_id, cmd).unwrap());
        (message, promise)
    }

    fn generate_next_id(&self) -> u8 {
        let mut guard = self
            .next_command_id
            .lock()
            .expect("Could not lock next_command_id");
        *guard += 1;
        *guard
    }
}

async fn process_messages_from_server<T>(
    stream: T,
    pending_requests: Arc<Mutex<HashMap<u8, Pinky<CommandResponse>>>>,
    registration_pinky: Pinky<Option<String>>,
) where
    T: Stream<Item = Result<Message, Error>>,
{
    stream
        .for_each(|message| match message {
            Ok(_message) => {
                if let Ok(text_message) = _message.into_text() {
                    if let Ok(json) = serde_json::from_str::<Value>(&text_message) {
                        debug!("JSON Response: {}", json);
                        if json["type"] == "registered" {
                            let key = json
                                .get("payload")
                                .and_then(|p| p.get("client-key"))
                                .and_then(|k| k.as_str())
                                .map(Into::into);
                            registration_pinky.swear(key);
                        } else {
                            let mut error: bool = false;
                            let res = match json["id"].as_i64() {
                                Some(r) => r,
                                None => {
                                    error = true;
                                    0
                                }
                            };
                            if !error {
                                let response = CommandResponse {
                                    id: res as u8,
                                    payload: Some(json["payload"].clone()),
                                };
                                let requests = pending_requests.lock().unwrap();
                                requests.get(&response.id).unwrap().swear(response);
                            }
                        }
                    }
                }
                ready(())
            }
            Err(_) => ready(()),
        })
        .await
}

impl From<&CommandRequest> for Message {
    fn from(request: &CommandRequest) -> Self {
        Message::text(serde_json::to_string(request).unwrap())
    }
}

/// Get the initial handhsake packet for connecting to a device.
/// A client-key can be set by something similar to
/// `get_handshake()["payload"]["client-key"] = ...`
/// # Return
/// The initial handshake packet needed to connect to a WebOS device.
fn get_handshake() -> serde_json::Value {
    serde_json::from_str(include_str!("../handshake.json")).expect("Could not parse handshake json")
}
