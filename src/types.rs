use std::fmt::{Debug, Display, Formatter, Result};
use std::ops::Add;

use rand::distributions::{Distribution, Standard};
use rand::Rng;

// EXAMPLE
// use rand::{
//     distributions::{Distribution, Standard},
//     Rng,
// }; // 0.8.0

// #[derive(Debug)]
// enum Spinner {
//     One,
//     Two,
//     Three,
// }

// impl Distribution<Spinner> for Standard {
//     fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> Spinner {
//         // match rng.gen_range(0, 3) { // rand 0.5, 0.6, 0.7
//         match rng.gen_range(0..=2) { // rand 0.8
//             0 => Spinner::One,
//             1 => Spinner::Two,
//             _ => Spinner::Three,
//         }
//     }
// }

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

impl Distribution<Direction> for Standard {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> Direction {
        match rng.gen_range(0..=3) {
            0 => Direction::Right,
            1 => Direction::Up,
            2 => Direction::Left,
            _ => Direction::Down,
        }
    }
}
