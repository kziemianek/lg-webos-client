use futures::channel::oneshot;
use futures::channel::oneshot::{Receiver, Sender};
use futures::{Sink, Stream, StreamExt};
use futures_util::lock::Mutex;
use futures_util::stream::SplitSink;
use futures_util::{
    future::{join_all, ready},
    SinkExt,
};
use log::debug;
use serde_json::{json, Value};
use std::{collections::HashMap, sync::Arc};
use tokio_tungstenite::tungstenite::Error;
use tokio_tungstenite::{
    connect_async, tungstenite::protocol::Message, MaybeTlsStream, WebSocketStream,
};

use super::command::{create_command, Command, CommandResponse};
use crate::command::CommandRequest;

use serde::{Deserialize, Serialize};

/// Client for interacting with TV
pub struct WebosClient<T> {
    write: Box<Mutex<T>>,
    next_command_id: Arc<Mutex<u64>>,
    callbacks: Arc<Mutex<HashMap<String, Sender<CommandResponse>>>>,
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
        let command_id_generator = Arc::from(Mutex::from(0));
        let callbacks: Arc<Mutex<HashMap<String, Sender<CommandResponse>>>> =
            Arc::from(Mutex::from(HashMap::new()));
        let callbacks_copy = callbacks.clone();
        let (sender, receiver) = oneshot::channel::<CommandResponse>();
        tokio::spawn(async move { process_messages_from_server(stream, callbacks_copy).await });
        let mut handshake = get_handshake();
        // Check to see if the config has a key, if it does, add it to the handshake.
        if let Some(key) = config.key {
            handshake["payload"]["client-key"] = Value::from(key);
        }
        let registration_id = 0.to_string();
        handshake["id"] = Value::from(registration_id.to_string());
        callbacks.lock().await.insert(registration_id, sender);
        let formatted_handshake = format!("{handshake}");
        sink.send(Message::text(formatted_handshake))
            .await
            .map_err(|_| ClientError::CommandSendError)?;
        let key = Some(receiver.await.unwrap().payload.unwrap().to_string());
        Ok(WebosClient {
            write: Box::new(Mutex::new(sink)),
            next_command_id: command_id_generator,
            callbacks,
            key,
        })
    }
    /// Sends single command and waits for response
    pub async fn send_command(&self, cmd: Command) -> Result<CommandResponse, ClientError> {
        let (message, promise) = self
            .prepare_command_to_send(cmd)
            .await
            .map_err(|_| ClientError::CommandSendError)?;
        self.write
            .lock()
            .await
            .send(message)
            .await
            .map_err(|_| ClientError::CommandSendError)?;
        promise.await.map_err(|_| ClientError::CommandSendError)
    }

    /// Sends a special luna command and waits for response
    pub async fn send_luna_command(
        &self,
        luna_uri: &'static str,
        params: Value,
    ) -> Result<CommandResponse, ClientError> {
        // https://github.com/chros73/bscpylgtv/blob/master/bscpylgtv/webos_client.py#L1098
        // n.b. this is a hack which abuses the alert API
        // to call the internal luna API which is otherwise
        // not exposed through the websocket interface
        // An important limitation is that any returned
        // data is not accessible

        // set desired action for click, fail and close
        // for redundancy/robustness

        let luna_uri = format!("luna://{}", luna_uri);

        let buttons = vec![json!({
            "label": "",
            "onClick": luna_uri,
            "params": params
        })];

        let payload = json!({
            "message": " ",
            "buttons": buttons,
            "onclose": {"uri": luna_uri, "params": params},
            "onfail": {"uri": luna_uri, "params": params},
        });

        let alert = self.send_command(Command::CreateAlert(payload)).await?;

        let alert_id = alert.payload.map(|v| {
            let str = v["alertId"].as_str().unwrap();
            // We first need to convert to str before calling to_string else this does not parse properly.
            str.to_string()
        });

        if let Some(alert_id) = alert_id {
            self.send_command(Command::CloseAlert(alert_id)).await
        } else {
            Err(ClientError::CommandSendError)
        }
    }

    async fn prepare_command_to_send(
        &self,
        cmd: Command,
    ) -> Result<(Message, Receiver<CommandResponse>), ()> {
        let id = self.generate_next_id().await;
        let (sender, receiver) = oneshot::channel::<CommandResponse>();

        if let Some(mut lock) = self.callbacks.try_lock() {
            lock.insert(id.clone(), sender);
            let message = Message::from(&create_command(id, cmd));
            Ok((message, receiver))
        } else {
            Err(())
        }
    }

    async fn generate_next_id(&self) -> String {
        let mut guard = self.next_command_id.lock().await;
        *guard += 1;
        guard.to_string()
    }
}

