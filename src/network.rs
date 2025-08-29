use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::sleep;

use crate::game::GameState;
use crate::protocol::{parse_client_message, ServerMessage, WebSocketFrame, *};
use std::collections::HashMap;
use std::{fs, slice};
use std::hash::Hash;
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
        let mut ticks : u64 = 0;
        loop {
            tokio::select! {
                _ = game_timer.tick() => {
                    println!("🎮 Game update #{ticks}");
                    ticks  += 1;
                    for game in self.games.values_mut() {
                        game.update();
                    }
                    let messages_to_send: Vec<(String, ServerMessage)> = self.clients
                        .keys()
                        .filter_map(|client_id| {
                            self.games.get(client_id).map(|game| {
                                (client_id.clone(), ServerMessage::game_state(game.clone()))
                            })
                        })
                        .collect();
                    for (client_id, message) in messages_to_send {
                        if let Err(e) = self.send_websocket_response(&client_id, &message).await {
                            eprintln!("Failed to send to {}: {}", client_id, e);
                        }
                    }
                }
        
                
                // Aceitando novas conexões
                result = tcp_listener.accept() => {
                    match result {
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
                    match clientinput {
                        Some((clientid, clientmsg)) => {
                            match clientmsg {
                                ClientMessage::ClientGameMessage(client_game_message) => todo!(),
                                ClientMessage::HttpRequest(req) => {
                                    if req.is_websocket_handshake() {
                                        println!("{clientid} is trying to handshake me");
                                        todo!();
                                    } else {
                                        // all the proper router stuff goes here
                                        // we only have index.html kkkk
                                        if req.method == HttpMethod::GET && req.path.eq("/") {
                                            println!("Sending index.html to {}", clientid);
                                            self.send_http_response(
                                                clientid.as_str(),
                                                HttpResponse::file_content("index.html")
                                            ).await;
                                        } else {
                                            self.send_http_response(
                                                clientid.as_str(),
                                                HttpResponse::not_found()
                                            ).await;
                                        }
                                    }
                                },
                                ClientMessage::Invalid => {
                                    println!("{clientid} is sending some shit i dont understand, gotta cut this mf out");
                                    self.clients.remove(clientid.as_str());
                                },
                                ClientMessage::Disconnect => {
                                    println!("{clientid} is trying to get the hell out of here");
                                    self.clients.remove(clientid.as_str());
                                },
                            }
                        },
                        None => {},
                    }
                }
            }
        }
    }

    // returns (client_id, payload)
    async fn any_client_input(&mut self) -> Option<(String, ClientMessage)> {
        let mut buff : [u8;1024] = [0;1024];
        for (key, val) in self.clients.iter() {
            match val.stream.try_read(&mut buff) {
                Ok(amount) => {
                    if amount > 0 {
                        match String::from_utf8(buff[0..amount].to_vec()) {
                            Ok(string) => {
                                return Some((key.clone(), parse_client_message(string.as_str())));
                            },
                            Err(_) => {
                                buff = [0;1024]; // resetting buffer
                                continue
                            },
                        }
                    } else {
                        return Some((key.clone(), ClientMessage::Disconnect));
                    }
                },
                Err(_) => continue,
            }
        }
        sleep(Duration::from_millis(4)).await;
        return None;
    }


    async fn send_http_response(&mut self, client_id: &str, res: HttpResponse) {
        let client = self.clients.get_mut(client_id).unwrap();
        client.stream.write_all( res.to_string().as_bytes() ).await;
    }

    async fn send_websocket_response(&mut self, client_id: &str, message: &ServerMessage) -> Result<(), Box<dyn std::error::Error>> {
        let json = serde_json::to_string(message)?;
        let frame = WebSocketFrame::new(&json);
        let client = self.clients.get_mut(client_id).unwrap();
        &mut client.stream.write_all(&frame.to_websocket()).await;
        Ok(())
    }
}

struct HttpResponse {
    protocol_version: String,
    status_code: u16,
    status_msg: String,
    headers: HashMap<String, String>,
    body: Option<String>,
}

impl HttpResponse {
    fn default_headers() -> HashMap<String, String> {
        let mut ret = HashMap::<String, String>::new();
        ret.insert("Server".to_string(), "rust-001".to_string());
        return ret;
    }
    fn with_content_length(mut self, size: usize) -> Self {
        self.headers.remove("content-length");
        self.headers.insert("content-length".to_string(), size.to_string());
        return self;
    }
    fn with_content_type(mut self, content_type: &str) -> Self {
        self.headers.remove("content-type");
        self.headers.insert("content-type".to_string(), content_type.to_string());
        return self;
    }
    pub fn not_found() -> HttpResponse {
        HttpResponse {
            protocol_version: "HTTP/1.1".to_string(),
            status_code: 404,
            status_msg: "No shit".to_string(),
            headers: Self::default_headers(),
            body: None,
        }.with_content_length(0)
    }
    pub fn file_content(filepath: &str) -> HttpResponse {
        match fs::read(filepath) {
            Err(err) => Self::not_found(),
            Ok(payload) => {
                let len = payload.len();
                HttpResponse {
                    protocol_version: "HTTP/1.1".to_string(),
                    status_code: 200,
                    status_msg: "Take this".to_string(),
                    headers: Self::default_headers(),
                    body: Some(String::from_utf8(payload).unwrap()),
                }.with_content_length(len)
            }
        }
    }
}
impl ToString for HttpResponse {
    fn to_string(&self) -> String {
        let ret =  vec![self.protocol_version.as_str(), self.status_code.to_string().as_str(), self.status_msg.as_str()].join(" ") +
                "\r\n" +
                self.headers.iter().map(|x| format!("{}: {}", x.0, x.1) ).collect::<Vec<String>>().join("\r\n").as_str() +
                "\r\n\r\n" +
                &self.body.as_ref().unwrap_or(&String::new()).as_str();
        return ret;
    }
}

