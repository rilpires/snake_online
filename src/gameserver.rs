use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::tcp::OwnedWriteHalf;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio::time::interval;

use crate::game::GameState;
use crate::protocol::{parse_client_message, ServerMessage, *};
use crate::http::*;
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

static MINIMUM_TICK : i32 = 50;

// ============================================================================
// SERVIDOR DE JOGOS ASSÍNCRONO
// ============================================================================

pub struct GameServer {
    games: HashMap<String, GameState>, // game_id -> gamestate
    clients: HashMap<String, ClientConnection>, // client_id -> client_connection
    tx: UnboundedSender<GameEvent>,
    rx: UnboundedReceiver<GameEvent>,
    interval_buffer : HashMap<String, i32>,
}

pub struct ClientConnection {
    id: String,
    game_id: Option<String>,
    websocket: bool,
    stream: OwnedWriteHalf,
    last_update: Instant,
}
impl ClientConnection {
    pub fn new(id: &str, stream: OwnedWriteHalf ) -> Self {
        ClientConnection {
            id: id.to_string(),
            websocket: false,
            stream: stream,
            last_update: std::time::Instant::now(),
            game_id: None,
        }
    }
}

enum GameEvent {
    ClientInput(String, ClientMessage),
    NewConnection(ClientConnection),
    GameTick,
}

impl GameServer {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::unbounded_channel::<GameEvent>();
        
