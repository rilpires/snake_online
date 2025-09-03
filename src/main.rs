use snake_online::GameServer;

#[tokio::main]
async fn main() {
    let mut server = GameServer::new();
    server.run([
        std::env::var("APP_HOST").unwrap_or("0.0.0.0".to_string()),
        std::env::var("APP_PORT").unwrap_or("8080".to_string())
    ].join(":")).await;
}