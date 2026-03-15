use std::ops::{Index, IndexMut};

use smallvec::{smallvec, SmallVec};

use crate::instruction::Instruction;

const INITIAL_PLASMIDS_CAPACITY: usize = 2;
const INITIAL_INSTRUCTIONS_CAPACITY: usize = 16;
const INITIAL_LABELS_CAPACITY: usize = 1;

#[derive(Clone, Debug)]
pub struct Program {
    pub plasmids: SmallVec<[Plasmid; INITIAL_PLASMIDS_CAPACITY]>,
}

impl Program {
    pub fn size(&self) -> u16 {
        self.plasmids
            .iter()
            .map(|plasmid| plasmid.len() as u16)
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

    pub fn remove_last_instruction(&mut self) -> bool {
        // might need to update labels here
        self.plasmids
            .last_mut()
            .and_then(|plasmid| plasmid.instructions.pop())
            .is_some()
    }
}

// Note: must match Default::default().size() !
pub const DEFAULT_PROGRAM_SIZE: u16 = 1;
// Note: must match DEFAULT_PROGRAM_SIZE !
impl Default for Program {
    fn default() -> Self {
        let mut plasmid = Plasmid::default();
        plasmid.add_instruction(Instruction::Nop);
        Self {
            plasmids: smallvec![plasmid],
        }
    }
}

#[derive(Clone, Debug)]
pub struct Plasmid {
    instructions: SmallVec<[Instruction; INITIAL_INSTRUCTIONS_CAPACITY]>,
    _labels: SmallVec<[usize; INITIAL_LABELS_CAPACITY]>,
}

impl Default for Plasmid {
    fn default() -> Self {
        Self {
            instructions: smallvec![],
            _labels: smallvec![0],
        }
    }
}

impl Plasmid {
    #[inline]
    fn len(&self) -> usize {
        self.instructions.len()
    }

    #[inline]
    pub fn add_instruction(&mut self, instruction: Instruction) {
        self.instructions.push(instruction);
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
