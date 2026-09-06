use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct RemoteUserInfo {
    pub id: u32,
    pub name: String,
    pub color_hex: String,
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "data")]
pub enum ClientMessage {
    Join {
        user_name: String,
        color_hex: String,
    },
    PointerMotion {
        x: f64,
        y: f64,
    },
    PointerButton {
        button: u32,
        pressed: bool,
    },
    KeyEvent {
        keycode: u32,
        pressed: bool,
    },
    CursorChat {
        message: String,
    },
    Emote {
        emoji: String,
    },
    Ping,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "data")]
pub enum ServerMessage {
    Welcome {
        session_id: u64,
        assigned_seat_name: String,
        user_name: String,
        user_color: String,
        canvas_width: u32,
        canvas_height: u32,
    },
    PresenceUpdate {
        active_users: Vec<RemoteUserInfo>,
    },
    CursorChatBroadcast {
        user_id: u32,
        user_name: String,
        message: String,
    },
    Pong,
}

impl ClientMessage {
    pub fn to_json_line(&self) -> Result<String, serde_json::Error> {
        let mut s = serde_json::to_string(self)?;
        s.push('\n');
        Ok(s)
    }

    pub fn from_json_line(line: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(line.trim())
    }
}

impl ServerMessage {
    pub fn to_json_line(&self) -> Result<String, serde_json::Error> {
        let mut s = serde_json::to_string(self)?;
        s.push('\n');
        Ok(s)
    }

    pub fn from_json_line(line: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(line.trim())
    }
}
