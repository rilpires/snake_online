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
    pub fn new(
        dir: Direction,
        head: Position,
        size: usize,
    ) -> Self {
        let mut body = Vec::with_capacity(size);
        for i in 0..size {
            let pos = match dir {
                Direction::Up => Position::new(head.x, head.y + i as i32),
                Direction::Down => Position::new(head.x, head.y - i as i32),
                Direction::Left => Position::new(head.x + i as i32, head.y),
                Direction::Right => Position::new(head.x - i as i32, head.y),
            };
            body.push(pos);
        }
        Snake {
            body,
            direction: dir,
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
