use futures::{Stream, StreamExt};
use futures_util::{
    future::{join_all, ready},
    Sink, SinkExt,
};
use log::debug;
use pinky_swear::{Pinky, PinkySwear};
use serde_json::Value;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use tokio_tungstenite::tungstenite::Error;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};

use crate::command::CommandRequest;
use super::command::{create_command, Command, CommandResponse};

use serde::{Serialize, Deserialize};

/// Client for interacting with TV
pub struct WebosClient {
    write: Box<dyn Sink<Message, Error=Error> + Unpin>,
    next_command_id: Arc<Mutex<u8>>,
    ongoing_requests: Arc<Mutex<HashMap<u8, Pinky<CommandResponse>>>>,
}

#[derive(Serialize, Deserialize)]
pub struct WebOsClientConfig {
    pub address: String,
    pub key: String,
}

impl ::std::default::Default for WebOsClientConfig {
    fn default() -> Self {
        WebOsClientConfig::new("ws://lgwebostv:3000/", "")
    }
}

impl WebOsClientConfig {
    /// Creates a new client configuration
    pub fn new(addr: &str, the_key: &str) -> WebOsClientConfig {
        let address = String::from(addr);
        let key = String::from(the_key);
        WebOsClientConfig {
            address,
            key,
        }
    }
}

