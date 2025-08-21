use snake_online::GameServer;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let server = GameServer::new();
    server.run("127.0.0.1:8080")?;
    Ok(())
}
