use crate::game::{Direction, GameState};
use serde::{Deserialize, Serialize};

// ============================================================================
// MENSAGENS DO PROTOCOLO DE COMUNICAÇÃO
// ============================================================================

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ClientMessage {
    #[serde(rename = "join_game")]
    JoinGame,
    #[serde(rename = "input")]
    Input { direction: Direction },
    #[serde(rename = "reset_game")]
    ResetGame,
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

// ============================================================================
// FRAME WEBSOCKET SIMPLES
// ============================================================================

pub struct WebSocketFrame {
    pub payload: Vec<u8>,
}

impl WebSocketFrame {
    pub fn new(data: &str) -> Self {
        Self {
            payload: data.as_bytes().to_vec(),
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut frame = Vec::new();

        // Frame simples: FIN=1, opcode=1 (text), sem mask
        frame.push(0x81);

        if self.payload.len() < 126 {
            frame.push(self.payload.len() as u8);
        } else {
            // Para mensagens maiores
            frame.push(126);
            frame.extend_from_slice(&(self.payload.len() as u16).to_be_bytes());
        }

        frame.extend_from_slice(&self.payload);
        frame
    }

    pub fn parse(data: &[u8]) -> Option<String> {
        if data.len() < 2 {
            return None;
        }

        let payload_len = data[1] & 0x7F;
        if payload_len as usize + 2 > data.len() {
            return None;
        }

        let payload = &data[2..2 + payload_len as usize];
        String::from_utf8(payload.to_vec()).ok()
    }
}

// ============================================================================
// HANDLER DE WEBSOCKET
// ============================================================================

pub struct WebSocketHandler;

impl WebSocketHandler {
    pub fn extract_key(request: &str) -> Option<&str> {
        request
            .lines()
            .find(|line| line.starts_with("Sec-WebSocket-Key:"))
            .and_then(|line| line.split(':').nth(1))
            .map(|key| key.trim())
    }

    pub fn calculate_accept_key(key: &str) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        // Versão simplificada - em produção usaria SHA-1 + base64
        let mut hasher = DefaultHasher::new();
        format!("{}{}", key, "258EAFA5-E914-47DA-95CA-C5AB0DC85B11").hash(&mut hasher);
        format!("{:x}", hasher.finish())
    }

    pub fn handshake_response(accept_key: &str) -> String {
        format!(
            "HTTP/1.1 101 Switching Protocols\r\n\
             Upgrade: websocket\r\n\
             Connection: Upgrade\r\n\
             Sec-WebSocket-Accept: {}\r\n\
             \r\n",
            accept_key
        )
    }
}
