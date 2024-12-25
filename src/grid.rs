use std::fmt::Debug;
use std::ops::{Index, IndexMut};

use rayon::prelude::*;

use crate::types::{Coord, Direction};

const DEFAULT_GRID_DIM: i32 = 100;

/// Two-dimensional grid of values backed by Vec
#[derive(Clone, Debug)]
pub struct Grid<Value> {
    pub values: Vec<Value>,
    pub width: i32,
    pub height: i32,
}

// Separate function to simplify calling, no generic type
#[inline]
pub fn grid_size_checked(width: i32, height: i32) -> isize {
    if width <= 0 || height <= 0 {
        panic!("Grid width and height must be positive");
    }

    (width as isize)
        .checked_mul(height as isize)
        .expect("Total number of grid cells exceeds isize::MAX")
}

#[inline]
pub fn grid_coord_to_idx(coord: Coord, grid_width: i32, grid_height: i32) -> u64 {
    // Convert to uncentered coordinates
    let grid_col = (coord.0 + grid_width / 2).rem_euclid(grid_width);
    let grid_row = (coord.1 + grid_height / 2).rem_euclid(grid_height);

    (grid_row as u64) * (grid_width as u64) + (grid_col as u64)
}

#[inline]
pub fn grid_idx_to_coord(idx: u64, grid_width: i32, grid_height: i32) -> Coord {
    let grid_col = idx % (grid_width as u64);
    let grid_row = idx / (grid_width as u64);

    let x = (grid_col as i32) - grid_width / 2;
    let y = (grid_row as i32) - grid_height / 2;

    Coord(x, y)
}

impl<Value> Grid<Value>
where
    Value: Clone + Debug,
{
    pub fn new<F, P>(width: i32, height: i32, params: P, init: F) -> Self
    where
        F: Fn(&P, usize) -> Value,
    {
        let size = grid_size_checked(width, height);
        Grid {
            values: (0..size).map(|idx| init(&params, idx as usize)).collect(),
            width,
            height,
        }
    }

    pub fn from_iter_row_major<I>(values: I, width: i32) -> Self
    where
        I: IntoIterator<Item = Value>,
    {
        let values: Vec<Value> = values.into_iter().collect();
        let size = values.len();

        if size == 0 {
            panic!("No values to place in grid")
        }
        if width <= 0 {
            panic!("Grid width must be positive");
        }
        if size % (width as usize) != 0 {
            panic!("Number of values must be divisible by width");
        }

        let height = (size / width as usize) as i32;

        Grid {
            values,
            width,
            height,
        }
    }

    pub fn values(&self) -> std::slice::Iter<'_, Value> {
        self.values.iter()
    }

    pub fn values_mut(&mut self) -> std::slice::IterMut<'_, Value> {
        self.values.iter_mut()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&Value, Coord)> {
        self.values
            .iter()
            .enumerate()
            .map(|(idx, val)| (val, grid_idx_to_coord(idx as u64, self.width, self.height)))
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (&mut Value, Coord)> {
        self.values
            .iter_mut()
            .enumerate()
            .map(|(idx, val)| (val, grid_idx_to_coord(idx as u64, self.width, self.height)))
    }

    pub fn offset_dir(&self, coord: Coord, dir: Direction) -> Coord {
        let dir_offset = dir.to_offset();
        self.add_offset_wrap(coord, dir_offset)
    }

    fn add_offset_wrap(&self, coord: Coord, offset: Coord) -> Coord {
        let sum = coord + offset;
        let half_width = self.width / 2;
        let half_height = self.height / 2;
        let new_x: i32 = self.wrap(sum.0, self.width, half_width);
        let new_y = self.wrap(sum.1, self.height, half_height);
        Coord(new_x, new_y)
    }

    /// Wraps a coordinate value around the grid (torus connectivity)
    fn wrap(&self, x_or_y: i32, dim: i32, half_dim: i32) -> i32 {
        (x_or_y + half_dim).rem_euclid(dim) - half_dim
    }

    #[inline]
    pub fn coord_to_idx(&self, coord: Coord) -> u64 {
        grid_coord_to_idx(coord, self.width, self.height)
    }

    #[inline]
    pub fn idx_to_coord(&self, idx: u64) -> Coord {
        grid_idx_to_coord(idx, self.width, self.height)
    }
}

impl<Value> Grid<Value>
where
    Value: Clone + Debug + Send + Sync,
{
    pub fn par_iter(&self) -> impl ParallelIterator<Item = (&Value, Coord)> {
        self.values
            .par_iter()
            .enumerate()
            .map(|(idx, val)| (val, grid_idx_to_coord(idx as u64, self.width, self.height)))
    }

    pub fn par_iter_mut(&mut self) -> impl ParallelIterator<Item = (&mut Value, Coord)> {
        self.values
            .par_iter_mut()
            .enumerate()
            .map(|(idx, val)| (val, grid_idx_to_coord(idx as u64, self.width, self.height)))
    }
}

impl<Value> Grid<Value>
where
    Value: Clone + Debug + Sync + Default,
{
    pub fn new_default(width: i32, height: i32) -> Self
    where
        Value: Default,
    {
        Self::new(width, height, (), |_, _| Value::default())
    }
}

impl<Value> Default for Grid<Value>
where
    Value: Clone + Debug + Sync + Default,
{
    fn default() -> Self {
        Self::new_default(DEFAULT_GRID_DIM, DEFAULT_GRID_DIM)
    }
}

impl<Value> Index<Coord> for Grid<Value>
where
    Value: Clone + Debug,
{
    type Output = Value;

    fn index(&self, coord: Coord) -> &Self::Output {
        let idx = self.coord_to_idx(coord);
        &self.values[idx as usize]
    }
}

impl<Value> IndexMut<Coord> for Grid<Value>
where
    Value: Clone + Debug,
{
    fn index_mut(&mut self, coord: Coord) -> &mut Self::Output {
        let idx = self.coord_to_idx(coord);
        &mut self.values[idx as usize]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_grid_coordinate_roundtrip() {
        for width in 1..=8 {
            for height in 1..=8 {
                let grid: Grid<usize> = Grid::new_default(width, height);
                let total_size = (grid.width * grid.height) as usize;
                let mut coords = Vec::with_capacity(total_size);
                for idx in 0..total_size {
                    let coord = grid.idx_to_coord(idx as u64);
                    coords.push(coord);
                    // Round trip check
                    assert_eq!(grid.coord_to_idx(coord) as usize, idx);
                }

                // Check uniqueness
                let unique_coords: std::collections::HashSet<_> = coords.into_iter().collect();
                assert_eq!(unique_coords.len(), total_size);
            }
        }
    }
}
