use crate::instruction::Instruction;

#[derive(Clone, Debug)]
pub struct Program {
    pub plasmids: Vec<Plasmid>,
    pub size: usize, // TODO: in debug mode, check size at each iteration?
    pub mutation_counter: u32,
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

// TODO add cell.rs
// next_instruction() to get instruction and increment
// Cell has vulnerable: bool
// program_strength() -> Option<_>
// THEN finish local execution function
