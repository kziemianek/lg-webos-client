use serde::Serialize;
use serde_json::{json, Value};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandRequest {
    id: String,
    r#type: String, // Required by lg api
    uri: String,
    payload: Option<Value>,
}

pub enum Command {
    CreateAlert(Value),
    CloseAlert(String),
    CreateToast(String),
    OpenBrowser(String),
    TurnOff,
    SetChannel(String),
    SetInput(String),
    SetMute(bool),
    SetVolume(i8),
    GetChannelList,
    GetCurrentChannel,
    OpenChannel(String),
    GetExternalInputList,
    SwitchInput(String),
    IsMuted,
    GetVolume,
    PlayMedia,
    StopMedia,
    PauseMedia,
    RewindMedia,
    ForwardMedia,
    ChannelUp,
    ChannelDown,
    Turn3DOn,
    Turn3DOff,
    GetServicesList,
    Launch(String, Value),
    GetAudioOutputs,
}

#[derive(Debug)]
pub struct CommandResponse {
    pub id: Option<String>,
    pub payload: Option<Value>,
}

pub fn create_command(id: String, cmd: Command) -> CommandRequest {
    match cmd {
        Command::CreateAlert(payload) => CommandRequest {
            id,
            r#type: String::from("request"),
            uri: String::from("ssap://system.notifications/createAlert"),
            payload: Some(payload),
        },
        Command::CloseAlert(alert_id) => CommandRequest {
            id,
            r#type: String::from("request"),
            uri: String::from("ssap://system.notifications/closeAlert"),
            payload: Some(json!({"alertId": alert_id})),
        },
        Command::CreateToast(text) => CommandRequest {
            id,
            r#type: String::from("request"),
            uri: String::from("ssap://system.notifications/createToast"),
            payload: Some(json!({ "message": text })),
        },
        Command::OpenBrowser(url) => CommandRequest {
            id,
            r#type: String::from("request"),
            uri: String::from("ssap://system.launcher/open"),
            payload: Some(json!({ "target": url })),
        },
        Command::TurnOff => CommandRequest {
            id,
            r#type: String::from("request"),
            uri: String::from("ssap://system/turnOff"),
            payload: None,
        },
        Command::SetChannel(channel_id) => CommandRequest {
            id,
            r#type: String::from("request"),
            uri: String::from("ssap://tv/openChannel"),
            payload: Some(json!({ "channelId": channel_id })),
        },
        Command::SetInput(input_id) => CommandRequest {
            id,
            r#type: String::from("request"),
            uri: String::from("ssap://tv/switchInput"),
            payload: Some(json!({ "inputId": input_id })),
        },
        Command::SetMute(mute) => CommandRequest {
            id,
            r#type: String::from("request"),
            uri: String::from("ssap://audio/setMute"),
            payload: Some(json!({ "mute": mute })),
        },
        Command::SetVolume(volume) => CommandRequest {
            id,
            r#type: String::from("request"),
            uri: String::from("ssap://audio/setVolume"),
            payload: Some(json!({ "volume": volume })),
        },
        Command::GetChannelList => CommandRequest {
            id,
            r#type: String::from("request"),
            uri: String::from("ssap://tv/getChannelList"),
            payload: None,
        },
        Command::GetCurrentChannel => CommandRequest {
            id,
            r#type: String::from("request"),
            uri: String::from("ssap://tv/getCurrentChannel"),
            payload: None,
        },
        Command::OpenChannel(channel_id) => CommandRequest {
            id,
            r#type: String::from("request"),
            uri: String::from("ssap://tv/openChannel"),
            payload: Some(json!({ "channelId": channel_id })),
        },
        Command::GetExternalInputList => CommandRequest {
            id,
            r#type: String::from("request"),
            uri: String::from("ssap://tv/getExternalInputList"),
            payload: None,
        },
        Command::SwitchInput(input_id) => CommandRequest {
            id,
            r#type: String::from("request"),
            uri: String::from("ssap://tv/switchInput"),
            payload: Some(json!({ "inputId": input_id })),
        },
        Command::IsMuted => CommandRequest {
            id,
            r#type: String::from("request"),
            uri: String::from("ssap://audio/getStatus"),
            payload: None,
        },
        Command::GetVolume => CommandRequest {
            id,
            r#type: String::from("request"),
            uri: String::from("ssap://audio/getVolume"),
            payload: None,
        },
        Command::PlayMedia => CommandRequest {
            id,
            r#type: String::from("request"),
            uri: String::from("ssap://media.controls/play"),
            payload: None,
        },
        Command::StopMedia => CommandRequest {
            id,
            r#type: String::from("request"),
            uri: String::from("ssap://media.controls/stop"),
            payload: None,
        },
        Command::PauseMedia => CommandRequest {
            id,
            r#type: String::from("request"),
            uri: String::from("ssap://media.controls/pause"),
            payload: None,
        },
        Command::RewindMedia => CommandRequest {
            id,
            r#type: String::from("request"),
            uri: String::from("ssap://media.controls/rewind"),
            payload: None,
        },
        Command::ForwardMedia => CommandRequest {
            id,
            r#type: String::from("request"),
            uri: String::from("ssap://media.controls/fastForward"),
            payload: None,
        },
        Command::ChannelUp => CommandRequest {
            id,
            r#type: String::from("request"),
            uri: String::from("ssap://tv/channelUp"),
            payload: None,
        },
        Command::ChannelDown => CommandRequest {
            id,
            r#type: String::from("request"),
            uri: String::from("ssap://tv/channelDown"),
            payload: None,
        },
        Command::Turn3DOn => CommandRequest {
            id,
            r#type: String::from("request"),
            uri: String::from("ssap://com.webos.service.tv.display/set3DOn"),
            payload: None,
        },
        Command::Turn3DOff => CommandRequest {
            id,
            r#type: String::from("request"),
            uri: String::from("ssap://com.webos.service.tv.display/set3DOff"),
            payload: None,
        },
        Command::GetServicesList => CommandRequest {
            id,
            r#type: String::from("request"),
            uri: String::from("ssap://com.webos.service.update/getCurrentSWInformation"),
            payload: None,
        },
        Command::Launch(app_id, params) => CommandRequest {
            id,
            r#type: String::from("request"),
            uri: String::from("ssap://system.launcher/launch"),
            payload: Some(json!({ "id": app_id, "params": params })),
        },
        Command::GetAudioOutputs => CommandRequest {
            id,
            r#type: String::from("request"),
            uri: String::from("ssap://audio/getSoundOutput"),
            payload: None,
        },
    }
}
