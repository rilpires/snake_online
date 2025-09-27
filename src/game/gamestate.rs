use std::collections::{HashMap, HashSet};

use rand::random_range;
use serde::{Deserialize, Serialize};

use crate::game::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameState {
    pub snakes: HashMap<String, Snake>, // per client_id
    pub food: Food,
    pub score: i32,
    pub width: i32,
    pub height: i32,
    pub interval: u16, // milliseconds

    #[serde(skip_serializing)]
    pub interval_buffer: i32,
    #[serde(skip_serializing)]
    pub already_sent_gameovers_to : HashSet<String>,
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

    //
    pub fn spawn_new_snake(&mut self, client_id: &str,  snake_size: usize) -> bool {
        // gotta find a valid position for it...
        // rules:
        // 1 - it will spawn as a straight line
        // 2 - do not collide with any food and any other snake
        // 3 - the next 4 squares do not contain any snake or wall
        // so we basiaclly should find a valid "4+snake_size" straight line to fit
        // the alghorithm is kinda simple: just randomly test up until 100 guesses
        // if even then not found, return false
        
        // self.snakes.insert(client_id.to_string(), Snake::new(2, 2));
        // return true;

        let directions = [Direction::Up, Direction::Down, Direction::Left, Direction::Right];
        let snake_positions: HashSet<Position> = self.snakes
            .values()
            .flat_map(|snake| snake.body.iter().cloned())
            .collect();

        for _ in 0..100 {
            let start_x = random_range(0..self.width);
            let start_y = random_range(0..self.height);
            let direction = directions[random_range(0..4)];
            
            let mut valid_spawn = true;
            let mut test_positions = Vec::new();
            
            for i in 0..(snake_size + 4) {
                let test_pos = match direction {
                    Direction::Up => Position::new(start_x, start_y - i as i32),
                    Direction::Down => Position::new(start_x, start_y + i as i32),
                    Direction::Left => Position::new(start_x - i as i32, start_y),
                    Direction::Right => Position::new(start_x + i as i32, start_y),
                };
                
                if test_pos.x < 0 || test_pos.x >= self.width || 
                   test_pos.y < 0 || test_pos.y >= self.height {
                    valid_spawn = false;
                    break;
                }
                
                if snake_positions.contains(&test_pos) || test_pos == self.food.position {
                    valid_spawn = false;
                    break;
                }
                
                test_positions.push(test_pos);
            }
            if valid_spawn {
                let mut snake_body = Vec::new();
                for i in 0..snake_size {
                    snake_body.push(test_positions[i]);
                }
                
                let new_snake = Snake::new(
                    direction,
                    Position::new(snake_body[snake_size-1].x, snake_body[snake_size-1].y),
                    snake_size,
                );

                self.snakes.insert(client_id.to_string(), new_snake);
                return true;
            }
        }
        return false;
    }

    // Returns true if collided
    pub fn move_snake_forward(&mut self, client_id: &str ) -> bool {
        
        let new_head : Position;
        // moving tail
        {
            let snake = self.snakes.get_mut(client_id).unwrap();
            snake.direction = snake.next_direction.unwrap_or(snake.direction);
            new_head = snake.head().move_in_direction(snake.direction);
            snake.next_direction = None;
            
            // moving tail
            if snake.grow_next {
                snake.grow_next = false;
            } else {
                snake.body.pop();
            }
        }
        
        // Now, checking collision on head
        if (new_head.x < 0) || (new_head.x >= self.width) ||
            (new_head.y < 0) || (new_head.y >= self.height) {
            return true;
        } else if self.snakes.iter().any(|(_, snake)| snake.body.contains(&new_head)) {
            return true;
        }

        // moving on head
        let snake = self.snakes.get_mut(client_id).unwrap();
        snake.body.insert(0, new_head);
        return false;
    }

    // returns a vector of dead client ids alongside its scores
    pub fn update(&mut self) -> Vec<(String, u32)> {
        let mut snake_eat : Option<String> = None;
        let mut dead_snakes = Vec::new();
        let clientids : Vec<_> = self.snakes.keys().cloned().collect();
        for clientid in clientids {
            let collided = self.move_snake_forward(&clientid);
            let snake = self.snakes.get(&clientid).unwrap();
            if collided {
                dead_snakes.push(
                    (clientid.to_string().clone(), (snake.body.len() * 10) as u32)
                );
            } else if snake.head() == self.food.position {
                snake_eat = Some(clientid.to_string());
            }
        }

        for (dead_snake_id, _score) in &dead_snakes {
            self.snakes.remove(dead_snake_id);
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
