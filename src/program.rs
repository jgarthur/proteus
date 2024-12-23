use std::ops::{Index, IndexMut};

use crate::instruction::Instruction;

#[derive(Clone, Debug)]
pub struct Program {
    pub plasmids: Vec<Plasmid>, // TODO: max length i8::MAX
}

impl Program {
    pub fn size(&self) -> u32 {
        // TODO max?
        self.plasmids
            .iter()
            .map(|plasmid| plasmid.len() as u32)
            .sum()
    }

    /// Check if a plasmid has instructions
    pub fn has_nonempty_plasmid(&self, plasmid_pointer: i8) -> bool {
        self.plasmids.len() > 0 && self.plasmids[plasmid_pointer as usize].len() > 0
    }

    /// Get immutable reference to current instruction. Returns None if instruction doesn't exist.
    /// Panics if pointers are out of bounds.
    pub fn get(&self, plasmid_pointer: i8, instruction_pointer: i16) -> Option<Instruction> {
        if !self.has_nonempty_plasmid(plasmid_pointer) {
            return None;
        }
        Some(self.plasmids[plasmid_pointer as usize][instruction_pointer as usize])
    }

    /// Get mutable reference to current instruction. Returns None if instruction doesn't exist.
    /// Panics if pointers are out of bounds.
    pub fn get_mut(
        &mut self,
        plasmid_pointer: i8,
        instruction_pointer: i16,
    ) -> Option<&mut Instruction> {
        if !self.has_nonempty_plasmid(plasmid_pointer) {
            return None;
        }
        Some(&mut self.plasmids[plasmid_pointer as usize][instruction_pointer as usize])
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
