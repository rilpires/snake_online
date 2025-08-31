use std::{collections::HashMap, str::FromStr};

use crate::{game::{Direction, GameState, JoinGame}, http::{HttpMethod, HttpRequest, WebSocketFrame}};
use serde::{Deserialize, Serialize};

#[derive(Debug)]
pub enum ClientMessage {
    ClientGameMessage(ClientGameMessage),
    HttpRequest(HttpRequest),
    Invalid,
    Disconnect
}

pub fn parse_client_message(payload: &[u8]) -> ClientMessage {
    if let Ok(string) = String::from_utf8(payload.to_vec()) {
        if let Ok(httpmethod) = HttpMethod::from_str(&string) {
            if let Some((_, b)) = string.split_once(' ') {
                if let Some((path, b)) = b.split_once(' ') {
                    if let Some((http_version, b)) = b.split_once("\r\n") {
                        let mut ret = HttpRequest {
                            method: httpmethod,
                            version: http_version.to_string(),
                            path: path.to_string(),
                            headers: HashMap::new(),
                        };
                        let (headers, _) = b.split_once("\r\n\r\n").unwrap_or_default();
                        for part in headers.split("\r\n") {
                            if let Some((k,v)) = part.split_once(": ") {
                                ret.headers.insert(k.to_string(), v.to_string());
                            }
                        }
                        return ClientMessage::HttpRequest(ret);
                    }
                }
            }
        }
    } else {
        // probably websocket frame
        if let Ok(ws) = WebSocketFrame::parse(payload) {
            if let Ok(string) = String::from_utf8(ws) {
                let result : Result<ClientGameMessage,_> = serde_json::from_str(&string);
                match result {
                    Ok(msg) => {
                        return ClientMessage::ClientGameMessage(msg);
                    },
                    Err(_) => {
                        //
                    }
                }
            }
        }
    }
    return ClientMessage::Invalid
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ClientGameMessage {
    #[serde(rename = "join_game")]
    JoinGame(JoinGame),
    #[serde(rename = "input")]
    Input { direction: Direction },
    #[serde(rename = "reset_game")]
    ResetGame,
    #[serde(rename = "set_speed")]
    SetSpeed { interval: u16},
    #[serde(rename = "ping")]
    Ping,
}



#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ServerMessage {
    #[serde(rename = "game_state")]
    GameState(GameState),
    #[serde(rename = "error")]
    Error { message: String },
    #[serde(rename = "pong")]
    Pong,
    #[serde(rename = "connected")]
    Connected { client_id: String },
}

impl ServerMessage {
    pub fn error(message: &str) -> Self {
        ServerMessage::Error {
            message: message.to_string(),
        }
    }

    pub fn game_state(state: GameState) -> Self {
        ServerMessage::GameState(state)
    }

    pub fn connected(client_id: String) -> Self {
        ServerMessage::Connected { client_id }
    }
}
