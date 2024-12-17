use crate::cpu::CPU;
use crate::grid::Grid;
use crate::mutation::MutationRules;
use crate::program::Program;
use crate::types::{Direction, Message};

#[derive(Clone, Debug)]
pub struct DirectedRadiation {
    pub direction: Direction,
    pub message: Message,
}

#[derive(Clone, Debug, Default)]
pub struct Cell {
    pub program: Option<Program>,
    pub cpu: CPU,
    pub free_energy: u32,
    pub free_mass: u32,
    pub background_radiation: u8,
    pub directed_radiation: Option<DirectedRadiation>,
    pub mutation_counter: u32,
}

#[derive(Clone, Debug)]
pub struct WorldParams {
    pub move_scale: usize,
    pub maintenance_scale: usize,
    pub radiation_to_mass_rate_log2: usize,
    pub background_radiation_scale: usize,     // binomial n
    pub background_radiation_rate_log2: usize, // binomial p
    pub mutation_rules: MutationRules,
    // TODO: background_radiation_resolution?
    // Seed organisms
    // Solar radiation parameters - xy/time resolution, shift, scale, octaves
}

impl Default for WorldParams {
    fn default() -> Self {
        Self {
            move_scale: 8,
            maintenance_scale: 64,
            radiation_to_mass_rate_log2: 8,
            background_radiation_scale: 2,
            background_radiation_rate_log2: 1,
            mutation_rules: Default::default(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct World {
    pub grid: Grid<Cell>,
    // radiation rate
}
