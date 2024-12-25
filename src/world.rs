use std::collections::HashMap;

use rand::{Rng, SeedableRng};
use rayon::prelude::*;

use crate::cell::Cell;
use crate::grid::{grid_size_checked, Grid};
use crate::mutation::MutationRules;
use crate::random::{binom_pow2, geometric_pow2, FastRng};
use crate::types::{Direction, Message};

#[derive(Clone, Debug)]
pub struct WorldParams {
    pub grid_width: i32,
    pub grid_height: i32,
    pub move_scale: usize,
    pub maintenance_scale: u32,
    pub rad_to_mass_rate_log2: usize,   // -log2(prob)
    pub bg_rad_update_rate_log2: usize, // -log2(prob)
    pub bg_rad_scale: u64,              // binomial: n
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
    rng: FastRng,
}

impl World {
    pub fn new(params: WorldParams) -> Self {
        let mut rng = FastRng::seed_from_u64(params.rng_seed);

        // initialize cells and grid
        let grid_size = grid_size_checked(params.grid_width, params.grid_height);
        // each cell has its own rng seeded by the world rng
        let mut initial_rad: Vec<BackgroundRadiation> = (0..grid_size)
            .map(|_| BackgroundRadiation::new(&mut rng, &params))
            .collect();
        let cells: Vec<Cell> = (0..grid_size)
            .map(|_| Cell::new(initial_rad.pop().unwrap(), rng.gen(), &params))
            .collect();
        let bg_rad_counter = Self::generate_radiation_counter(&mut rng, &params);
        let grid = Grid::from_iter_row_major(cells, params.grid_width);

        Self {
            params,
            grid,
            bg_rad_counter,
            rng,
        }
    }

    #[inline]
    fn generate_radiation_counter(rng: &mut FastRng, params: &WorldParams) -> u32 {
        geometric_pow2(rng, params.bg_rad_update_rate_log2) as u32
    }

    pub fn update_physics(&mut self) {
        self.bg_rad_counter -= 1;
        let bg_rad_update = self.bg_rad_counter == 0;
        if bg_rad_update {
            self.bg_rad_counter = Self::generate_radiation_counter(&mut self.rng, &self.params);
        }

        let dir_rad_updates: Vec<_> = self
            .grid
            .par_iter_mut()
            .flat_map(|(cell, coord)| {
                // background radiation may be converted to free mass or new program
                cell.handle_bg_radiation(&self.params);
                cell.handle_program_maintenance(&self.params);
                cell.free_resource_decay();
                if bg_rad_update {
                    cell.bg_rad.update(&mut cell.rng, &self.params);
                }
                cell.directed_rad.take().map(|rad| (coord, rad))
            })
            .collect();

        // Group radiation by destination coordinate and apply effects
        let mut updated_rad = HashMap::new();
        for (coord, rad) in dir_rad_updates {
            updated_rad
                .entry(coord + rad.direction.to_offset())
                .or_insert(vec![])
                .push(rad);
        }
        for (coord, rads) in updated_rad {
            if rads.len() == 1 {
                self.grid[coord].directed_rad = Some(rads[0]);
            } else {
                self.grid[coord].free_energy += rads.len() as u32;
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct BackgroundRadiation(pub u8);

impl BackgroundRadiation {
    #[inline]
    pub fn new(rng: &mut FastRng, params: &WorldParams) -> Self {
        Self(Self::new_value(rng, params))
    }

    #[inline]
    pub fn update(&mut self, rng: &mut FastRng, params: &WorldParams) {
        self.0 = Self::new_value(rng, params);
    }

    #[inline]
    fn new_value(rng: &mut FastRng, params: &WorldParams) -> u8 {
        binom_pow2(rng, params.bg_rad_scale, params.bg_rad_rate_log2) as u8
    }
}

#[derive(Clone, Copy, Debug)]
pub struct DirectedRadiation {
    pub direction: Direction,
    pub message: Message,
}
