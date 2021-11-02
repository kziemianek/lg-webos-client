use serde::Serialize;
use serde_json::{json, Value};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandRequest {
    id: u8,
    r#type: String,
    uri: String,
    payload: Option<Value>,
}

pub enum Command {
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
}

pub struct CommandResponse {
    pub id: u8,
    pub payload: Option<Value>,
}

pub fn create_command(id: u8, cmd: &Command) -> Option<CommandRequest> {
    match cmd {
        Command::CreateToast(text) => Some(CommandRequest {
            id,
            r#type: String::from("request"),
            uri: String::from("ssap://system.notifications/createToast"),
            payload: Some(json!({ "message": text })),
        }),
        Command::OpenBrowser(url) => Some(CommandRequest {
            id,
            r#type: String::from("request"),
            uri: String::from("ssap://system.launcher/open"),
            payload: Some(json!({ "target": url })),
        }),
        Command::TurnOff => Some(CommandRequest {
            id,
            r#type: String::from("request"),
            uri: String::from("ssap://system/turnOff"),
            payload: None,
        }),
        Command::SetChannel(channel_id) => Some(CommandRequest {
            id,
            r#type: String::from("request"),
            uri: String::from("ssap://tv/openChannel"),
            payload: Some(json!({ "channelId": channel_id })),
        }),
        Command::SetInput(input_id) => Some(CommandRequest {
            id,
            r#type: String::from("request"),
            uri: String::from("ssap://tv/switchInput"),
            payload: Some(json!({ "inputId": input_id })),
        }),
        Command::SetMute(mute) => Some(CommandRequest {
            id,
            r#type: String::from("request"),
            uri: String::from("ssap://audio/setMute"),
            payload: Some(json!({ "mute": mute })),
        }),
        Command::SetVolume(volume) => Some(CommandRequest {
            id,
            r#type: String::from("request"),
            uri: String::from("ssap://audio/setVolume"),
            payload: Some(json!({ "volume": volume })),
        }),
        Command::GetChannelList => Some(CommandRequest {
            id,
            r#type: String::from("request"),
            uri: String::from("ssap://tv/getChannelList"),
            payload: None,
        }),
        Command::GetCurrentChannel => Some(CommandRequest {
            id,
            r#type: String::from("request"),
            uri: String::from("ssap://tv/getCurrentChannel"),
            payload: None,
        }),
        Command::OpenChannel(channel_id) => Some(CommandRequest {
            id,
            r#type: String::from("request"),
            uri: String::from("ssap://tv/openChannel"),
            payload: Some(json!({ "channelId": channel_id })),
        }),
        Command::GetExternalInputList => Some(CommandRequest {
            id,
            r#type: String::from("request"),
            uri: String::from("ssap://tv/getExternalInputList"),
            payload: None,
        }),
        Command::SwitchInput(input_id) => Some(CommandRequest {
            id,
            r#type: String::from("request"),
            uri: String::from("ssap://tv/switchInput"),
            payload: Some(json!({ "inputId": input_id })),
        }),
        Command::IsMuted => Some(CommandRequest {
            id,
            r#type: String::from("request"),
            uri: String::from("ssap://audio/getStatus"),
            payload: None,
        }),
        Command::GetVolume => Some(CommandRequest {
            id,
            r#type: String::from("request"),
            uri: String::from("ssap://audio/getVolume"),
            payload: None,
        }),
        Command::PlayMedia => Some(CommandRequest {
            id,
            r#type: String::from("request"),
            uri: String::from("ssap://media.controls/play"),
            payload: None,
        }),
        Command::StopMedia => Some(CommandRequest {
            id,
            r#type: String::from("request"),
            uri: String::from("ssap://media.controls/stop"),
            payload: None,
        }),
        Command::PauseMedia => Some(CommandRequest {
            id,
            r#type: String::from("request"),
            uri: String::from("ssap://media.controls/pause"),
            payload: None,
        }),
        Command::RewindMedia => Some(CommandRequest {
            id,
            r#type: String::from("request"),
            uri: String::from("ssap://media.controls/rewind"),
            payload: None,
        }),
        Command::ForwardMedia => Some(CommandRequest {
            id,
            r#type: String::from("request"),
            uri: String::from("ssap://media.controls/fastForward"),
            payload: None,
        }),
        Command::ChannelUp => Some(CommandRequest {
            id,
            r#type: String::from("request"),
            uri: String::from("ssap://tv/channelUp"),
            payload: None,
        }),
        Command::ChannelDown => Some(CommandRequest {
            id,
            r#type: String::from("request"),
            uri: String::from("ssap://tv/channelDown"),
            payload: None,
        }),
        Command::Turn3DOn => Some(CommandRequest {
            id,
            r#type: String::from("request"),
            uri: String::from("ssap://com.webos.service.tv.display/set3DOn"),
            payload: None,
        }),
        Command::Turn3DOff => Some(CommandRequest {
            id,
            r#type: String::from("request"),
            uri: String::from("ssap://com.webos.service.tv.display/set3DOff"),
            payload: None,
        }),
        Command::GetServicesList => Some(CommandRequest {
            id,
            r#type: String::from("request"),
            uri: String::from("ssap://com.webos.service.update/getCurrentSWInformation"),
            payload: None,
        }),
        Command::Launch(app_id, params) => Some(CommandRequest {
            id,
            r#type: String::from("request"),
            uri: String::from("ssap://system.launcher/launch"),
            payload: Some(json!({ "id": app_id, "params": params })),
        }),
    }
}
