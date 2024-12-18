use std::cmp::min;
use std::ops::{Index, IndexMut};

use rand::Rng;

use crate::instruction::Instruction;
use crate::random::geometric_pow2;
use crate::world::WorldParams;

#[derive(Clone, Debug)]
pub struct Program {
    pub plasmids: Vec<Plasmid>, // TODO: max length i8::MAX
    pub mutation_counter: u32,
}

impl Program {
    pub fn initialize<R: Rng + ?Sized>(&mut self, params: &WorldParams, rng: &mut R) {
        self.mutation_counter = min(
            geometric_pow2(rng, params.mutations.mut_rate_log2) as u32,
            u32::MAX,
        );
    }

    pub fn size(&self) -> u32 {
        // TODO max?
        self.plasmids
            .iter()
            .map(|plasmid| plasmid.len() as u32)
            .sum()
    }

    /// Get instruction. Return None if no instruction to return.
    /// Panics on an invalid pointer
    /// TODO: consider inlining
    pub fn next_instruction(
        &self,
        plasmid_pointer: i8,
        instruction_pointer: i16,
    ) -> Option<Instruction> {
        if self.plasmids.len() == 0 {
            return None;
        }
        let plasmid = &self.plasmids[plasmid_pointer as usize];
        if plasmid.len() == 0 {
            return None;
        }
        Some(plasmid[instruction_pointer as usize])
    }

    /// Increment instruction pointer register
    pub fn inc_inst_ptr(&self, plasmid_pointer: &i8, instruction_pointer: &mut i16) {
        // Get lengths before incrementing pointers
        let num_plasmids = self.plasmids.len();
        if num_plasmids == 0 {
            return;
        }
        let num_instructions = self.plasmids[*plasmid_pointer as usize].len();
        if num_instructions == 0 {
            return;
        }
        *instruction_pointer += 1;
        if *instruction_pointer == num_instructions as i16 {
            *instruction_pointer = 0;
        }
    }
}

#[derive(Clone, Debug)]
pub struct Plasmid {
    pub instructions: Vec<Instruction>,
    pub labels: Vec<usize>, // FIXME?
}

impl Default for Plasmid {
    fn default() -> Self {
        todo!()
    }
}

impl Plasmid {
    fn len(&self) -> usize {
        self.instructions.len()
    }

    fn get(&self, idx: usize) -> Option<&Instruction> {
        self.instructions.get(idx)
    }

    fn get_mut(&mut self, idx: usize) -> Option<&mut Instruction> {
        self.instructions.get_mut(idx)
    }
}

impl Index<usize> for Plasmid {
    type Output = Instruction;

    fn index(&self, index: usize) -> &Self::Output {
        &self.instructions[index]
    }
}

impl IndexMut<usize> for Plasmid {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.instructions[index]
    }
}
