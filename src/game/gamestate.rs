use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::game::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameState {
    pub snakes: HashMap<String, Snake>,
    pub food: Food,
    pub score: i32,
    pub width: i32,
    pub height: i32,
    pub interval: u16, // milliseconds

    #[serde(skip_serializing)]
    pub interval_buffer: i32,
    #[serde(skip_serializing)]
    pub already_sent_gameovers_to : HashSet<String>,
    #[serde(skip_serializing)]
    pub high_scores: HashMap<String, u32> // per client_id
}

impl GameState {
    pub fn new(width: i32, height: i32) -> Self {
        let mut game = GameState {
            snakes: HashMap::new(),
            food: Food::new(Position::new(0, 0)),
            score: 0,
            width,
            height,
            interval: 1500,
            interval_buffer: 0,
            already_sent_gameovers_to: HashSet::new(),
            high_scores: HashMap::new(),
        };
        game.spawn_food();
        game
    }

    pub fn spawn_food(&mut self) {
        let mut x = ((self.score * 7 + 3) % (self.width as i32)) as i32;
        let mut y = ((self.score * 11 + 5) % (self.height as i32)) as i32;
        
        let snake_positions: HashSet<&Position> = self.snakes
            .values()
            .flat_map(|snake|snake.body.iter())
            .collect();

        for _ in 0..500 {
            let pos = Position::new(x, y);
            if !snake_positions.contains(&pos) {
                self.food.position = pos;
                return;
            }
            x = (x + 1) % (self.width as i32);
            if x == 0 {
                y = (y + 1) % (self.height as i32);
            }
        }
        
        self.food.position = Position::new(0, 0);
    }

    // returns a vector of dead client ids
    pub fn update(&mut self) -> Vec<(String, u32)> {
        let mut snake_eat : Option<String> = None;
        let mut dead_snakes = Vec::new();
        for (clientid, snake) in self.snakes.iter_mut() {
            let collided = snake.move_forward(
                Size::new(self.width as u32, self.height as u32),
                self.snakes.values().collect()
            );
    
            if collided {
                dead_snakes.push(
                    (clientid.clone(), (snake.body.len() * 10) as u32)
                );
            } else if snake.head() == self.food.position {
                snake_eat = Some(clientid.clone());
            }
        }

        for (dead_snake_id, score) in dead_snakes {
            self.snakes.remove(&dead_snake_id);
        }

        if let Some(client_id) = snake_eat {
            if self.snakes.contains_key(&client_id) {
                self.snakes.get_mut(&client_id).unwrap().grow();
                self.score += 10;
                self.spawn_food();
            }
        }
        return dead_snakes;
    }

    pub fn handle_input(&mut self, client_id: &String, direction: Direction) {
        if self.snakes.contains_key(client_id) {
            let snake = self.snakes.get_mut(client_id).unwrap();
            snake.change_direction(direction);
        }
    }

    pub fn reset(&mut self) {
        *self = Self::new(self.width, self.height);
    }
}
