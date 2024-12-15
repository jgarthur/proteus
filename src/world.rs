use crate::program::Program;
use crate::types::{Direction, Message};

#[derive(Clone, Debug)]
struct DirectedRadiation {
    direction: Direction,
    message: Message,
}

#[derive(Clone, Default, Debug)]
struct Cell {
    program: Option<Program>,
    free_energy: u32,
    free_mass: u32,
    background_radiation: u8,
    directed_radiation: Option<DirectedRadiation>,
}

struct Grid {
    cells: Vec<Cell>,
    width: u32,
    height: u32,
    half_width: i32,
    half_height: i32,
}

impl Grid {
    pub fn new(width: u32, height: u32) -> Self {
        let size = width
            .checked_mul(height)
            .expect("Grid size exceeds maximum value");

        Grid {
            cells: vec![Cell::default(); size as usize],
            width,
            height,
            half_width: (width / 2) as i32,
            half_height: (height / 2) as i32,
        }
    }

    pub fn get_cell(&self, x: i32, y: i32) -> &Cell {
        let idx = self.xy_to_idx(x, y);
        &self.cells[idx as usize]
    }

    pub fn get_cell_mut(&mut self, x: i32, y: i32) -> &mut Cell {
        let idx = self.xy_to_idx(x, y);
        &mut self.cells[idx as usize]
    }

    fn xy_to_idx(&self, x: i32, y: i32) -> u32 {
        // Convert from centered coordinates and wrap around
        let grid_x = (x + self.half_width).rem_euclid(self.width as i32);
        let grid_y = (y + self.half_height).rem_euclid(self.height as i32);

        (grid_y as u32) * self.width + (grid_x as u32)
    }

    fn idx_to_xy(&self, idx: u32) -> (i32, i32) {
        let grid_x = idx % self.width;
        let grid_y = idx / self.width;

        let x = (grid_x as i32) - self.half_width;
        let y = (grid_y as i32) - self.half_height;

        (x, y)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_grid_coordinate_roundtrip() {
        let grid = Grid::new(6, 4);
        let total_size = (grid.width * grid.height) as usize;
        let mut xy_coords = Vec::with_capacity(total_size);
        for idx in 0..total_size {
            let xy = grid.idx_to_xy(idx as u32);
            xy_coords.push(xy);
            // Round trip check
            assert_eq!(grid.xy_to_idx(xy.0, xy.1) as usize, idx);
        }

        // Check uniqueness
        let unique_coords: std::collections::HashSet<_> = xy_coords.into_iter().collect();
        assert_eq!(unique_coords.len(), total_size);
    }
}
