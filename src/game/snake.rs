use serde::{Deserialize, Serialize};

use crate::game::*;


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
