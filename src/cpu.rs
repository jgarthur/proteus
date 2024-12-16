use crate::types::{Direction, Message};

const INITIAL_STACK_CAPACITY: usize = 8;

// CPU state for a program
#[derive(Clone, Debug)]
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

impl Default for CPUState {
    fn default() -> Self {
        Self {
            stack: Vec::with_capacity(INITIAL_STACK_CAPACITY),
            id: 0,
            pp: 0,
            ip: 0,
            flag: false,
            lc: 0,
            msg: 0,
            msg_dir: Default::default(),
            dir: Default::default(),
            adj: false,
            po: 0,
            io: 0,
            lab: -1,
        }
    }
}
