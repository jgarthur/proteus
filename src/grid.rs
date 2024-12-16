use crate::types::Direction;
use std::fmt::Debug;

#[derive(Clone, Debug, Default)]
pub struct Grid<Value> {
    values: Vec<Value>,
    size: usize,
    width: i32,
    height: i32,
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
struct Coord(i32, i32);

impl<Value> Grid<Value>
where
    Value: Clone + Debug + Default,
{
    fn new(width: i32, height: i32) -> Self {
        if width <= 0 || height <= 0 {
            panic!("Grid width and height must be positive");
        }

        let size = (width as usize)
            .checked_mul(height as usize)
            .expect("Total number of grid cells exceeds usize::MAX");

        Grid {
            values: vec![Value::default(); size],
            size,
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
        let grid_x = (coord.0 + self.width / 2).rem_euclid(self.width);
        let grid_y = (coord.1 + self.height / 2).rem_euclid(self.height);

        (grid_y as u64) * (self.width as u64) + (grid_x as u64)
    }

    fn idx_to_coord(&self, idx: u64) -> Coord {
        let grid_x = idx % (self.width as u64);
        let grid_y = idx / (self.width as u64);

        let x = (grid_x as i32) - self.width / 2;
        let y = (grid_y as i32) - self.height / 2;

        Coord(x, y)
    }

    fn offset_dir(&self, coord: Coord, dir: Direction) -> Coord {
        let dir_xy = dir.to_xy();
        self.add_offset(coord, dir_xy.0, dir_xy.1)
    }

    fn add_offset(&self, coord: Coord, dx: i32, dy: i32) -> Coord {
        let half_width = self.width / 2;
        let half_height = self.height / 2;
        let new_x: i32 = self.wrap(coord.0 + dx, self.width, half_width);
        let new_y = self.wrap(coord.1 + dy, self.height, half_height);
        Coord(new_x, new_y)
    }

    /// Wraps a coordinate value around the grid toroidally.
    fn wrap(&self, x_or_y: i32, dim: i32, half_dim: i32) -> i32 {
        (x_or_y + half_dim).rem_euclid(dim) - half_dim
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
