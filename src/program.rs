use crate::cpu::CPUState;
use crate::instruction::Instruction;

#[derive(Clone, Debug)]
pub struct Program {
    plasmids: Vec<Plasmid>,
    size: usize, // TODO: in debug mode, check size at each iteration?
    mutation_counter: u32,
}

#[derive(Clone, Debug)]
pub struct Plasmid {
    instructions: Vec<Instruction>,
    labels: Vec<usize>, // FIXME?
}
