use crate::game::GameState;
use crate::protocol::{ClientMessage, ServerMessage, WebSocketFrame, WebSocketHandler};
use std::collections::HashMap;
use std::io::{Error, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::time::{Duration, Instant};
use std::future::Future;
use std::task::{Context, Poll};
use std::pin::Pin;

// ============================================================================
// SERVIDOR DE JOGOS ASSÍNCRONO
// ============================================================================

pub struct GameServer {
    pub games: HashMap<String, GameState>, // client_id -> gamestate
    pub clients: HashMap<String, ClientConnection>, // client_id -> client_connection
}

pub struct ClientConnection {
    id: String,
    stream: TcpStream,
    last_update: Instant,
}

// Future para o servidor principal
struct AsyncGameServer<'a> {
    server: &'a mut GameServer,
    address: String,
    listener: Option<TcpListener>,
    last_game_update: Instant,
    game_speed: Duration,
}

impl GameServer {
    pub fn new() -> Self {
        GameServer {
            games: HashMap::new(),
            clients: HashMap::new(),
        }
    }

    pub fn run(&mut self, address: &str) -> impl Future<Output = Result<(), Box<dyn std::error::Error>>> + '_ {
        AsyncGameServer {
            server: self,
            address: address.to_string(),
            listener: None,
            last_game_update: Instant::now(),
            game_speed: Duration::from_millis(200),
        }
    }
}

impl<'a> Future for AsyncGameServer<'a> {
    type Output = Result<(), Box<dyn std::error::Error>>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // Inicializa o listener se ainda não foi criado
        if self.listener.is_none() {
            match TcpListener::bind(&self.address) {
                Ok(listener) => {
                    if let Err(e) = listener.set_nonblocking(true) {
                        return Poll::Ready(Err(Box::new(e)));
                    }
                    println!("Servidor Snake rodando em http://{}", self.address);
                    println!("Abra http://{} no navegador para jogar!", self.address);
                    self.listener = Some(listener);
                }
                Err(e) => return Poll::Ready(Err(Box::new(e))),
            }
        }

        // Aceita novas conexões
        let mut stream_addr_to_handle = Vec::new();
        if let Some(ref listener) = self.listener {
            loop {
                match listener.accept() {
                    Ok((stream, addr)) => {
                        println!("Novo cliente conectado de: {:?}", addr);
                        stream_addr_to_handle.push(
                            (stream, addr)
                        );
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        break; // Sem novas conexões
                    }
                    Err(e) => {
                        eprintln!("Erro ao aceitar conexão: {}", e);
                        break;
                    }
                }
            }
        }
        for stream_addr in stream_addr_to_handle {
            if let Err(e) = self.server.handle_new_client(stream_addr.0) {
                eprintln!("Erro ao processar novo cliente: {}", e);
            }
        }


        // Processa mensagens dos clientes
        if let Err(e) = self.server.process_client_messages() {
            eprintln!("Erro ao processar mensagens: {}", e);
        }

        // Atualiza jogos periodicamente
        if self.last_game_update.elapsed() >= self.game_speed {
            if let Err(e) = self.server.update_all_games() {
                return Poll::Ready(Err(e));
            }
            
            if let Err(e) = self.server.send_all_game_states() {
                eprintln!("Erro ao enviar estados do jogo: {}", e);
            }
            
            self.last_game_update = Instant::now();
        }

        // Mantém o servidor rodando
        cx.waker().wake_by_ref();
        Poll::Pending
    }
}

impl GameServer {
    fn handle_new_client(&mut self, mut stream: TcpStream) -> Result<(), Box<dyn std::error::Error>> {
        if let Err(e) = stream.set_nonblocking(true) {
            return Err(Box::new(e));
        }
        
        let mut buffer = [0; 1024];
        match stream.read(&mut buffer) {
            Ok(bytes_read) => {
                let request = String::from_utf8_lossy(&buffer[..bytes_read]);
                
                if request.contains("Upgrade: websocket") {
                    self.handle_websocket_connection(stream, &request)
                } else {
                    self.handle_http_request(&mut stream, &request)
                }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                // Adiciona à lista para processar depois
                let client_id = format!("{:?}", stream.peer_addr()?);
                let client = ClientConnection {
                    id: client_id.clone(),
                    stream,
                    last_update: Instant::now(),
                };
                
                self.clients.insert(client_id, client);
                Ok(())
            }
            Err(e) => Err(Box::new(e)),
        }
    }

