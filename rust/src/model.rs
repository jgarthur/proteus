use std::error::Error;
use std::fmt;

use crate::config::PROGRAM_SIZE_CAP;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Direction {
    #[default]
    Right = 0,
    Up = 1,
    Left = 2,
    Down = 3,
}

impl Direction {
    pub const ALL: [Self; 4] = [Self::Right, Self::Up, Self::Left, Self::Down];

    pub fn from_i16(value: i16) -> Self {
        match value.rem_euclid(4) {
            0 => Self::Right,
            1 => Self::Up,
            2 => Self::Left,
            _ => Self::Down,
        }
    }

    pub fn clockwise(self) -> Self {
        match self {
            Self::Right => Self::Down,
            Self::Up => Self::Right,
            Self::Left => Self::Up,
            Self::Down => Self::Left,
        }
    }

    pub fn counterclockwise(self) -> Self {
        match self {
            Self::Right => Self::Up,
            Self::Up => Self::Left,
            Self::Left => Self::Down,
            Self::Down => Self::Right,
        }
    }

    pub fn opposite(self) -> Self {
        match self {
            Self::Right => Self::Left,
            Self::Up => Self::Down,
            Self::Left => Self::Right,
            Self::Down => Self::Up,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Registers {
    pub ip: u16,
    pub dir: Direction,
    pub src: u16,
    pub dst: u16,
    pub flag: bool,
    pub msg: i16,
    pub id: u8,
    pub lc: i16,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TickState {
    pub absorb_count: u8,
    pub absorb_dir: Option<Direction>,
    pub did_listen: bool,
    pub did_collect: bool,
    pub did_nop: bool,
    pub is_open: bool,
    pub bg_radiation_consumed: u32,
    pub is_newborn: bool,
}

impl TickState {
    pub fn reset_for_new_tick(&mut self) {
        *self = Self::default();
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Program {
    pub code: Vec<u8>,
    pub registers: Registers,
    pub stack: Vec<i16>,
    pub live: bool,
    pub age: u32,
    pub abandonment_timer: u32,
    pub tick: TickState,
}

impl Program {
    pub fn new_live(code: Vec<u8>, dir: Direction, id: u8) -> Result<Self, ProgramError> {
        Self::new(code, dir, id, true)
    }

    pub fn new_inert(code: Vec<u8>, dir: Direction, id: u8) -> Result<Self, ProgramError> {
        Self::new(code, dir, id, false)
    }

    pub fn size(&self) -> u16 {
        u16::try_from(self.code.len())
            .expect("program size exceeds u16::MAX; constructors enforce the configured cap")
    }

    pub fn is_inert(&self) -> bool {
        !self.live
    }

    fn new(code: Vec<u8>, dir: Direction, id: u8, live: bool) -> Result<Self, ProgramError> {
        validate_code_size(code.len())?;

        let registers = Registers {
            dir,
            id,
            ..Registers::default()
        };

        Ok(Self {
            code,
            registers,
            stack: Vec::new(),
            live,
            age: 0,
            abandonment_timer: 0,
            tick: TickState::default(),
        })
    }
}

fn validate_code_size(len: usize) -> Result<(), ProgramError> {
    if len == 0 {
        return Err(ProgramError::EmptyCode);
    }
    if len > usize::from(PROGRAM_SIZE_CAP) {
        return Err(ProgramError::SizeCapExceeded {
            attempted: len,
            cap: usize::from(PROGRAM_SIZE_CAP),
        });
    }
    Ok(())
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProgramError {
    EmptyCode,
    SizeCapExceeded { attempted: usize, cap: usize },
}

impl fmt::Display for ProgramError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyCode => write!(f, "programs must contain at least one instruction"),
            Self::SizeCapExceeded { attempted, cap } => {
                write!(f, "program size {attempted} exceeds cap {cap}")
            }
        }
    }
}

impl Error for ProgramError {}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Cell {
    pub program: Option<Program>,
    pub free_energy: u32,
    pub free_mass: u32,
    pub bg_radiation: u32,
    pub bg_mass: u32,
}

impl Cell {
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn with_program(program: Program) -> Self {
        Self {
            program: Some(program),
            ..Self::default()
        }
    }

    pub fn has_program(&self) -> bool {
        self.program.is_some()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CellSnapshot {
    pub free_energy: u32,
    pub free_mass: u32,
    pub bg_radiation: u32,
    pub bg_mass: u32,
    pub program_size: u16,
    pub program_id: u8,
    pub has_program: bool,
}

impl From<&Cell> for CellSnapshot {
    fn from(cell: &Cell) -> Self {
        let (program_size, program_id, has_program) =
            cell.program.as_ref().map_or((0, 0, false), |program| {
                (program.size(), program.registers.id, true)
            });

        Self {
            free_energy: cell.free_energy,
            free_mass: cell.free_mass,
            bg_radiation: cell.bg_radiation,
            bg_mass: cell.bg_mass,
            program_size,
            program_id,
            has_program,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Packet {
    pub position: usize,
    pub direction: Direction,
    pub message: i16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QueuedAction {
    ReadAdj {
        source: usize,
        target: usize,
        src_cursor: u16,
    },
    WriteAdj {
        source: usize,
        target: usize,
        value: u8,
        dst_cursor: u16,
    },
    AppendAdj {
        source: usize,
        target: usize,
        value: u8,
    },
    DelAdj {
        source: usize,
        target: usize,
        dst_cursor: u16,
    },
    GiveE {
        source: usize,
        target: usize,
        amount: i16,
    },
    GiveM {
        source: usize,
        target: usize,
        amount: i16,
    },
    Move {
        source: usize,
        target: usize,
    },
    Boot {
        source: usize,
        target: usize,
    },
}

#[cfg(test)]
mod tests {
    use super::{Cell, CellSnapshot, Direction, Program, ProgramError};

    #[test]
    fn direction_rotation_matches_spec_clockwise_order() {
        assert_eq!(Direction::Right.clockwise(), Direction::Down);
        assert_eq!(Direction::Right.counterclockwise(), Direction::Up);
        assert_eq!(Direction::Up.opposite(), Direction::Down);
        assert_eq!(Direction::from_i16(-1), Direction::Down);
    }

    #[test]
    fn program_rejects_empty_code() {
        assert_eq!(
            Program::new_live(Vec::new(), Direction::Right, 7),
            Err(ProgramError::EmptyCode)
        );
    }

    #[test]
    fn snapshot_uses_zero_program_fields_for_empty_cells() {
        let snapshot = CellSnapshot::from(&Cell::empty());
        assert_eq!(snapshot.program_size, 0);
        assert_eq!(snapshot.program_id, 0);
        assert!(!snapshot.has_program);
    }
}
