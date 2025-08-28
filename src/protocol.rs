use std::io::{Error, Write};

use crate::game::{Direction, GameState};
use base64::Engine;
use serde::{Deserialize, Serialize};
use sha1::Digest;

// ============================================================================
// MENSAGENS DO PROTOCOLO DE COMUNICAÇÃO
// ============================================================================

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ClientGameMessage {
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
// FRAME WEBSOCKET, HARDCODED BABY
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

    pub fn to_websocket(self) -> Vec<u8> {
        let mut frame = Vec::new();
        frame.push(0x81);
        
        let payload_len = self.payload.len();
        
        if payload_len <= 125 {
            frame.push(payload_len as u8);
        } else if payload_len <= 65535 {
            frame.push(126);
            frame.extend_from_slice(&(payload_len as u16).to_be_bytes());
        } else {
            frame.push(127);
            frame.extend_from_slice(&(payload_len as u64).to_be_bytes());
        }
        
        frame.extend(self.payload); // ✅ Move sem clone
        frame
    }

    pub fn parse(data: &[u8]) -> Result<String, Error> {
        if data.len() < 2 {
            return Result::Err(Error::new(
                std::io::ErrorKind::InvalidData,
                "Invalid websocket frame",
            ));
        }
        if (data[0] & 0x01) == 0 {
            return Result::Err(Error::new(
                std::io::ErrorKind::ConnectionAborted,
                "Can't handle multiframe payloads yet!!!",
            ));
        }
        let mut payload_start = 2;
        let masking_bit = data[1] >> 7;
        let mut mask : u32 = 0xFFFF;
        let mut payload_len : usize = (data[1] & 0x7F).into();
        if payload_len == 126 {
            // gotta read next 2 bytes
            payload_len = u16::from_be_bytes([data[2], data[3]]).into();
            payload_start += 2;
        } else if payload_len == 127 {
            // gotta read next 8 bytes
            payload_len = u64::from_be_bytes(data[2..=9].try_into().unwrap()).try_into().unwrap();
            payload_start +=8;
        }
        if masking_bit == 1 {
            mask = u32::from_be_bytes(data[payload_start..(payload_start+4)].try_into().unwrap());
            payload_start += 4;
        }

        if data.len() != (payload_start + payload_len) {
            return Result::Err(Error::new(
                std::io::ErrorKind::InvalidData,
                "Invalid websocket frame",
            ));
        }

        let mut payloadvec : Vec<u8> = data[payload_start..(payload_start + payload_len)].to_vec();

        if masking_bit > 0 {
            payloadvec = payloadvec.iter().enumerate().map(
                |(index, byte)| byte ^ (mask.to_be_bytes())[index % 4]
            ).collect();
        }

        return String::from_utf8(payloadvec)
            .or(Result::Err(Error::new(
                std::io::ErrorKind::InvalidData,
                "Invalid UTF8 content",
            )));
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

        // Versão simplificada - em produção usaria SHA-1 + base64
        let mut hasher = sha1::Sha1::new();
        let fullstring = format!("{}{}", key, "258EAFA5-E914-47DA-95CA-C5AB0DC85B11");
        match hasher.write(fullstring.as_bytes()) {
            Ok(_result) => {
                let finished = hasher.finalize();
                let ret = base64::engine::general_purpose::STANDARD.encode(finished);
                return ret;
            },
            Err(err) => {
                println!("Error writing hash: {}", err);
                return "".to_string();
            }
        }
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
