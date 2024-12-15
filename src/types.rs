use std::fmt::{Debug, Display, Formatter, Result};

pub type Message = i16;

#[derive(Clone, Debug)]
#[repr(u8)]
pub enum Direction {
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
    fn rotate_cw(&self) -> Self {
        match self {
            Direction::Right => Direction::Down,
            Direction::Down => Direction::Left,
            Direction::Left => Direction::Up,
            Direction::Up => Direction::Right,
        }
    }

    fn rotate_ccw(&self) -> Self {
        match self {
            Direction::Right => Direction::Up,
            Direction::Up => Direction::Left,
            Direction::Left => Direction::Down,
            Direction::Down => Direction::Right,
        }
    }

    fn flip(&self) -> Self {
        match self {
            Direction::Right => Direction::Left,
            Direction::Left => Direction::Right,
            Direction::Up => Direction::Down,
            Direction::Down => Direction::Up,
        }
    }
}
