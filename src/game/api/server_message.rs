
use crate::game::*;
use serde::{Deserialize, Serialize};


#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ServerMessage {
    #[serde(rename = "game_state")]
    GameState(GameState),
    #[serde(rename = "game_over")]
    GameOver {client_id: String},
    #[serde(rename = "error")]
    Error { message: String },
    #[serde(rename = "pong")]
    Pong,
    #[serde(rename = "connected")]
    Connected { client_id: String },
    #[serde(rename = "highscores")]
    HighScores { highscores: Vec<HighScoreEntry> },
    #[serde(rename = "rooms")]
    LobbyList { lobby_list: Vec<(String, Size, usize)> },
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