use std::{net::TcpListener, time::Duration};

use snake_online::{network::ClientConnection, GameServer};

// Função helper para criar setInterval
fn set_interval<F, Fut>(duration: Duration, mut f: F) -> tokio::task::JoinHandle<()> 
where
    F: FnMut() -> Fut + 'static + Send,
    Fut: std::future::Future<Output = ()> + Send,
{
    tokio::spawn(async move {
        loop {
            f().await;
            tokio::time::sleep(duration).await;
        }
    })
}

async fn mainloop() {
    println!("hello motherfuckers, this is the mainloop")
}
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let address = "127.0.0.1:8080";
    let mut server = GameServer::new();
    let mut games = server.games;
    let mut clients = server.clients;
    let mut tcp_listener = tokio::net::TcpListener::bind(address).await.expect(format!("Error binding to {address}").as_str());
    let mut game_timer = tokio::time::interval(Duration::from_millis(200));
    
    let mut client_futures = FuturesUnordered::new();
    
    loop {
        tokio::select! {
            _ = game_timer.tick() => {
                println!("🎮 Game update");
                for game in games.values_mut() {
                    game.update();
                }
            }
    
            
            // Aceitando novas conexões
            result = tcp_listener.accept() => {
                match (result) {
                    Ok((tcp_stream,addr)) => {
                        clients.insert(
                            addr.to_string(),
                            ClientConnection::new(addr.to_string().as_str(),tcp_stream)
                        );
                    },
                    Err(err) => println!("{err}"),
                }
            }

            // Recebendo dados de conexões existentes:
            
        }
    }
}