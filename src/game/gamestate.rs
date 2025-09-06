use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::game::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameState {
    pub snake: Snake,
    pub food: Food,
    pub score: i32,
    pub game_over: bool,
    pub width: i32,
    pub height: i32,
    pub interval: u16, // milliseconds

    #[serde(skip_serializing)]
    pub already_sent_gameovers_to : HashSet<String>,
}

impl GameState {
    pub fn new(width: i32, height: i32) -> Self {
        let mut game = GameState {
            snake: Snake::new(width as i32 / 2, height as i32 / 2),
            food: Food::new(Position::new(0, 0)),
            score: 0,
            game_over: false,
            width,
            height,
            interval: 1500,
            already_sent_gameovers_to: HashSet::new(),
        };
        game.spawn_food();
        game
    }

    pub fn spawn_food(&mut self) {
        let mut x = ((self.score * 7 + 3) % (self.width as i32)) as i32;
        let mut y = ((self.score * 11 + 5) % (self.height as i32)) as i32;
        
        let snake_positions: HashSet<Position> = self.snake.body.iter().cloned().collect();
        
        for _ in 0..100 {
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

    pub fn update(&mut self) {
        if self.game_over {
            return;
        }

        self.snake.move_forward();

        // Verifica colisões
        if self.snake.is_colliding_with_walls(self.width, self.height) 
            || self.snake.check_self_collision() {
            self.game_over = true;
            return;
        }

        // Verifica se comeu a comida
        if self.snake.head() == self.food.position {
            self.snake.grow();
            self.score += 10;
            self.spawn_food();
        }
    }

    pub fn handle_input(&mut self, direction: Direction) {
        if !self.game_over {
            self.snake.change_direction(direction);
        }
    }

    pub fn reset(&mut self) {
        *self = Self::new(self.width, self.height);
    }
}
