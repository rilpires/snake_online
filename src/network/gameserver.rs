use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::tcp::OwnedWriteHalf;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};

use std::collections::{HashMap};
use std::time::Duration;

use crate::game::*;
use crate::network::*;

static MINIMUM_TICK : i32 = 50;
static MAX_HTTP_BUFFER_LEN : usize = 8192;

// ============================================================================
// SERVIDOR DE JOGOS ASSÍNCRONO
// ============================================================================

pub struct GameServer {
    games: HashMap<String, GameState>, // game_id -> gamestate
    clients: HashMap<String, ClientConnection>, // client_id -> client_connection
    tx: UnboundedSender<GameEvent>,
    rx: UnboundedReceiver<GameEvent>,
}

pub struct ClientConnection {
    pub id: String,
    game_id: Option<String>,
    websocket: bool,
    stream: OwnedWriteHalf,
    pub username: Option<String>,
    awaiting_highscores: Vec<u32>,
}
impl ClientConnection {
    pub fn new(id: &str, stream: OwnedWriteHalf ) -> Self {
        ClientConnection {
            id: id.to_string(),
            websocket: false,
            stream: stream,
            username: None,
            game_id: None,
            awaiting_highscores: vec![],
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
            println!("Web server listening from {}", address);
            loop {
                let result = tcp_listener.accept().await;
                match result {
                    Ok((tcp_stream, addr)) => {
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
                                let mut buff = [0; 2048];
                                let mut vec_buff = Vec::new();
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
                                            vec_buff.append(&mut buff[0..n].to_vec());
                                            if vec_buff.len() > MAX_HTTP_BUFFER_LEN {
                                                println!("Buffer from {} is huge (>{}), clearing it", addr.to_string(), MAX_HTTP_BUFFER_LEN);
                                                vec_buff.clear();
                                            }
                                            if n == 0 {
                                                let _ = client_tx.send(
                                                    GameEvent::ClientInput(
                                                        addr.to_string(),
                                                        ClientMessage::Disconnect,
                                                    ),
                                                );
                                                break; // to end the task
                                            } else {
                                                let parsed_input = parse_client_message(&mut vec_buff);
                                                let _ = client_tx.send(
                                                    GameEvent::ClientInput(
                                                        addr.to_string(),
                                                        parsed_input,
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
        tokio::spawn( async move {
            loop {
                game_timer.tick().await;
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
                    ClientMessage::Incomplete => {
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
                let mut messages_to_send: Vec<(String, ServerMessage)> = vec![];
                let mut dead_games : Vec<String> = vec![];
                let mut dead_clients : Vec<String> = vec![];
                let mut awaiting_scores = HashMap::new(); // client_id -> score
                for (gameid, game) in self.games.iter_mut() {
                    game.interval_buffer += MINIMUM_TICK;
                    if game.interval_buffer >= game.interval as i32 {
                        game.interval_buffer = 0;
                        let clients_in_game : Vec<&ClientConnection> = self.clients.values().filter(
                            |client| client.game_id.as_ref() == Some(gameid)
                        ).collect();

                        if clients_in_game.is_empty() {
                            dead_games.push(gameid.clone());
                            continue;
                        }

                        for (dead_clientid, score) in game.update() {
                            if self.clients.contains_key(&dead_clientid) {
                                dead_clients.push(dead_clientid.clone());
                                // sending to everyone in this game
                                for c in clients_in_game.iter() {
                                    messages_to_send.push(
                                        (
                                            c.id.clone(),
                                            ServerMessage::GameOver {
                                                client_id: dead_clientid.clone()
                                            }
                                        )
                                    )
                                }
                                let client = self.clients.get(&dead_clientid).unwrap();
                                if client.username.is_some() {
                                    store_highscore(
                                        client,
                                        score,
                                    );
                                } else {
                                    awaiting_scores.insert(dead_clientid, score);
                                }
                            }
                        }
                        // sending new state to current clietns
                        for c in clients_in_game {
                            messages_to_send.push(
                                (c.id.clone(), ServerMessage::game_state(game.clone()))
                            )
                        }

                    }
                }
                for it in awaiting_scores.iter() {
                    if let Some(client) = self.clients.get_mut(it.0) {
                        client.awaiting_highscores.push(*it.1);
                    }
                }
                for dead_game_id in dead_games {
                    self.games.remove(&dead_game_id);
                }
                for (client_id, message) in messages_to_send {
                    if let Err(e) = self.send_websocket_response(&client_id, &message).await {
                        eprintln!("Failed to send to {}: {}", client_id, e);
                    }
                }
                for dead_client_id in dead_clients {
                    if let Some(client) = self.clients.get_mut(&dead_client_id) {
                        client.game_id = None;
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
            let _ = self.send_websocket_highscores(&clientid).await;
        } else {
            // all the proper router stuff goes here
            // we only have index.html so
            if req.method == HttpMethod::GET {
                let (_, mut filepath) = req.path.split_once('/').unwrap();
                if filepath.len() == 0 {
                    filepath = "index.html";
                }
                self.send_http_response(
                    clientid.as_str(),
                    HttpResponse::file_content(
                        format!("public/{}", filepath).as_str()
                    ),
                ).await;
            } else {
                self.send_http_response(
                    clientid.as_str(),
                    HttpResponse::not_found()
                ).await;
            }
        }
    }

    async fn handle_client_game_message(&mut self, clientid: String, msg: ClientGameMessage) {
        let client = self.clients.get_mut(&clientid).unwrap();
        let mut current_game : Option<&mut GameState> = None;
        if let Some(id) = &client.game_id {
            current_game = self.games.get_mut(id);
        }
        let client_response : Option<ServerMessage> = match (current_game, msg) {
            (_, ClientGameMessage::JoinGame(joingame)) => {
                if let Some(_id) = &client.game_id {
                    // if its the same, do nothing
                    if joingame.game_id.as_ref() == Some(_id) {
                        return;
                    }

                    if let Some(game) = self.games.get_mut(_id) {
                        game.snakes.remove(&client.id);
                    }
                    client.game_id = None;
                };
                let new_game_id = match joingame.game_id {
                    Some(str) if (!str.is_empty()) => str,
                    _ => rand::random::<u64>().to_string(),
                };
                // creating game if not existent yet
                if !self.games.contains_key(&new_game_id) {
                    let mut new_game = GameState::new(
                        joingame.size.unwrap_or_default().width,
                        joingame.size.unwrap_or_default().height,
                    );
                    new_game.interval = joingame.interval.unwrap_or(1000);
                    self.games.insert(new_game_id.clone(), new_game);
                }
                let game = self.games.get_mut(&new_game_id).unwrap();
                if game.spawn_new_snake(&clientid, 3) {
                    client.game_id = Some(new_game_id.clone());
                    Some(ServerMessage::Connected { client_id: clientid.clone() })
                } else {
                    None
                }
            },
            (Some(gamestate), ClientGameMessage::Input { direction }) => {
                // match direction {
                //     Direction::Up => println!("Up"),
                //     Direction::Down => println!("Down"),
                //     Direction::Left => println!("Left"),
                //     Direction::Right => println!("Right"),
                // };
                gamestate.handle_input(&clientid, direction);
                None
            },
            (Some(gamestate), ClientGameMessage::ResetGame) => {
                println!("Resetting game for {}", clientid);
                gamestate.reset();
                None
            },

            (_, ClientGameMessage::Username { username }) => {
                if client.username.is_none() {
                    client.username = Some(username);
                    for score in &client.awaiting_highscores {
                        store_highscore(client, *score);
                    }
                }
                None
            },
            (_, ClientGameMessage::ReqLobbyList) => {
                Some(
                    ServerMessage::LobbyList {
                        lobby_list: self.games.iter().filter(
                            |(_, game)| {
                                !game.snakes.is_empty()
                            } 
                        ).map(
                            |(gameid, game)| (
                                gameid.clone(),
                                Size::new(game.width as u32, game.height as u32),
                                game.snakes.len()
                            ),
                        ).collect()
                    }
                )
            },
            (_, ClientGameMessage::Ping) => Some(ServerMessage::Pong),
            (_, _) => None,
        };
        if let Some(res) = client_response {
            let _ = self.send_websocket_response(&clientid, &res).await;
        }
    }

    async fn send_http_response(&mut self, client_id: &str, res: HttpResponse) {
        let client = self.clients.get_mut(client_id).unwrap();
        let _ = client.stream.write_all( &res.as_bytes() ).await;
    }

    async fn send_websocket_response(&mut self, client_id: &str, message: &ServerMessage) -> Result<(), Box<dyn std::error::Error>> {
        let json = serde_json::to_string(message)?;
        let frame = WebSocketFrame::to_websocket(json.as_bytes().to_vec());
        let client = self.clients.get_mut(client_id).unwrap();
        let _ = &mut client.stream.write_all(&frame).await;
        Ok(())
    }
    async fn send_websocket_highscores(&mut self, client_id: &str) -> Result<(), Box<dyn std::error::Error>> {
        let highscores = ServerMessage::HighScores{
            highscores: retrieve_top_highscore()
        }; 
        self.send_websocket_response(client_id, &highscores).await
    }
}
