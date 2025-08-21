# Snake Online - Rust TCP Raw Implementation

Um jogo da cobrinha multiplayer implementado em **Rust puro** usando apenas TCP raw, sem bibliotecas como Tokio. Este projeto é focado em aprender os conceitos básicos do Rust.

## 🎯 Objetivos de Aprendizado

- **TCP Raw**: Manipulação direta de sockets TCP
- **Threading Manual**: Uso de `std::thread` em vez de async/await
- **Memory Management**: Ownership, borrowing e lifetimes
- **Concorrência**: `Arc<Mutex<T>>` para compartilhar estado
- **WebSocket Manual**: Implementação básica do protocolo WebSocket
- **Pattern Matching**: Uso extensivo de `match` e `enum`

## 🏗️ Arquitetura

### Estruturas de Dados Principais

```rust
pub struct GameState {
    pub snake: Snake,
    pub food: Food,
    pub score: u32,
    pub game_over: bool,
    pub width: i32,
    pub height: i32,
}

pub struct GameServer {
    pub games: Arc<Mutex<HashMap<String, GameState>>>,
}
```

### Fluxo de Execução

1. **Servidor TCP** escuta na porta 8080
2. **Thread por Cliente**: Cada conexão ganha uma thread dedicada
3. **Detecção de Protocolo**: HTTP ou WebSocket baseado no cabeçalho
4. **Game Loop**: Atualiza estado do jogo a cada 200ms
5. **Sincronização**: `Arc<Mutex<>>` compartilha estado entre threads

## 🔧 Tecnologias Usadas

- **Rust std::net**: TcpListener e TcpStream
- **std::thread**: Threading manual
- **std::sync**: Arc, Mutex para concorrência
- **serde**: Serialização JSON (única dependência externa)

## 🚀 Como Executar

```bash
# Compile e execute o servidor
cd snake_online
cargo run

# Abra no navegador
http://127.0.0.1:8080
```

## 🎮 Como Jogar

1. Clique em **"Conectar"** para se conectar ao servidor
2. Use as **setas do teclado** ou botões na tela para controlar a cobra
3. Colete a comida vermelha para crescer e aumentar a pontuação
4. Evite colidir com as paredes ou com seu próprio corpo
5. Clique em **"Novo Jogo"** para recomeçar

## 🧠 Conceitos Rust Demonstrados

### 1. Ownership e Borrowing
```rust
// Move ownership para a thread
thread::spawn(move || {
    if let Err(e) = server_clone.handle_client(stream) {
        println!("Erro: {}", e);
    }
});
```

### 2. Pattern Matching
```rust
match client_msg {
    ClientMessage::Input { direction } => {
        game.handle_input(direction);
    }
    ClientMessage::JoinGame => {
        // Lógica para entrar no jogo
    }
    ClientMessage::Ping => {
        // Responder pong
    }
}
```

### 3. Shared State com Arc<Mutex<T>>
```rust
pub struct GameServer {
    pub games: Arc<Mutex<HashMap<String, GameState>>>,
}

// Uso em múltiplas threads
let mut games = self.games.lock().unwrap();
games.insert(client_id.clone(), GameState::new(20, 15));
```

### 4. Error Handling com Result<T, E>
```rust
fn handle_client(&self, stream: TcpStream) -> Result<(), Box<dyn std::error::Error>> {
    // Operações que podem falhar
    let bytes_read = stream.read(&mut buffer)?; // ? propaga erro
    Ok(())
}
```

### 5. Enums com Dados
```rust
#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ClientMessage {
    JoinGame,
    Input { direction: Direction },
    Ping,
}
```

## 🔍 Diferenças do Projeto WebSocket Anterior

| Aspecto | Projeto Anterior (Tokio) | Este Projeto (Raw) |
|---------|-------------------------|-------------------|
| **Runtime** | `tokio::main` | `fn main()` |
| **Concorrência** | `tokio::spawn()` tasks | `std::thread::spawn()` |
| **Estado Compartilhado** | `DashMap` | `Arc<Mutex<HashMap>>` |
| **WebSocket** | `tokio-tungstenite` | Implementação manual |
| **I/O** | Async/await | Blocking I/O |
| **Performance** | Alta (event loop) | Boa (thread per client) |

## 🎯 Lições de Rust

### 1. **Ownership é Power**
- Rust previne data races em **compile time**
- `Arc<T>` permite compartilhar ownership
- `Mutex<T>` garante acesso mutuamente exclusivo

### 2. **Pattern Matching é Expressivo**
- `match` força você a tratar todos os casos
- `if let` é conveniente para um único pattern
- Enums podem carregar dados diferentes

### 3. **Error Handling é Explícito**
- `Result<T, E>` força tratamento de erros
- `?` operator simplifica propagação
- `unwrap()` só para quando tem certeza

### 4. **Zero Cost Abstractions**
- Generics são resolvidos em compile time
- Traits são dispatch estático por padrão
- Nenhum overhead de runtime

## 🔧 Melhorias Possíveis

- **Autenticação**: Login de usuários
- **Salas Múltiplas**: Vários jogos simultâneos
- **Spectator Mode**: Assistir outros jogando
- **Ranking**: Melhores pontuações
- **Power-ups**: Itens especiais no jogo
- **WebSocket Completo**: Implementar ping/pong automático

## 🐛 Limitações Conhecidas

- WebSocket implementation é simplificada
- Não trata fragmentação de mensagens
- Geração de comida é pseudo-aleatória básica
- Um jogo por cliente (não há salas)

## 📚 Recursos para Aprender Mais

- [The Rust Book](https://doc.rust-lang.org/book/)
- [Rust by Example](https://doc.rust-lang.org/rust-by-example/)
- [Rustlings](https://github.com/rust-lang/rustlings) - Exercícios práticos
- [WebSocket RFC 6455](https://tools.ietf.org/html/rfc6455) - Protocolo WebSocket
