use snake_online::GameServer;

async fn mainloop() {
    println!("hello motherfuckers, this is the mainloop")
}
#[tokio::main]
async fn main() {
    let mut server = GameServer::new();
    server.run("127.0.0.1:8080".to_string()).await;
}