async fn process_messages_from_server<T>(
    stream: T,
    pending_requests: Arc<Mutex<HashMap<String, Sender<CommandResponse>>>>,
) where
    T: Stream<Item = Result<Message, Error>> + Send,
{
    stream
        .for_each(|message| match message {
            Ok(_message) => {
                if let Ok(text_message) = _message.into_text() {
                    if let Ok(json) = serde_json::from_str::<Value>(&text_message) {
                        if let Some(r) = json["id"].as_str() {
                            // there is one response after pairing prompt which we need to skip
                            if json["payload"]["pairingType"] != "PROMPT" {
                                let response = CommandResponse {
                                    id: Some(r.to_string()),
                                    payload: Some(json["payload"].clone()),
                                };
                                if let Some(mut requests) = pending_requests.try_lock() {
                                    if let Some(id) = response.id.clone() {
                                        if let Some(sender) = requests.remove(&id) {
                                            sender.send(response).unwrap();
                                        }
                                    }
                                }
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

/// Get the initial handshake packet for connecting to a device.
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
        responses: HashMap<String, Message>,
        queue: VecDeque<Message>,
    }
    impl LgDevice {
        pub fn new(responses: HashMap<String, Message>) -> Self {
            LgDevice {
                registered: false,
                responses,
                queue: VecDeque::new(),
            }
        }
    }

    impl Sink<Message> for LgDevice {
        type Error = Error;

        fn poll_ready(
            self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
        ) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }

        fn start_send(self: Pin<&mut Self>, item: Message) -> Result<(), Self::Error> {
            if let Ok(text_message) = item.into_text() {
                if let Ok(json) = serde_json::from_str::<Value>(&text_message) {
                    let id = json["id"].as_str().unwrap();
                    let mut _self = self.get_mut();
                    if let Some(response) = _self.responses.remove(id) {
                        _self.queue.push_front(response);
                    }
                }
            }

            Ok(())
        }

        fn poll_flush(
            self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
        ) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }

        fn poll_close(
            self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
        ) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }
    }

    impl Stream for LgDevice {
        type Item = Result<Message, Error>;

        fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
            cx.waker().wake_by_ref();
            if !self.registered {
                self.get_mut().registered = true;
                return Poll::Ready(Some(Ok(Message::Text(
                    r#"{
                        "id": "0",
                        "type": "registered",
                        "payload": {
                            "client-key": "key"
                        }
                    }"#
                    .to_owned(),
                ))));
            } else {
                return if let Some(message) = self.get_mut().queue.pop_front() {
                    Poll::Ready(Some(Ok(message)))
                } else {
                    Poll::Pending
                };
            }
        }
    }

    use crate::client::{WebOsClientConfig, WebosClient};
    use crate::command::Command;
    use futures_util::{Sink, Stream, StreamExt};
    use serde_json::Value;
    use std::collections::{HashMap, VecDeque};
    use std::pin::Pin;
    use std::task::{Context, Poll};
    use tokio_tungstenite::tungstenite::{Error, Message};

    #[tokio::test]
    async fn create_client() {
        let device = LgDevice::new(HashMap::new());
        let (sink, stream) = device.split();
        assert!(
            WebosClient::from_stream_and_sink(stream, sink, WebOsClientConfig::default())
                .await
                .is_ok()
        );
    }

    #[tokio::test]
    async fn send_command() {
        let mut responses = HashMap::new();
        responses.insert(
            "1".to_owned(),
            Message::Text(
                r#"
                {
                    "id": "1",
                    "payload": {
                                    "returnValue": true
                                },
                    "type":"response"
                }"#
                .to_owned(),
            ),
        );

        let device = LgDevice::new(responses);
        let (sink, stream) = device.split();
        let client = WebosClient::from_stream_and_sink(stream, sink, WebOsClientConfig::default())
            .await
            .unwrap();
        client.send_command(Command::ChannelUp).await.unwrap();
    }
}
