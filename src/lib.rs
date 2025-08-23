// Re-exporta todos os módulos públicos da biblioteca
pub mod game;
pub mod network;
pub mod protocol;

// Re-exporta tipos principais para facilitar o uso
pub use game::{Direction, GameState, Position, Snake, Food};
pub use protocol::{ClientMessage, ServerMessage};

// Re-exporta GameServer diretamente
pub use crate::network::GameServer;