impl Clone for WebOsClientConfig {
    fn clone(&self) -> Self {
        let addr = self.address.clone();
        let key = self.key.clone();
        WebOsClientConfig {
            address: addr,
            key,
        }
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
    pub async fn from_stream_and_sink<T, S>(stream: T, mut sink: S, config: WebOsClientConfig) -> Result<WebosClient, String>
        where
            T: Stream<Item=Result<Message, Error>> + 'static + Send,
            S: Sink<Message, Error=Error> + Unpin + 'static,
    {
        let next_command_id = Arc::from(Mutex::from(0));
        let ongoing_requests = Arc::from(Mutex::from(HashMap::new()));
        let requests_to_process = ongoing_requests.clone();
        let (registration_promise, registration_pinky) = PinkySwear::<bool>::new();
        tokio::spawn(async move {
            process_messages_from_server(stream, requests_to_process, registration_pinky).await
        });

        let mut handshake = get_handshake();
        // Check to see if the config has a key, if it does, add it to the handshake.
        if config.key != "" {
            handshake["payload"]["client-key"] = Value::from(config.key);
        }
        let formatted_handshake = format!("{}", handshake);
        sink.send(Message::text(formatted_handshake)).await.unwrap();
        registration_promise.await;
        Ok(WebosClient {
            write: Box::new(sink),
            next_command_id,
            ongoing_requests,
        })
    }
    /// Sends single command and waits for response
    pub async fn send_command(&mut self, cmd: Command) -> Result<CommandResponse, String> {
        let (message, promise) = self.prepare_command_to_send(&cmd);
        self.write.send(message).await.unwrap();
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
        self.write.send_all(&mut iter).await.unwrap();
        Result::Ok(join_all(promises).await)
    }

    fn prepare_command_to_send(&mut self, cmd: &Command) -> (Message, PinkySwear<CommandResponse>) {
        let next_command_id = self.generate_next_id();
        let (promise, pinky) = PinkySwear::<CommandResponse>::new();

        self.ongoing_requests
            .lock()
            .unwrap()
            .insert(next_command_id, pinky);
        let message = Message::from(&create_command(next_command_id, cmd).unwrap());
        (message, promise)
    }

    fn generate_next_id(&mut self) -> u8 {
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
    registration_pinky: Pinky<bool>,
) where
    T: Stream<Item=Result<Message, Error>>,
{
    stream
        .for_each(|message| match message {
            Ok(_message) => {
                if let Some(text_message) = _message.clone().into_text().ok() {
                    if let Ok(json) = serde_json::from_str::<Value>(&text_message) {
                        println!("JSON Response -> {}", json);
                        if json["type"] == "registered" {
                            registration_pinky.swear(true);
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
///     The initial handshake packet needed to connect to a WebOS device.
fn get_handshake() -> serde_json::Value {
    serde_json::json!(
        {
        "type": "register",
        "id": "0",
        "payload": {
            "forcePairing": false,
            "pairingType": "PROMPT",
            "manifest": {
                "manifestVersion": 1,
                "appVersion": "1.1",
                "signed": {
                    "created": "20140509",
                    "appId": "com.lge.test",
                    "vendorId": "com.lge",
                    "localizedAppNames": {
                        "": "LG Remote App",
                        "ko-KR": "리모컨 앱",
                        "zxx-XX": "ЛГ Rэмotэ AПП"
                    },
                    "localizedVendorNames": {
                        "": "LG Electronics"
                    },
                    "permissions": [
                        "TEST_SECURE",
                        "CONTROL_INPUT_TEXT",
                        "CONTROL_MOUSE_AND_KEYBOARD",
                        "READ_INSTALLED_APPS",
                        "READ_LGE_SDX",
                        "READ_NOTIFICATIONS",
                        "SEARCH",
                        "WRITE_SETTINGS",
                        "WRITE_NOTIFICATION_ALERT",
                        "CONTROL_POWER",
                        "READ_CURRENT_CHANNEL",
                        "READ_RUNNING_APPS",
                        "READ_UPDATE_INFO",
                        "UPDATE_FROM_REMOTE_APP",
                        "READ_LGE_TV_INPUT_EVENTS",
                        "READ_TV_CURRENT_TIME"
                    ],
                    "serial": "2f930e2d2cfe083771f68e4fe7bb07"
                },
                "permissions": [
                    "LAUNCH",
                    "LAUNCH_WEBAPP",
                    "APP_TO_APP",
                    "CLOSE",
                    "TEST_OPEN",
                    "TEST_PROTECTED",
                    "CONTROL_AUDIO",
                    "CONTROL_DISPLAY",
                    "CONTROL_INPUT_JOYSTICK",
                    "CONTROL_INPUT_MEDIA_RECORDING",
                    "CONTROL_INPUT_MEDIA_PLAYBACK",
                    "CONTROL_INPUT_TV",
                    "CONTROL_POWER",
                    "READ_APP_STATUS",
                    "READ_CURRENT_CHANNEL",
                    "READ_INPUT_DEVICE_LIST",
                    "READ_NETWORK_STATE",
                    "READ_RUNNING_APPS",
                    "READ_TV_CHANNEL_LIST",
                    "WRITE_NOTIFICATION_TOAST",
                    "READ_POWER_STATE",
                    "READ_COUNTRY_INFO"
                ],
                "signatures": [
                    {
                        "signatureVersion": 1,
                        "signature": "eyJhbGdvcml0aG0iOiJSU0EtU0hBMjU2Iiwia2V5SWQiOiJ0ZXN0LXNpZ25pbmctY2VydCIsInNpZ25hdHVyZVZlcnNpb24iOjF9.hrVRgjCwXVvE2OOSpDZ58hR+59aFNwYDyjQgKk3auukd7pcegmE2CzPCa0bJ0ZsRAcKkCTJrWo5iDzNhMBWRyaMOv5zWSrthlf7G128qvIlpMT0YNY+n/FaOHE73uLrS/g7swl3/qH/BGFG2Hu4RlL48eb3lLKqTt2xKHdCs6Cd4RMfJPYnzgvI4BNrFUKsjkcu+WD4OO2A27Pq1n50cMchmcaXadJhGrOqH5YmHdOCj5NSHzJYrsW0HPlpuAx/ECMeIZYDh6RMqaFM2DXzdKX9NmmyqzJ3o/0lkk/N97gfVRLW5hA29yeAwaCViZNCP8iC9aO0q9fQojoa7NQnAtw=="
                    }
                ]
            }
        }
    })
}