use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::tcp::OwnedWriteHalf;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};


use crate::game::{GameState};
use crate::protocol::{parse_client_message, ServerMessage, *};
use crate::http::*;
use std::collections::{HashMap, HashSet};
use std::time::{Duration};

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
    interval_buffer : HashMap<String, i32>,
    high_scores: Vec<HighScoreEntry>,
}

pub struct ClientConnection {
    id: String,
    game_id: Option<String>,
    websocket: bool,
    stream: OwnedWriteHalf,
    username: Option<String>,
}
impl ClientConnection {
    pub fn new(id: &str, stream: OwnedWriteHalf ) -> Self {
        ClientConnection {
            id: id.to_string(),
            websocket: false,
            stream: stream,
            username: None,
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
            high_scores: Vec::new(),
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
                        let game_over = game.game_over;
                        game.update();
                        if game.game_over && !game_over {
                            // game has done now
                            // lets register high scores
                            for (_,client) in self.clients.iter() {
                                if let Some(username) = &client.username {
                                    self.high_scores.push(
                                        HighScoreEntry {
                                            username: username.to_string(),
                                            score: game.score as u32
                                        }
                                    );
                                }
                            }
                        }
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
                let messages_to_send: Vec<(String, Vec<ServerMessage>)> = self.clients
                    .iter()
                    .filter_map(|(clientid, client)| {
                        match &client.game_id {
                            Some(gameid) if updated_gameids.contains(gameid) => {
                                if let Some(gamestate) = self.games.get_mut(gameid) {
                                    if gamestate.game_over && gamestate.already_sent_gameovers_to.contains(clientid) {
                                        None
                                    } else {
                                        let mut ret = Vec::new();
                                        if gamestate.game_over {
                                            gamestate.already_sent_gameovers_to.insert(clientid.clone());
                                            ret.push(ServerMessage::HighScores(HighScores::from_vec(&mut self.high_scores)));
                                        }
                                        ret.push(ServerMessage::game_state(gamestate.clone()));
                                        Some((clientid.clone(), ret))   
                                    }
                                } else {
                                    None
                                }
                            },
                            Some(_) => None,
                            None => None,
                        }
                    })
                    .collect();
                for (client_id, messages) in messages_to_send {
                    for message in messages {
                        if let Err(e) = self.send_websocket_response(&client_id, &message).await {
                            eprintln!("Failed to send to {}: {}", client_id, e);
                        }
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
            (Some(gamestate), ClientGameMessage::Input { direction }) => {
                gamestate.handle_input(direction);
                None
            },
            (Some(gamestate), ClientGameMessage::ResetGame) => {
                println!("Resetting game for {}", clientid);
                gamestate.reset();
                None
            },
            (Some(gamestate), ClientGameMessage::SetSpeed { interval }) => {
                gamestate.interval = interval;
                None
            },
            // User may be sending username after gameover, so we can register it
            (Some(gamestate), ClientGameMessage::Username { username }) if (gamestate.game_over) && (client.username == None) => {
                if None == client.username {
                    client.username = Some(username.clone());
                    self.high_scores.push(HighScoreEntry {
                        username: username,
                        score: gamestate.score as u32,
                    });
                    Some(ServerMessage::HighScores(HighScores::from_vec(&mut self.high_scores)))
                } else {
                    None
                }
            },
            (_, ClientGameMessage::Username { username }) => {
                client.username = Some(username);
                None
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
        let highscores = ServerMessage::HighScores(HighScores::from_vec(&mut self.high_scores)); 
        println!("Sending highscores to {}", client_id);
        self.send_websocket_response(client_id, &highscores).await
    }
}
