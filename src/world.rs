use std::cmp::min;

use rand::Rng;

use crate::cell::{Cell, DirectedRadiation};
use crate::grid::Grid;
use crate::mutation::MutationRules;
use crate::random::geometric_pow2;

#[derive(Clone, Debug)]
pub struct WorldParams {
    pub grid_width: i32,
    pub grid_height: i32,
    pub move_scale: usize,
    pub maintenance_scale: usize,
    pub rad_to_mass_rate_log2: usize,   // -log2(prob)
    pub bg_rad_update_rate_log2: usize, // -log2(prob)
    pub bg_rad_scale: usize,            // binomial: n
    pub bg_rad_rate_log2: usize,        // binomial: -log2(p)
    pub mutations: MutationRules,
    // background_radiation_resolution?
    // Seed organisms
    // Solar radiation parameters - xy/time resolution, shift, scale, octaves
}

impl Default for WorldParams {
    fn default() -> Self {
        Self {
            grid_width: 20,
            grid_height: 20,
            move_scale: 8,
            maintenance_scale: 64,
            rad_to_mass_rate_log2: 8,
            bg_rad_update_rate_log2: 3,
            bg_rad_scale: 2,
            bg_rad_rate_log2: 1,
            mutations: Default::default(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct World {
    pub params: WorldParams,
    pub grid: Grid<Cell>,
    bg_rad_counter: u32, // time until background radiation update
}

impl World {
    pub fn new<R: Rng + ?Sized>(params: WorldParams, rng: &mut R) -> Self {
        // Initialize cell grid
        // TODO: Cells and Programs should have their own Rng. Cell needs to update radiation in initialize
        // TODO: world.update method etc
        let mut grid: Grid<Cell> = Default::default();
        for cell in &mut grid.values {
            cell.initialize(&params, rng);
        }
        let bg_rad_counter = 0;
        let mut world = Self {
            grid,
            params,
            bg_rad_counter,
        };
        world.update_radiation_counter(rng);
        return world;
    }

    pub fn update_radiation_counter<R: Rng + ?Sized>(&mut self, rng: &mut R) {
        self.bg_rad_counter = geometric_pow2(rng, self.params.bg_rad_update_rate_log2) as u32;
    }
}
