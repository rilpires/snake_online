use std::{collections::HashSet};

use serde::{Deserialize, Serialize};

// ============================================================================
// TIPOS BÁSICOS DO JOGO
// ============================================================================

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JoinGame {
    pub game_id: Option<String>,
    pub size: Option<Size>,
}


#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Position {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Size {
    pub width: i32,
    pub height: i32,
}

impl Default for Size {
    fn default() -> Self {
        Self { width: 32, height: 32 }
    }
}

impl Position {
    pub fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
    
    pub fn move_in_direction(self, direction: Direction) -> Self {
        match direction {
            Direction::Up => Position::new(self.x, self.y - 1),
            Direction::Down => Position::new(self.x, self.y + 1),
            Direction::Left => Position::new(self.x - 1, self.y),
            Direction::Right => Position::new(self.x + 1, self.y),
        }
    }
}

// ============================================================================
// ENTIDADES DO JOGO
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snake {
    pub body: Vec<Position>,
    pub direction: Direction,
    pub grow_next: bool,
    
    #[serde(skip_serializing)]
    pub next_direction: Option<Direction>,
}

impl Snake {
    pub fn new(start_x: i32, start_y: i32) -> Self {
        Snake {
            body: vec![
                Position::new(start_x, start_y),
                Position::new(start_x - 1, start_y),
                Position::new(start_x - 2, start_y),
            ],
            direction: Direction::Right,
            next_direction: None,
            grow_next: false,
        }
    }

    pub fn head(&self) -> Position {
        self.body[0]
    }

    pub fn move_forward(&mut self) {
        self.direction = self.next_direction.unwrap_or(self.direction);
        let new_head = self.head().move_in_direction(self.direction);
        self.next_direction = None;
        self.body.insert(0, new_head);

        if !self.grow_next {
            self.body.pop();
        } else {
            self.grow_next = false;
        }
    }

    pub fn change_direction(&mut self, new_direction: Direction) {
        if !self.is_opposite_direction(new_direction) {
            self.next_direction = Some(new_direction);
        }
    }

    fn is_opposite_direction(&self, direction: Direction) -> bool {
        matches!(
            (self.direction, direction),
            (Direction::Up, Direction::Down) |
            (Direction::Down, Direction::Up) |
            (Direction::Left, Direction::Right) |
            (Direction::Right, Direction::Left)
        )
    }

    pub fn check_self_collision(&self) -> bool {
        let head = self.head();
        self.body[1..].iter().any(|&pos| pos == head)
    }

    pub fn grow(&mut self) {
        self.grow_next = true;
    }

    pub fn is_colliding_with_walls(&self, width: i32, height: i32) -> bool {
        let head = self.head();
        head.x < 0 || head.x >= width as i32 || head.y < 0 || head.y >= height as i32
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Food {
    pub position: Position,
}

impl Food {
    pub fn new(position: Position) -> Self {
        Self { position }
    }
}

// ============================================================================
// ESTADO PRINCIPAL DO JOGO
// ============================================================================

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
