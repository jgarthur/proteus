use rand::distributions::{Distribution, Standard};
use rand::Rng;

pub struct InstructionProperties {
    pub execution_time: u8,
    pub base_energy_cost: u8,
    pub is_local: bool,
    pub makes_passable: bool,
}

#[derive(Debug, Default, Clone, Copy, PartialEq)]
#[repr(u8)]
pub enum Instruction {
    #[default]
    Nop = 0b00000000,
    Move = 0b10000000,
    Clone = 0b00001000,
    Absorb = 0b00000001,
    Push0 = 0b00000010,
    Push1 = 0b00000011,
    Add = 0b10000001,
    CW = 0b11000000,
}

impl Instruction {
    pub const fn from_opcode(opcode: u8) -> Self {
        match opcode {
            0b00000000 => Self::Nop,
            0b10000000 => Self::Move,
            0b00001000 => Self::Clone,
            0b00000001 => Self::Absorb,
            0b00000010 => Self::Push0,
            0b00000011 => Self::Push1,
            0b10000001 => Self::Add,
            0b11000000 => Self::CW,
            _ => panic!(),
        }
    }

    pub const fn to_opcode(&self) -> u8 {
        *self as u8
    }

    pub const fn properties(&self) -> InstructionProperties {
        match self {
            Self::Nop => InstructionProperties {
                execution_time: 1,
                base_energy_cost: 0,
                is_local: true,
                makes_passable: true,
            },
            Self::Move => InstructionProperties {
                execution_time: 1,
                base_energy_cost: 1,
                is_local: false,
                makes_passable: false,
            },
            Self::Clone => InstructionProperties {
                execution_time: 1,
                base_energy_cost: 1,
                is_local: false,
                makes_passable: false,
            },
            Self::Absorb => InstructionProperties {
                execution_time: 1,
                base_energy_cost: 0,
                is_local: true,
                makes_passable: true,
            },
            Self::Push0 => InstructionProperties {
                execution_time: 0,
                base_energy_cost: 0,
                is_local: true,
                makes_passable: false,
            },
            Self::Push1 => InstructionProperties {
                execution_time: 0,
                base_energy_cost: 0,
                is_local: true,
                makes_passable: false,
            },
            Self::Add => InstructionProperties {
                execution_time: 0,
                base_energy_cost: 0,
                is_local: true,
                makes_passable: false,
            },
            Self::CW => InstructionProperties {
                execution_time: 0,
                base_energy_cost: 0,
                is_local: true,
                makes_passable: false,
            },
        }
    }

    pub const fn execution_time(&self) -> u8 {
        self.properties().execution_time
    }

    pub const fn base_energy_cost(&self) -> u8 {
        self.properties().base_energy_cost
    }

    pub const fn is_local(&self) -> bool {
        self.properties().is_local
    }

    pub const fn makes_passable(&self) -> bool {
        self.properties().makes_passable
    }
}

impl Distribution<Instruction> for Standard {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> Instruction {
        match rng.gen_range(0..=7) {
            0 => Instruction::Nop,
            1 => Instruction::Move,
            2 => Instruction::Clone,
            3 => Instruction::Absorb,
            4 => Instruction::Push0,
            5 => Instruction::Push1,
            6 => Instruction::Add,
            7 => Instruction::CW,
            _ => unreachable!(),
        }
    }
}
