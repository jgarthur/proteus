use std::cmp::min;

use rand::rngs::SmallRng;
use rand::Rng;
use rand::SeedableRng;

use crate::cell::{Cell, DirectedRadiation};
use crate::grid::{grid_size_checked, Grid};
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
    pub rng_seed: u64,
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
            rng_seed: 0,
        }
    }
}

#[derive(Clone, Debug)]
pub struct World {
    pub params: WorldParams,
    pub grid: Grid<Cell>,
    bg_rad_counter: u32,
    rng: SmallRng,
}

impl World {
    pub fn new(params: WorldParams) -> Self {
        let mut rng = SmallRng::seed_from_u64(params.rng_seed);

        // initialize cells and grid
        let grid_size = grid_size_checked(params.grid_width, params.grid_height);
        // each cell has its own rng seeded by the world rng
        let cells: Vec<Cell> = (0..grid_size)
            .map(|_| Cell::new(rng.gen(), &params))
            .collect();
        let grid = Grid::from_iter_row_major(cells, params.grid_width);

        let bg_rad_counter = Self::generate_counter(&mut rng, &params);
        Self {
            params,
            grid,
            bg_rad_counter,
            rng,
        }
    }

    #[inline]
    fn generate_counter(rng: &mut SmallRng, params: &WorldParams) -> u32 {
        geometric_pow2(rng, params.bg_rad_update_rate_log2) as u32
    }

    pub fn update_radiation_counter(&mut self) {
        self.bg_rad_counter = Self::generate_counter(&mut self.rng, &self.params);
    }
}
