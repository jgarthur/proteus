use crate::types::{Coord, Direction};
use std::fmt::Debug;

const DEFAULT_GRID_DIM: i32 = 100;

#[derive(Clone, Debug)]
pub struct Grid<Value> {
    pub values: Vec<Value>,
    pub width: i32,
    pub height: i32,
}

impl<Value> Grid<Value>
where
    Value: Clone + Debug + Default,
{
    fn new(width: i32, height: i32) -> Self {
        if width <= 0 || height <= 0 {
            panic!("Grid width and height must be positive");
        }

        let size = (width as isize)
            .checked_mul(height as isize)
            .expect("Total number of grid cells exceeds isize::MAX");

        Grid {
            values: vec![Value::default(); size as usize],
            width,
            height,
        }
    }

    fn from_iter_row_major<I>(values: I, width: i32) -> Self
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

    pub fn value(&self, coord: Coord) -> &Value {
        let idx = self.coord_to_idx(coord);
        &self.values[idx as usize]
    }

    pub fn value_mut(&mut self, coord: Coord) -> &mut Value {
        let idx = self.coord_to_idx(coord);
        &mut self.values[idx as usize]
    }

    fn coord_to_idx(&self, coord: Coord) -> u64 {
        // Convert to uncentered coordinates
        let grid_col = (coord.0 + self.width / 2).rem_euclid(self.width);
        let grid_row = (coord.1 + self.height / 2).rem_euclid(self.height);

        (grid_row as u64) * (self.width as u64) + (grid_col as u64)
    }

    fn idx_to_coord(&self, idx: u64) -> Coord {
        let grid_col = idx % (self.width as u64);
        let grid_row = idx / (self.width as u64);

        let x = (grid_col as i32) - self.width / 2;
        let y = (grid_row as i32) - self.height / 2;

        Coord(x, y)
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
}

impl<Value> Default for Grid<Value>
where
    Value: Clone + Debug + Default,
{
    fn default() -> Self {
        Self::new(DEFAULT_GRID_DIM, DEFAULT_GRID_DIM)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_grid_coordinate_roundtrip() {
        for width in 1..=8 {
            for height in 1..=8 {
                let grid: Grid<usize> = Grid::new(width, height);
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
