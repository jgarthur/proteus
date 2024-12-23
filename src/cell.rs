use std::cmp::min;

use rand::{Rng, SeedableRng};

use crate::cpu::CPU;
use crate::instruction::Instruction;
use crate::program::Program;
use crate::random::{geometric_pow2, FastRng};
use crate::world::{BackgroundRadiation, DirectedRadiation, WorldParams};

#[derive(Clone, Debug)]
pub struct Cell {
    pub program: Option<Program>,
    pub free_energy: u32,
    pub free_mass: u32,
    pub cpu: CPU,
    pub bg_rad: BackgroundRadiation,
    pub directed_rad: Option<DirectedRadiation>,
    pub mutation_counter: u32,
    pub rad_to_mass_counter: u32,
    pub is_passable: bool,
    pub is_trapped: bool,
    pub rng: FastRng,
}

impl Cell {
    pub fn new(initial_bg_rad: BackgroundRadiation, rng_seed: u64, params: &WorldParams) -> Self {
        let mut rng = FastRng::seed_from_u64(rng_seed);

        Self {
            free_energy: 0,
            free_mass: 0,
            program: None,
            cpu: CPU {
                dir: rng.gen(),
                ..Default::default()
            },
            bg_rad: initial_bg_rad,
            directed_rad: None,
            mutation_counter: Self::generate_mutation_counter(&mut rng, params),
            rad_to_mass_counter: Self::generate_rad_to_mass_counter(&mut rng, params),
            is_passable: true,
            is_trapped: false,
            rng,
        }
    }

    /// Generate random value for radiation to mass conversion counter
    #[inline]
    pub fn generate_rad_to_mass_counter(rng: &mut FastRng, params: &WorldParams) -> u32 {
        min(
            geometric_pow2(rng, params.rad_to_mass_rate_log2) as u32,
            u32::MAX,
        )
    }

    /// Generate random value for mutation counter
    #[inline]
    pub fn generate_mutation_counter(rng: &mut FastRng, params: &WorldParams) -> u32 {
        min(
            geometric_pow2(rng, params.mutations.mut_rate_log2) as u32,
            u32::MAX,
        )
    }

    pub fn program_size(&self) -> u32 {
        self.program.as_ref().map_or(0, |p| p.size())
    }

    pub fn program_strength(&self) -> u32 {
        min(self.program_size(), self.free_energy)
    }

    /// Get instruction. Return None if no instruction to return
    pub fn next_instruction(&self) -> Option<Instruction> {
        self.program
            .as_ref()
            .and_then(|p| p.get(self.cpu.pp, self.cpu.ip))
    }

    /// Get instruction mutably. Return None if no instruction to return
    pub fn next_instruction_mut(&mut self) -> Option<&mut Instruction> {
        self.program
            .as_mut()
            .and_then(|p| p.get_mut(self.cpu.pp, self.cpu.ip))
    }

    pub fn inc_inst_ptr(&mut self) {
        if let Some(p) = &self.program {
            p.inc_inst_ptr(&self.cpu.pp, &mut self.cpu.ip);
        }
    }

    /// Check if mutation should occur based on the mutation counter and background radiation used
    pub fn check_mutation(&mut self, params: &WorldParams, background_rad: Option<u8>) -> bool {
        self.mutation_counter -= params.mutations.get_counter_decrement(background_rad);
        if self.mutation_counter == 0 {
            self.mutation_counter = Self::generate_mutation_counter(&mut self.rng, params);
            return true;
        }
        false
    }
}
