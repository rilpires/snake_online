// Re-exporta todos os módulos públicos da biblioteca
pub mod game;
pub mod gameserver;
pub mod protocol;
pub mod http;

// Re-exporta tipos principais para facilitar o uso
pub use game::{Direction, GameState, Position, Snake, Food};
pub use protocol::{ClientGameMessage, ServerMessage};

// Re-exporta GameServer diretamente
pub use crate::gameserver::GameServer;