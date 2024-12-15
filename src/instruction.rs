#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(u8)]
pub enum Instruction {
    Nop = 0b00000000,
    Absorb = 0b00000001,
    Push0 = 0b00000010,
    Push1 = 0b00000011,
    Move = 0b10000000,
    Add = 0b10000001,
}

pub struct InstructionProperties {
    pub execution_time: u8,
    pub base_energy_cost: u8,
    pub is_local: bool,
}

impl Instruction {
    pub const fn execution_time(&self) -> u8 {
        self.properties().execution_time
    }

    pub const fn base_energy_cost(&self) -> u8 {
        self.properties().base_energy_cost
    }

    pub const fn is_local(&self) -> bool {
        self.properties().is_local
    }

    pub const fn properties(&self) -> InstructionProperties {
        match self {
            Self::Nop => InstructionProperties {
                execution_time: 1,
                base_energy_cost: 0,
                is_local: true,
            },
            Self::Absorb => InstructionProperties {
                execution_time: 1,
                base_energy_cost: 0,
                is_local: true,
            },
            Self::Push0 => InstructionProperties {
                execution_time: 0,
                base_energy_cost: 0,
                is_local: true,
            },
            Self::Push1 => InstructionProperties {
                execution_time: 0,
                base_energy_cost: 0,
                is_local: true,
            },
            Self::Move => InstructionProperties {
                execution_time: 1,
                base_energy_cost: 1,
                is_local: false,
            },
            Self::Add => InstructionProperties {
                execution_time: 0,
                base_energy_cost: 0,
                is_local: true,
            },
        }
    }

    pub const fn from_opcode(opcode: u8) -> Self {
        match opcode {
            0b00000000 => Self::Nop,
            0b00000001 => Self::Absorb,
            0b00000010 => Self::Push0,
            0b00000011 => Self::Push1,
            0b10000000 => Self::Move,
            0b10000001 => Self::Add,
            _ => panic!(),
        }
    }

    pub const fn to_opcode(&self) -> u8 {
        *self as u8
    }
}
