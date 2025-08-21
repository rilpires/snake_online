use crate::game::GameState;
use crate::protocol::{ClientMessage, ServerMessage, WebSocketFrame, WebSocketHandler};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

// ============================================================================
// SERVIDOR DE JOGOS
// ============================================================================

pub struct GameServer {
    pub games: Arc<Mutex<HashMap<String, GameState>>>,
}

impl GameServer {
    pub fn new() -> Self {
        GameServer {
            games: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn run(&self, address: &str) -> Result<(), Box<dyn std::error::Error>> {
        let listener = TcpListener::bind(address)?;
        println!("Servidor Snake rodando em http://{}", address);
        println!("Abra http://{} no navegador para jogar!", address);

        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    let server_clone = GameServer {
                        games: Arc::clone(&self.games),
                    };

                    thread::spawn(move || {
                        if let Err(e) = server_clone.handle_client(stream) {
                            eprintln!("Erro ao processar cliente: {}", e);
                        }
                    });
                }
                Err(e) => {
                    eprintln!("Erro ao aceitar conexão: {}", e);
                }
            }
        }

        Ok(())
    }

    fn handle_client(&self, mut stream: TcpStream) -> Result<(), Box<dyn std::error::Error>> {
        println!("Cliente conectado: {:?}", stream.peer_addr()?);

        let mut buffer = [0; 1024];
        let bytes_read = stream.read(&mut buffer)?;
        let request = String::from_utf8_lossy(&buffer[..bytes_read]);

        if request.contains("Upgrade: websocket") {
            self.handle_websocket_connection(stream, &request)
        } else {
            self.handle_http_request(&mut stream, &request)
        }
    }

    fn handle_websocket_connection(
        &self,
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

        // Gerencia o cliente WebSocket
        self.handle_websocket_client(stream)
    }

    fn handle_websocket_client(&self, mut stream: TcpStream) -> Result<(), Box<dyn std::error::Error>> {
        let client_id = format!("{:?}", stream.peer_addr()?);
        
        // Cria novo jogo para o cliente
        {
            let mut games = self.games.lock().unwrap();
            games.insert(client_id.clone(), GameState::new(20, 15));
        }

        // Envia mensagem de conexão bem-sucedida
        let welcome_msg = ServerMessage::connected(client_id.clone());
        self.send_message(&mut stream, &welcome_msg)?;

        let mut last_update = Instant::now();
        let game_speed = Duration::from_millis(200);

        loop {
            // Lê mensagens do cliente (non-blocking)
            if let Some(message) = self.read_client_message(&mut stream)? {
                if !self.handle_client_message(&client_id, &message)? {
                    break; // Cliente desconectou
                }
            }

            // Atualiza jogo periodicamente
            if last_update.elapsed() >= game_speed {
                self.update_game(&client_id)?;
                self.send_game_state(&mut stream, &client_id)?;
                last_update = Instant::now();
            }

            thread::sleep(Duration::from_millis(10));
        }

        // Cleanup quando cliente desconecta
        {
            let mut games = self.games.lock().unwrap();
            games.remove(&client_id);
        }
        println!("Cliente {} desconectado", client_id);

        Ok(())
    }

    fn read_client_message(&self, stream: &mut TcpStream) -> Result<Option<String>, Box<dyn std::error::Error>> {
        stream.set_nonblocking(true)?;
        let mut buffer = [0; 1024];
        
        match stream.read(&mut buffer) {
            Ok(0) => Ok(None), // Cliente desconectou
            Ok(bytes_read) => {
                stream.set_nonblocking(false)?;
                Ok(WebSocketFrame::parse(&buffer[..bytes_read]))
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                stream.set_nonblocking(false)?;
                Ok(Some(String::new())) // Sem dados, mas cliente ainda conectado
            }
            Err(e) => {
                stream.set_nonblocking(false)?;
                Err(Box::new(e))
            }
        }
    }

    fn handle_client_message(
        &self,
        client_id: &str,
        message: &str,
    ) -> Result<bool, Box<dyn std::error::Error>> {
        if message.is_empty() {
            return Ok(true); // Mensagem vazia, mas cliente ainda conectado
        }

        match serde_json::from_str::<ClientMessage>(message) {
            Ok(client_msg) => {
                match client_msg {
                    ClientMessage::Input { direction } => {
                        let mut games = self.games.lock().unwrap();
                        if let Some(game) = games.get_mut(client_id) {
                            game.handle_input(direction);
                        }
                    }
                    ClientMessage::ResetGame => {
                        let mut games = self.games.lock().unwrap();
                        if let Some(game) = games.get_mut(client_id) {
                            game.reset();
                        }
                    }
                    ClientMessage::JoinGame => {
                        // Cliente já tem jogo criado
                    }
                    ClientMessage::Ping => {
                        // Pong será enviado automaticamente
                    }
                }
                Ok(true)
            }
            Err(e) => {
                eprintln!("Erro ao parsear mensagem de {}: {} - {}", client_id, message, e);
                Ok(true) // Continua mesmo com erro de parsing
            }
        }
    }

    fn update_game(&self, client_id: &str) -> Result<(), Box<dyn std::error::Error>> {
        let mut games = self.games.lock().unwrap();
        if let Some(game) = games.get_mut(client_id) {
            game.update();
        }
        Ok(())
    }

    fn send_game_state(&self, stream: &mut TcpStream, client_id: &str) -> Result<(), Box<dyn std::error::Error>> {
        let games = self.games.lock().unwrap();
        if let Some(game) = games.get(client_id) {
            let message = ServerMessage::game_state(game.clone());
            self.send_message(stream, &message)
        } else {
            Ok(())
        }
    }

    fn send_message(&self, stream: &mut TcpStream, message: &ServerMessage) -> Result<(), Box<dyn std::error::Error>> {
        let json = serde_json::to_string(message)?;
        let frame = WebSocketFrame::new(&json);
        stream.write_all(&frame.to_bytes())?;
        Ok(())
    }
}

// ============================================================================
// SERVIDOR HTTP SIMPLES
// ============================================================================

impl GameServer {
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
