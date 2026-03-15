use std::error::Error;
use std::fmt;

use crate::model::{Cell, Direction};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Grid {
    width: u32,
    height: u32,
    cells: Vec<Cell>,
}

impl Grid {
    pub fn new(width: u32, height: u32) -> Result<Self, GridError> {
        let len = cell_count(width, height)?;
        let cells = vec![Cell::default(); len];
        Ok(Self {
            width,
            height,
            cells,
        })
    }

    pub fn from_cells(width: u32, height: u32, cells: Vec<Cell>) -> Result<Self, GridError> {
        let expected = cell_count(width, height)?;
        if cells.len() != expected {
            return Err(GridError::CellCountMismatch {
                expected,
                actual: cells.len(),
            });
        }

        Ok(Self {
            width,
            height,
            cells,
        })
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn len(&self) -> usize {
        self.cells.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }

    pub fn index(&self, x: u32, y: u32) -> usize {
        assert!(
            x < self.width,
            "x coordinate {x} exceeds grid width {}",
            self.width
        );
        assert!(
            y < self.height,
            "y coordinate {y} exceeds grid height {}",
            self.height
        );

        let width = usize::try_from(self.width).expect("grid width does not fit usize");
        let x = usize::try_from(x).expect("x coordinate does not fit usize");
        let y = usize::try_from(y).expect("y coordinate does not fit usize");
        (y * width) + x
    }

    pub fn x(&self, index: usize) -> u32 {
        let width = usize::try_from(self.width).expect("grid width does not fit usize");
        let x = index % width;
        u32::try_from(x).expect("x coordinate does not fit u32")
    }

    pub fn y(&self, index: usize) -> u32 {
        let width = usize::try_from(self.width).expect("grid width does not fit usize");
        let y = index / width;
        u32::try_from(y).expect("y coordinate does not fit u32")
    }

    pub fn neighbor(&self, index: usize, dir: Direction) -> usize {
        let x = self.x(index);
        let y = self.y(index);

        match dir {
            Direction::Right => self.index((x + 1) % self.width, y),
            Direction::Up => self.index(x, (y + (self.height - 1)) % self.height),
            Direction::Left => self.index((x + (self.width - 1)) % self.width, y),
            Direction::Down => self.index(x, (y + 1) % self.height),
        }
    }

    pub fn get(&self, index: usize) -> Option<&Cell> {
        self.cells.get(index)
    }

    pub fn get_mut(&mut self, index: usize) -> Option<&mut Cell> {
        self.cells.get_mut(index)
    }

    pub fn cells(&self) -> &[Cell] {
        &self.cells
    }

    pub fn cells_mut(&mut self) -> &mut [Cell] {
        &mut self.cells
    }
}

pub fn cell_count(width: u32, height: u32) -> Result<usize, GridError> {
    if width == 0 {
        return Err(GridError::ZeroWidth);
    }
    if height == 0 {
        return Err(GridError::ZeroHeight);
    }

    let width = usize::try_from(width).map_err(|_| GridError::DimensionsTooLarge)?;
    let height = usize::try_from(height).map_err(|_| GridError::DimensionsTooLarge)?;

    width
        .checked_mul(height)
        .ok_or(GridError::DimensionsTooLarge)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GridError {
    ZeroWidth,
    ZeroHeight,
    DimensionsTooLarge,
    CellCountMismatch { expected: usize, actual: usize },
}

impl fmt::Display for GridError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroWidth => write!(f, "grid width must be greater than zero"),
            Self::ZeroHeight => write!(f, "grid height must be greater than zero"),
            Self::DimensionsTooLarge => write!(f, "grid dimensions are too large"),
            Self::CellCountMismatch { expected, actual } => {
                write!(f, "grid expected {expected} cells but received {actual}")
            }
        }
    }
}

impl Error for GridError {}

#[cfg(test)]
mod tests {
    use super::{Grid, GridError};
    use crate::model::Direction;

    #[test]
    fn toroidal_neighbors_wrap_on_all_sides() {
        let grid = Grid::new(3, 2).expect("grid should build");
        let origin = grid.index(0, 0);

        assert_eq!(grid.neighbor(origin, Direction::Left), grid.index(2, 0));
        assert_eq!(grid.neighbor(origin, Direction::Right), grid.index(1, 0));
        assert_eq!(grid.neighbor(origin, Direction::Up), grid.index(0, 1));
        assert_eq!(grid.neighbor(origin, Direction::Down), grid.index(0, 1));
    }

    #[test]
    fn from_cells_rejects_wrong_cell_count() {
        let result = Grid::from_cells(2, 2, Vec::new());
        assert_eq!(
            result,
            Err(GridError::CellCountMismatch {
                expected: 4,
                actual: 0
            })
        );
    }
}
