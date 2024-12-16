use std::fmt::{Debug, Display, Formatter, Result};

pub type Message = i16;

#[derive(Clone, Copy, Debug, Default)]
#[repr(u8)]
pub enum Direction {
    #[default]
    Right = 0,
    Up = 1,
    Left = 2,
    Down = 3,
}

impl Display for Direction {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        match self {
            Direction::Right => write!(f, "→"),
            Direction::Up => write!(f, "↑"),
            Direction::Left => write!(f, "←"),
            Direction::Down => write!(f, "↓"),
        }
    }
}

impl Direction {
    pub fn to_xy(&self) -> (i32, i32) {
        match self {
            Direction::Right => (1, 0),
            Direction::Up => (0, 1),
            Direction::Left => (-1, 0),
            Direction::Down => (0, -1),
        }
    }

    pub fn rotate_cw(&self) -> Self {
        match self {
            Direction::Right => Direction::Down,
            Direction::Up => Direction::Right,
            Direction::Left => Direction::Up,
            Direction::Down => Direction::Left,
        }
    }

    pub fn rotate_ccw(&self) -> Self {
        match self {
            Direction::Right => Direction::Up,
            Direction::Up => Direction::Left,
            Direction::Left => Direction::Down,
            Direction::Down => Direction::Right,
        }
    }

    pub fn flip(&self) -> Self {
        match self {
            Direction::Right => Direction::Left,
            Direction::Up => Direction::Down,
            Direction::Left => Direction::Right,
            Direction::Down => Direction::Up,
        }
    }
}
