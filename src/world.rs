use crate::cpu::CPUState;
use crate::physics::DirectedRadiation;
use crate::program::Program;
use crate::types::{Direction, Message};

#[derive(Clone, Debug, Default)]
pub struct Cell {
    program: Option<Program>,
    cpu: CPUState,
    free_energy: u32,
    free_mass: u32,
    background_radiation: u8,
    directed_radiation: Option<DirectedRadiation>,
    mutation_counter: u32,
}

#[derive(Clone, Debug, Default)]
pub struct Grid {
    cells: Vec<Cell>,
    width: i32,
    height: i32,
}

#[derive(Clone, Debug)]
pub struct WorldParams {
    radiation_to_mass_rate_log2: usize,
    background_radiation_scale: usize,
    background_radiation_rate_log2: usize,
    // Seed organisms
    // Solar radiation parameters - xy/time resolution, shift, scale, octaves
}

impl Default for WorldParams {
    fn default() -> Self {
        Self {
            radiation_to_mass_rate_log2: 8,
            background_radiation_scale: 2,
            background_radiation_rate_log2: 1,
        }
    }
}

#[derive(Clone, Debug)]
struct World {
    grid: Grid,
    // radiation, etc.
}
