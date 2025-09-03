use std::{cmp::min, collections::HashMap, io::ErrorKind, str::FromStr};

use crate::{game::{Direction, GameState, JoinGame}, http::{HttpMethod, HttpRequest, WebSocketFrame}};
use serde::{Deserialize, Serialize};

#[derive(Debug)]
pub enum ClientMessage {
    ClientGameMessage(ClientGameMessage),
    HttpRequest(HttpRequest),
    Invalid,
    Incomplete,
    Disconnect
}

pub fn parse_client_message(payload: &mut Vec<u8>) -> ClientMessage {
    match String::from_utf8(payload.to_vec()) {
        // If it is utf8 string, it probably is http request, not websocket frame
        Ok(string) => {
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
                            payload.clear();
                            return ClientMessage::HttpRequest(ret);
                        }
                    }
                }
            }
            payload.clear();
            return ClientMessage::Invalid;
        },

        // probably, websocket
        Err(_) => match WebSocketFrame::parse(payload) {
            Ok(ws) => {
                if let Ok(string) = String::from_utf8(ws) {
                    let result : Result<ClientGameMessage,_> = serde_json::from_str(&string);
                    match result {
                        Ok(msg) => {
                            ClientMessage::ClientGameMessage(msg)
                        },
                        Err(_) => ClientMessage::Invalid
                    }
                } else {
                    // not valid utf8 websocket dataframe
                    ClientMessage::Invalid
                }
            },
            Err(e) if e.kind() == ErrorKind::Interrupted => {
                // data not fully arrived yet
                // the only kind of error after trying to parse websocket frame
                // that we dont clear the buffer
                println!("Someone is sending websocket dataframes without nagle's alg");
                ClientMessage::Incomplete
            },
            Err(e) => {
                println!("{}", e);
                // something wrong, this probably isnt a websocket frame,
                // so lets reset buffer
                payload.clear();
                ClientMessage::Invalid
            },
        },
    }

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
    #[serde(rename = "username")]
    Username { username: String},
    #[serde(rename = "ping")]
    Ping,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HighScoreEntry {
    pub username: String,
    pub score: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HighScores {
    pub highscores: HashMap<String, HighScoreEntry>
}
impl HighScores {
    pub fn from_vec(value: &mut Vec<HighScoreEntry>) -> Self {
        let mut ret = HashMap::new();
        value.sort_by(
            |a, b| {b.score.cmp(&a.score)}
        );
        for i in 0..min(10, value.len()) {
            ret.insert(
                format!("{}", i+1),
                value.get(i).unwrap().clone(),
            );
        }
        HighScores{
            highscores: ret
        }
    }
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
    #[serde(rename = "highscores")]
    HighScores (HighScores),
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
