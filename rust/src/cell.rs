use std::cmp::min;

use log::debug;
use rand::{Rng, SeedableRng};

use crate::cpu::CPU;
use crate::instruction::Instruction;
use crate::program::{Program, DEFAULT_PROGRAM_SIZE};
use crate::random::{geometric_pow2, FastRng};
use crate::world::{BackgroundRadiation, DirectedRadiation, WorldParams};

#[derive(Clone, Debug)]
pub struct Cell {
    pub program: Option<Program>,
    pub rng: FastRng,
    pub free_energy: u32,
    pub free_mass: u32,
    pub cpu: CPU,
    pub directed_rad: Option<DirectedRadiation>,
    pub mutation_counter: u32,
    pub rad_to_mass_counter: u32,
    pub program_size: u16,
    pub bg_rad: BackgroundRadiation,
    pub is_vulnerable: bool,
    pub is_trapped: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CostPayment {
    /// Cost was paid using only free energy
    FreeEnergy,
    /// Cost was paid using some background radiation (may have also used free energy)
    UsedRadiation,
    /// Not enough resources to pay the cost
    Insufficient,
}

impl Cell {
    pub fn new(initial_bg_rad: BackgroundRadiation, rng_seed: u64, params: &WorldParams) -> Self {
        let mut rng = FastRng::seed_from_u64(rng_seed);

        Self {
            free_energy: 0,
            free_mass: 0,
            program: None,
            program_size: 0,
            cpu: CPU {
                dir: rng.gen(),
                ..Default::default()
            },
            bg_rad: initial_bg_rad,
            directed_rad: None,
            mutation_counter: Self::generate_mutation_counter(&mut rng, params),
            rad_to_mass_counter: Self::generate_rad_to_mass_counter(&mut rng, params),
            is_vulnerable: true,
            is_trapped: false,
            rng,
        }
    }

    pub fn handle_bg_radiation(&mut self, params: &WorldParams) {
        if self.bg_rad.0 > 0 {
            self.rad_to_mass_counter -= 1;
            if self.rad_to_mass_counter == 0 {
                self.bg_rad.0 -= 1;
                if let Some(_) = &self.program {
                    debug!("bg radiation to free mass");
                    self.free_mass += 1;
                } else {
                    debug!("bg radiation to new program");
                    self.program = Some(Default::default());
                    self.program_size = DEFAULT_PROGRAM_SIZE;
                }
                self.rad_to_mass_counter =
                    Self::generate_rad_to_mass_counter(&mut self.rng, params);
            }
        }
    }

    pub fn handle_program_maintenance(&mut self, params: &WorldParams) {
        let mut maintenance_cost = (self.program_size() as u32) / params.maintenance_scale;
        if maintenance_cost > 0 {
            // Try to pay with free energy
            if self.free_energy >= maintenance_cost {
                self.free_energy -= maintenance_cost;
                return;
            } else {
                maintenance_cost -= self.free_energy;
                self.free_energy = 0;
            }
            // Try to pay with free mass
            if self.free_mass >= maintenance_cost {
                self.free_mass -= maintenance_cost;
                return;
            } else {
                maintenance_cost -= self.free_mass;
                self.free_mass = 0;
            }
            // Last resort: destroy instructions from the end of the last plasmid
            let program = self.program.as_mut().unwrap();
            while maintenance_cost > 0 && program.remove_last_instruction() {
                self.program_size -= 1;
                maintenance_cost -= 1;
            }
        }
    }

    #[inline]
    pub fn free_resource_decay(&mut self) {
        let soft_cap = self.program_size() as u32;
        if self.free_energy > soft_cap {
            self.free_energy = soft_cap + (self.free_energy - soft_cap) / 2;
        }
        if self.free_mass > soft_cap {
            self.free_mass = soft_cap + (self.free_mass - soft_cap) / 2;
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

    #[inline]
    pub fn program_size(&self) -> u16 {
        debug_assert!(self.program_size == self.program.as_ref().map_or(0, |p| p.size()));
        self.program_size
    }

    #[inline]
    pub fn program_strength(&self) -> u32 {
        min(self.program_size() as u32, self.free_energy)
    }

    /// Get instruction. Return None if no instruction to return
    /// Does not advance the instruction pointer
    pub fn next_instruction(&self) -> Option<Instruction> {
        self.program
            .as_ref()
            .and_then(|p| p.get(self.cpu.pp, self.cpu.ip))
    }

    /// Get instruction mutably. Return None if no instruction to return
    /// Does not advance the instruction pointer
    pub fn next_instruction_mut(&mut self) -> Option<&mut Instruction> {
        self.program
            .as_mut()
            .and_then(|p| p.get_mut(self.cpu.pp, self.cpu.ip))
    }

    /// Increment the instruction pointer
    pub fn inc_inst_ptr(&mut self) {
        if let Some(p) = &self.program {
            p.inc_inst_ptr(&self.cpu.pp, &mut self.cpu.ip);
        }
    }

    /// Check if mutation should occur based on the mutation counter and background radiation used
    pub fn check_mutation(&mut self, params: &WorldParams, background_rad: Option<u8>) -> bool {
        self.mutation_counter = self
            .mutation_counter
            .saturating_sub(params.mutations.get_counter_decrement(background_rad));
        if self.mutation_counter == 0 {
            self.mutation_counter = Self::generate_mutation_counter(&mut self.rng, params);
            return true;
        }
        false
    }

    /// Attempts to pay the given cost using free energy first, then background radiation if needed.
    /// Returns how the cost was paid, or Insufficient if it couldn't be paid.
    pub fn pay_cost(&mut self, energy: u32, mass: u32) -> CostPayment {
        let total_energy = self.free_energy + self.bg_rad.0 as u32;
        if self.free_mass >= mass && total_energy >= energy {
            let bg_rad_payment = energy.saturating_sub(self.free_energy) as u8;
            self.free_mass -= mass;
            self.free_energy = self.free_energy.saturating_sub(energy);
            self.bg_rad.0 -= bg_rad_payment;
            if bg_rad_payment > 0 {
                CostPayment::UsedRadiation
            } else {
                CostPayment::FreeEnergy
            }
        } else {
            CostPayment::Insufficient
        }
    }

    /// Check if the cell can pay the given cost using free energy and background radiation
    pub fn can_pay_cost(&self, energy: u32, mass: u32) -> bool {
        let total_energy = self.free_energy + self.bg_rad.0 as u32;
        self.free_mass >= mass && total_energy >= energy
    }
}
