use std::{
    fmt::{Debug, Display, Formatter, Result},
    ops::Add,
};

pub type Message = i16;

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct Coord(pub i32, pub i32);

impl Add for Coord {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Coord(self.0 + other.0, self.1 + other.1)
    }
}

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
    pub fn to_offset(&self) -> Coord {
        match self {
            Direction::Right => Coord(1, 0),
            Direction::Up => Coord(0, 1),
            Direction::Left => Coord(-1, 0),
            Direction::Down => Coord(0, -1),
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
