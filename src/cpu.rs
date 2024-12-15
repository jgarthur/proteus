use crate::types::{Direction, Message};

const INITIAL_STACK_CAPACITY: usize = 8;

// CPU state for a program
pub struct CPUState {
    stack: Vec<i16>,
    // Program ID
    id: i8,

    // READ-ONLY REGISTERS
    // Plasmid pointer
    pp: i8,
    // Instruction pointer
    ip: i16,
    // Error/message received flag
    flag: bool,
    // Loop counter
    lc: i16,
    // Message
    msg: Message,
    // Message received from direction
    msg_dir: Direction,

    // TARGETING REGISTERS
    // Direction if targeting adjacent cell
    dir: Direction,
    // Target adjacent cell (true) or self (false)
    adj: bool,
    // Plasmid offset
    po: i8,
    // Instruction offset
    io: i16,
    // Label
    lab: i8,
}
