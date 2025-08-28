use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::sleep;

use crate::game::GameState;
use crate::protocol::{ServerMessage, WebSocketFrame};
use std::collections::HashMap;
use std::time::{Duration, Instant};


// ============================================================================
// SERVIDOR DE JOGOS ASSÍNCRONO
// ============================================================================

pub struct GameServer {
    games: HashMap<String, GameState>, // client_id -> gamestate
    clients: HashMap<String, ClientConnection>, // client_id -> client_connection
    last_game_update: Instant,
    game_speed: Duration,
}

pub struct ClientConnection {
    id: String,
    stream: TcpStream,
    last_update: Instant,
}
impl ClientConnection {
    pub fn new(id: &str, stream: TcpStream ) -> Self {
        ClientConnection { id: id.to_string(), stream: stream, last_update: std::time::Instant::now() }
    }
}


impl GameServer {
    pub fn new() -> Self {
        GameServer {
            games: HashMap::new(),
            clients: HashMap::new(),
            last_game_update: Instant::now(),
            game_speed: Duration::from_millis(200),
        }
    }

    pub async fn run(&mut self, address: &str) {
        let tcp_listener = tokio::net::TcpListener::bind(address).await.expect(format!("Error binding to {address}").as_str());
        let mut game_timer = tokio::time::interval(Duration::from_millis(200));
        let (mut client_tx, mut client_rx) = tokio::sync::mpsc::channel::<(String, String)>(512);
        let mut ticks : u64 = 0;
        loop {
            tokio::select! {
                _ = game_timer.tick() => {
                    println!("🎮 Game update #{ticks}");
                    ticks  += 1;
                    for game in self.games.values_mut() {
                        game.update();
                    }
                }
        
                
                // Aceitando novas conexões
                result = tcp_listener.accept() => {
                    match (result) {
                        Ok((tcp_stream,addr)) => {
                            println!("New connection from {addr}");
                            self.clients.insert(
                                addr.to_string(),
                                ClientConnection::new(addr.to_string().as_str(), tcp_stream)
                            );
                        },
                        Err(err) => println!("Error on tcp_listener: {err}"),
                    }
                }

                // Recebendo dados de conexões existentes:
                clientinput = self.any_client_input() => {
                    match (clientinput) {
                        Some((clientid,clientpayload)) => {
                            println!("{clientid} has sent: {clientpayload}")
                        },
                        None => {},
                    }
                }
            }
        }
    }

    // returns (client_id, payload)
    async fn any_client_input(&mut self) -> Option<(String, String)> {
        let mut buff : [u8;1024] = [0;1024];
        let mut ret: Option<(String, String)> = None;
        let mut client_ids_to_remove = Vec::new();
        for (key, val) in self.clients.iter() {
            match val.stream.try_read(&mut buff) {
                Ok(amount) => {
                    if amount > 0 {
                        match String::from_utf8(buff[0..amount].to_vec()) {
                            Ok(string) => {
                                ret = Some((key.clone(), string));
                                break;
                            },
                            Err(_) => {
                                buff = [0;1024]; // resetting buffer
                                continue
                            },
                        }
                    } else {
                        println!("Client '{key}' has disconnected");
                        client_ids_to_remove.push(key.clone());
                        continue;
                    }
                },
                Err(_) => continue,
            }
        }
        for clientid in client_ids_to_remove.iter() {
            self.clients.remove(clientid);
        }
        match (ret) {
            Some(r) => Some(r),
            None => {
                sleep(Duration::from_millis(4)).await;
                return ret;
            }
        }
    }

    fn send_all_game_states(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        
        let mut client_ids_with_message = Vec::new();
        {
            for it in self.clients.iter() {
                if self.games.contains_key(it.0) {
                    client_ids_with_message.push(
                        (
                            it.0.clone(),
                            ServerMessage::game_state(self.games.get(it.0).unwrap().clone())
                        )
                    );
                }
            }
        }
        for (client_id, msg) in client_ids_with_message {
            self.send_message(&client_id, &msg);
        }
        Ok(())
    }

    fn send_message(&mut self, client_id: &str, message: &ServerMessage) -> Result<(), Box<dyn std::error::Error>> {
        let json = serde_json::to_string(message)?;
        let frame = WebSocketFrame::new(&json);
        let client = self.clients.get_mut(client_id).unwrap();
        &mut client.stream.write_all(&frame.to_websocket());
        Ok(())
    }

    fn handle_http_request(&self, stream: &mut TcpStream, request: &str) -> Result<(), Box<dyn std::error::Error>> {
        let first_line = request.lines().next().unwrap_or("Requisição inválida");
        println!("Requisição HTTP: {}", first_line);

        match self.route_http_request(request) {
            HttpResponse::Html(content) => {
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\n\r\n{}",
                    content.len(),
                    content
                );
                stream.write_all(response.as_bytes());
            }
            HttpResponse::NotFound(message) => {
                let response = format!(
                    "HTTP/1.1 404 Not Found\r\nContent-Type: text/plain\r\n\r\n{}",
                    message
                );
                stream.write_all(response.as_bytes());
            }
        }

        Ok(())
    }

    fn route_http_request(&self, request: &str) -> HttpResponse {
        if request.starts_with("GET / ") || request.starts_with("GET /index.html") {
            match std::fs::read_to_string("client.html") {
                Ok(content) => HttpResponse::Html(content),
                Err(_) => HttpResponse::NotFound("404 - Arquivo client.html não encontrado".to_string()),
            }
        } else if request.starts_with("GET /.well-known/") {
            HttpResponse::NotFound("404 - Resource not found".to_string())
        } else if request.starts_with("GET /favicon.ico") {
            HttpResponse::NotFound("404 - Favicon not found".to_string())
        } else {
            HttpResponse::NotFound("404 - Página não encontrada".to_string())
        }
    }
}

enum HttpResponse {
    Html(String),
    NotFound(String),
}