    fn handle_websocket_connection(
        &mut self,
        mut stream: TcpStream,
        request: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Realiza handshake WebSocket
        if let Some(key) = WebSocketHandler::extract_key(request) {
            let accept_key = WebSocketHandler::calculate_accept_key(key);
            let response = WebSocketHandler::handshake_response(&accept_key);
            stream.write_all(response.as_bytes())?;
            println!("WebSocket handshake realizado com sucesso");
        }

        let client_id = format!("{:?}", stream.peer_addr()?);
        
        // Adiciona cliente à lista
        let client = ClientConnection {
            id: client_id.clone(),
            stream,
            last_update: Instant::now(),
        };

        // Envia mensagem de conexão bem-sucedida
        let welcome_msg = ServerMessage::connected(client_id.clone());

        self.clients.insert(client_id.to_string(), client);
        self.send_message(client_id.as_str(), &welcome_msg)?;

        Ok(())
    }

    fn process_client_messages(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let mut clients_to_remove : Vec<String> = Vec::new();
        let client_ids : Vec<String> = self.clients.keys().cloned().collect();

        for client_id in client_ids {
            let message = self.read_client_message(&client_id);
            match message {
                Ok(Some(message)) => {
                    self.handle_client_message(&client_id, &message)
                }
                Ok(None) => {
                    // Nothing
                    println!("not a valid websocket message?");
                }
                Err(err) => {
                    clients_to_remove.push(client_id.clone());
                    println!("Client {} has disconnected because error: {}", client_id, err.to_string());
                }
            }
        }
        
        for client_id in clients_to_remove {
            self.clients.remove(&client_id);
        }

        Ok(())
    }

    fn read_client_message(&mut self, client_id: &str) -> Result<Option<ClientMessage>, Box<dyn std::error::Error>> {
        let mut buffer = [0; 1024];
        let mut stream = &mut self.clients.get_mut(client_id).unwrap().stream;
        match stream.read(&mut buffer) {
            Ok(0) => Ok(None),
            Ok(bytes_read) => {
                match WebSocketFrame::parse(&buffer[..bytes_read]) {
                    Ok(str) => {
                        if str.is_empty() {
                            return Ok(None);
                        }
                        match serde_json::from_str::<ClientMessage>(str.as_str()) {
                            Ok(msg) => Ok(Some(msg)),
                            Err(err) => Err(Box::new(err))
                        }
                    },
                    Err(err) => Err(Box::new(err)),
                }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                Ok(None) // Sem dados, mas cliente ainda conectado
            }
            Err(e) => Err(Box::new(e)),
        }
    }

    fn handle_client_message(
        &mut self,
        client_id: &str,
        message: &ClientMessage,
    ) {
        match message {
            ClientMessage::Input { direction } => {
                if let Some(game) = self.games.get_mut(client_id) {
                    game.handle_input(*direction);
                }
            }
            ClientMessage::ResetGame => {
                if let Some(game) = self.games.get_mut(client_id) {
                    game.reset();
                }
            }
            ClientMessage::JoinGame => {
                self.games.insert(
                    client_id.to_string(),
                    GameState::new(32, 32),
                );
            }
            ClientMessage::Ping => {
                // Pong será enviado automaticamente
            }
        }
    }

    fn update_all_games(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        for game in self.games.values_mut() {
            game.update();
        }
        Ok(())
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
        &mut client.stream.write_all(&frame.to_websocket())?;
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
                stream.write_all(response.as_bytes())?;
            }
            HttpResponse::NotFound(message) => {
                let response = format!(
                    "HTTP/1.1 404 Not Found\r\nContent-Type: text/plain\r\n\r\n{}",
                    message
                );
                stream.write_all(response.as_bytes())?;
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