        GameServer {
            games: HashMap::new(),
            clients: HashMap::new(),
            tx: tx,
            rx: rx,
            interval_buffer: HashMap::new(),
        }
    }

    pub async fn run(&mut self, address: String) {
        // INPUT IO
        let input_tx = self.tx.clone();
        tokio::spawn(  async move {
            let tcp_listener = tokio::net::TcpListener::bind(address.clone())
                .await
                .expect(
                    format!("Error binding to {}", address).as_str()
                );
            loop {
                let result = tcp_listener.accept().await;
                match result {
                    Ok((mut tcp_stream, addr)) => {
                        println!("New connection from {}", addr.clone());
                        let client_tx = input_tx.clone();
                        tokio::spawn(
                            async move {
                                let (mut tcp_rx, tcp_tx) = tcp_stream.into_split();
                                let _ = client_tx.send(
                                    GameEvent::NewConnection(
                                        ClientConnection::new(
                                            addr.to_string().as_str(),
                                            tcp_tx,
                                        ),
                                    ),
                                );
                                let mut buff : [u8; 1024] = [0; 1024];
                                loop {
                                    
                                    match tcp_rx.read(&mut buff).await {
                                        Err(_) => {
                                            let _ = client_tx.send(
                                                GameEvent::ClientInput(
                                                    addr.to_string(),
                                                    ClientMessage::Invalid,
                                                ),
                                            );
                                        },
                                        Ok(n) => {
                                            if n == 0 {
                                                let _ = client_tx.send(
                                                    GameEvent::ClientInput(
                                                        addr.to_string(),
                                                        ClientMessage::Disconnect,
                                                    ),
                                                );
                                                break; // to end the task
                                            } else {
                                                let _ = client_tx.send(
                                                    GameEvent::ClientInput(
                                                        addr.to_string(),
                                                        parse_client_message(&buff[0..n]),
                                                    ),
                                                );
                                            }
                                        },
                                    }
                                    
                                }      
                            }
                        );
                    },
                    Err(err) => println!("Error on tcp_listener: {err}"),
                }
            }
        });

        // GAME UPDATE TIMER TICK
        let tick_tx = self.tx.clone();
        let mut game_timer = tokio::time::interval(Duration::from_millis(MINIMUM_TICK as u64));
        let mut ticks : u64 = 0;
        tokio::spawn( async move {
            loop {
                game_timer.tick().await;
                ticks += 1;
                let _ = tick_tx.send(GameEvent::GameTick);
            }
        });

        // Receiving events on a loop
        loop {
            while let Some(event) = self.rx.recv().await {
                self.handle_io_event(event).await;
            }
        }
        
    }

    async fn handle_io_event(&mut self, ev: GameEvent) {
        match ev {
            GameEvent::ClientInput(clientid, client_message) => {
                match client_message {
                    ClientMessage::ClientGameMessage(client_game_message) => {
                        self.handle_client_game_message(clientid, client_game_message).await;
                    },
                    ClientMessage::HttpRequest(http_request) => {
                        self.handle_client_http_request(
                            clientid, &http_request
                        ).await;
                    },
                    ClientMessage::Invalid => {
                        println!("Client {} sent an invalid message", clientid);
                    },
                    ClientMessage::Disconnect => {
                        self.clients.remove(&clientid);
                    },
                }
            },
            GameEvent::NewConnection(client_connection) => {
                self.clients.insert(
                    client_connection.id.clone(),
                    client_connection,
                );
            },
            GameEvent::GameTick => {
                let mut updated_gameids = HashSet::new();
                for (gameid, game) in self.games.iter_mut() {
                    if !self.interval_buffer.contains_key(gameid) {
                        self.interval_buffer.insert(gameid.clone(), 0);
                    }
                    self.interval_buffer
                            .entry(gameid.clone())
                            .and_modify(
                                |old| { *old -= MINIMUM_TICK }
                            ).or_insert(0);
                    if *self.interval_buffer.get(gameid).unwrap() < 0 {
                        game.update();
                        updated_gameids.insert(gameid.clone());
                        self.interval_buffer
                            .entry(gameid.clone())
                            .and_modify(
                                |old| { *old += game.interval as i32 }
                            );
                    }
                }
                self.interval_buffer.retain(
                    |k, _| { self.games.contains_key(k)}
                );
                let messages_to_send: Vec<(String, ServerMessage)> = self.clients
                    .iter()
                    .filter_map(|(clientid, client)| {
                        match &client.game_id {
                            Some(gameid) if updated_gameids.contains(gameid) => {
                                self.games
                                    .get(gameid)
                                    .map( |game| {
                                        (clientid.clone(), ServerMessage::game_state(game.clone()))
                                    })
                            },
                            Some(_) => None,
                            None => None,
                        }
                    })
                    .collect();
                for (client_id, message) in messages_to_send {
                    if let Err(e) = self.send_websocket_response(&client_id, &message).await {
                        eprintln!("Failed to send to {}: {}", client_id, e);
                    }
                }
            },
        }
    }

    async fn handle_client_http_request(&mut self, clientid: String, req: &HttpRequest) {
        let client = self.clients.get_mut(&clientid).unwrap();
        if req.is_websocket_handshake() {
            client.websocket = true;
            self.send_http_response(
                &clientid, 
                HttpResponse::websocket_handshake(req),
            ).await;
        } else {
            // all the proper router stuff goes here
            // we only have index.html so
            if req.method == HttpMethod::GET {
                let (_, mut filepath) = req.path.split_once('/').unwrap();
                if filepath.len() == 0 {
                    filepath = "index.html";
                }
                println!("Sending {} to {}", filepath, client.id);
                self.send_http_response(
                    clientid.as_str(),
                    HttpResponse::file_content(
                        format!("public/{}", filepath).as_str()
                    ),
                ).await;
            }
            self.send_http_response(
                clientid.as_str(),
                HttpResponse::not_found()
            ).await;
        }
    }

    async fn handle_client_game_message(&mut self, clientid: String, msg: ClientGameMessage) {
        let client = self.clients.get_mut(&clientid).unwrap();
        let client_response : Option<ServerMessage> = match msg {
            ClientGameMessage::JoinGame(joingame) => {
                if let Some(id) = &client.game_id {
                    // gotta leave
                    client.game_id = None;
                };
                let new_game_id = rand::random::<u64>().to_string();
                client.game_id = Some(new_game_id.clone());
                self.games.insert(
                    new_game_id,
                    GameState::new(
                        joingame.size.unwrap_or_default().width,
                        joingame.size.unwrap_or_default().height,
                    ),
                );
                Some(ServerMessage::Connected { client_id: clientid.clone() })
            },
            ClientGameMessage::Input { direction } => {
                match &client.game_id {
                    Some(game_id) => {
                        match self.games.get_mut(game_id) {
                            Some(gamestate) => {
                                gamestate.handle_input(direction);
                                None
                            },
                            None => {
                                println!("Inconsistency, no game for client with gameid {game_id}");
                                None
                            },
                        }
                    },
                    None => None,
                }
            },
            ClientGameMessage::ResetGame => match &client.game_id {
                Some(game_id) => {
                    match self.games.get_mut(game_id) {
                        Some(gamestate) => {
                            println!("Resetting game for {}", clientid);
                            gamestate.reset();
                            None
                        },
                        None => {
                            println!("Inconsistency, no game for client with gameid {game_id}");
                            None
                        },
                    }
                },
                None => None,
            },
            ClientGameMessage::SetSpeed { interval } => {
                match &client.game_id {
                    Some(game_id) => {
                        match self.games.get_mut(game_id) {
                            Some(gamestate) => {
                                gamestate.interval = interval;
                                None
                            },
                            None => {
                                println!("Inconsistency, no game for client with gameid {game_id}");
                                None
                            },
                        }
                    },
                    None => None,
                }
            },
            ClientGameMessage::Ping => Some(ServerMessage::Pong),
        };
        if let Some(res) = client_response {
            self.send_websocket_response(&clientid, &res).await;
        }
    }

    async fn send_http_response(&mut self, client_id: &str, res: HttpResponse) {
        let client = self.clients.get_mut(client_id).unwrap();
        let _ = client.stream.write_all( res.to_string().as_bytes() ).await;
    }

    async fn send_websocket_response(&mut self, client_id: &str, message: &ServerMessage) -> Result<(), Box<dyn std::error::Error>> {
        let json = serde_json::to_string(message)?;
        let frame = WebSocketFrame::to_websocket(json.as_bytes().to_vec());
        let client = self.clients.get_mut(client_id).unwrap();
        let _ = &mut client.stream.write_all(&frame).await;
        Ok(())
    }
}
