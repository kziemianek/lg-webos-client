use futures::{Sink, Stream, StreamExt};
use futures_util::stream::SplitSink;
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
use tokio_tungstenite::tungstenite::Error;
use tokio_tungstenite::{
    connect_async, tungstenite::protocol::Message, MaybeTlsStream, WebSocketStream,
};

use super::command::{create_command, Command, CommandResponse};
use crate::command::CommandRequest;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Client for interacting with TV
pub struct WebosClient<T> {
    write: Box<RefCell<T>>,
    //todo: use RwLock instead of Mutex or ?
    ongoing_requests: Arc<Mutex<HashMap<Uuid, Pinky<CommandResponse>>>>,
    pub key: Option<String>,
}

#[derive(Debug)]
pub enum ClientError {
    MalformedUrl,
    ConnectionError,
    CommandSendError,
}

#[derive(Serialize, Deserialize)]
pub struct WebOsClientConfig {
    pub address: String,
    pub key: Option<String>,
}

impl Default for WebOsClientConfig {
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

impl WebosClient<SplitSink<WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>, Message>> {
    /// Creates client connected to device with given address
    pub async fn new(config: WebOsClientConfig) -> Result<Self, ClientError> {
        let url = url::Url::parse(&config.address).map_err(|_| ClientError::MalformedUrl)?;
        let (ws_stream, _) = connect_async(url)
            .await
            .map_err(|_| ClientError::ConnectionError)?;
        debug!("WebSocket handshake has been successfully completed");
        let (write, read) = ws_stream.split();
        WebosClient::from_stream_and_sink(read, write, config).await
    }
}

impl<T> WebosClient<T>
where
    T: Sink<Message, Error = Error> + Unpin,
{
    /// Creates client using provided stream and sink
    pub async fn from_stream_and_sink<S>(
        stream: S,
        mut sink: T,
        config: WebOsClientConfig,
    ) -> Result<Self, ClientError>
    where
        S: Stream<Item = Result<Message, Error>> + Send + 'static,
    {
        let ongoing_requests: Arc<Mutex<HashMap<Uuid, Pinky<CommandResponse>>>> =
            Arc::from(Mutex::from(HashMap::new()));
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
        let formatted_handshake = format!("{handshake}");
        sink.send(Message::text(formatted_handshake))
            .await
            .map_err(|_| ClientError::CommandSendError)?;
        let key = registration_promise.await;
        Ok(WebosClient {
            write: Box::new(RefCell::new(sink)),
            ongoing_requests,
            key,
        })
    }
    /// Sends single command and waits for response
    pub async fn send_command(self, cmd: Command) -> Result<CommandResponse, ClientError> {
        let (message, promise) = self
            .prepare_command_to_send(&cmd)
            .map_err(|_| ClientError::CommandSendError)?;
        self.write
            .try_borrow_mut()
            .map_err(|_| ClientError::CommandSendError)?
            .send(message)
            .await
            .map_err(|_| ClientError::CommandSendError)?;
        Ok(promise.await)
    }

    /// Sends multiple commands and waits for responses
    pub async fn send_all_commands(
        self,
        cmds: Vec<Command>,
    ) -> Result<Vec<CommandResponse>, ClientError> {
        let mut promises: Vec<PinkySwear<CommandResponse>> = vec![];
        let messages: Vec<Result<Message, tokio_tungstenite::tungstenite::Error>> = cmds
            .iter()
            .map(|cmd| {
                let (message, promise) = self.prepare_command_to_send(cmd).unwrap();
                promises.push(promise);
                Ok(message)
            })
            .collect();

        let mut iter = futures_util::stream::iter(messages);
        self.write
            .try_borrow_mut()
            .map_err(|_| ClientError::CommandSendError)?
            .send_all(&mut iter)
            .await
            .map_err(|_| ClientError::CommandSendError)?;
        Ok(join_all(promises).await)
    }

    fn prepare_command_to_send(
        &self,
        cmd: &Command,
    ) -> Result<(Message, PinkySwear<CommandResponse>), ()> {
        let id = Uuid::new_v4();
        let (promise, pinky) = PinkySwear::<CommandResponse>::new();

        self.ongoing_requests
            .lock()
            //todo: get rid of this unwrap
            .unwrap()
            .insert(id, pinky);
        let message = Message::from(&create_command(id, cmd));
        Ok((message, promise))
    }
}

async fn process_messages_from_server<T>(
    stream: T,
    pending_requests: Arc<Mutex<HashMap<Uuid, Pinky<CommandResponse>>>>,
    registration_pinky: Pinky<Option<String>>,
) where
    T: Stream<Item = Result<Message, Error>> + Send,
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
                        } else if let Ok(r) = serde_json::from_value::<Uuid>(json["id"].clone()) {
                            let response = CommandResponse {
                                id: Some(r),
                                payload: Some(json["payload"].clone()),
                            };
                            let requests = pending_requests.lock().unwrap();
                            requests.get(&response.id.unwrap()).unwrap().swear(response);
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

#[cfg(test)]
mod tests {

    struct LgDevice {
        registered: bool,
    }

    impl Sink<Message> for LgDevice {
        type Error = Error;

        fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }

        fn start_send(self: Pin<&mut Self>, item: Message) -> Result<(), Self::Error> {
            Ok(())
        }

        fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }

        fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }
    }

    impl Stream for LgDevice {
        type Item = Result<Message, Error>;

        fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
            if !self.registered {
                self.get_mut().registered = true;
                return Poll::Ready(Some(Ok(Message::Text(
                    r#"{
                        "type": "registered",
                        "payload": {
                            "client-key": "key"
                        }
                    }"#
                    .to_owned(),
                ))));
            }
            Poll::Pending
        }
    }

    use crate::client::{WebOsClientConfig, WebosClient};
    use futures_util::{sink, stream, Sink, SinkExt, Stream, StreamExt};
    use std::ops::{Deref, DerefMut};
    use std::pin::Pin;
    use std::task::{Context, Poll};
    use tokio_tungstenite::tungstenite::{Error, Message};

    #[tokio::test]
    async fn create_client() {
        let device = LgDevice { registered: false };
        let (sink, stream) = device.split();
        assert!(
            WebosClient::from_stream_and_sink(stream, sink, WebOsClientConfig::default())
                .await
                .is_ok()
        );
    }
}
