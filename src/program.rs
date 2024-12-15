use crate::cpu::CPUState;
use crate::instruction::Instruction;

// Representation of a program in a cell
pub struct Program {
    plasmids: Vec<Plasmid>,
    cpu: CPUState,
    size: usize,
}

pub struct Plasmid {
    instructions: Vec<Instruction>,
    labels: Vec<usize>, /* FIXME */
}
