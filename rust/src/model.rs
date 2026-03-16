//! Defines the core world, program, and queued-action data structures.

use std::error::Error;
use std::fmt;

use crate::config::PROGRAM_SIZE_CAP;

/// Represents one of the four cardinal directions used throughout the world.
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
    /// Lists all cardinal directions in spec encoding order.
    pub const ALL: [Self; 4] = [Self::Right, Self::Up, Self::Left, Self::Down];

    /// Converts an arbitrary signed value into a wrapped direction.
    pub fn from_i16(value: i16) -> Self {
        match value.rem_euclid(4) {
            0 => Self::Right,
            1 => Self::Up,
            2 => Self::Left,
            _ => Self::Down,
        }
    }

    /// Rotates a direction one step clockwise.
    pub fn clockwise(self) -> Self {
        match self {
            Self::Right => Self::Down,
            Self::Up => Self::Right,
            Self::Left => Self::Up,
            Self::Down => Self::Left,
        }
    }

    /// Rotates a direction one step counterclockwise.
    pub fn counterclockwise(self) -> Self {
        match self {
            Self::Right => Self::Up,
            Self::Up => Self::Left,
            Self::Left => Self::Down,
            Self::Down => Self::Right,
        }
    }

    /// Returns the opposite cardinal direction.
    pub fn opposite(self) -> Self {
        match self {
            Self::Right => Self::Left,
            Self::Up => Self::Down,
            Self::Left => Self::Right,
            Self::Down => Self::Up,
        }
    }
}

/// Stores the mutable registers carried by one program.
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

/// Stores per-tick transient flags and counters for one program.
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
    /// Clears all transient tick state back to defaults.
    pub fn reset_for_new_tick(&mut self) {
        *self = Self::default();
    }

    /// Prepares the transient state for the start of Pass 1.
    pub fn reset_for_pass1(&mut self, is_inert: bool) {
        let is_newborn = self.is_newborn;
        *self = Self {
            is_open: is_inert,
            is_newborn,
            ..Self::default()
        };
    }
}

/// Represents one program occupying a cell.
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
    /// Builds a live program with initialized registers and empty transient state.
    pub fn new_live(code: Vec<u8>, dir: Direction, id: u8) -> Result<Self, ProgramError> {
        Self::new(code, dir, id, true)
    }

    /// Builds an inert program with initialized registers and empty transient state.
    pub fn new_inert(code: Vec<u8>, dir: Direction, id: u8) -> Result<Self, ProgramError> {
        Self::new(code, dir, id, false)
    }

    /// Returns the program length as a validated `u16`.
    pub fn size(&self) -> u16 {
        u16::try_from(self.code.len())
            .expect("program size exceeds u16::MAX; constructors enforce the configured cap")
    }

    /// Reports whether the program is currently inert.
    pub fn is_inert(&self) -> bool {
        !self.live
    }

    /// Builds a program after enforcing the shared constructor invariants.
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

/// Validates that a program length fits the spec constraints.
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

/// Describes why a program could not be constructed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProgramError {
    EmptyCode,
    SizeCapExceeded { attempted: usize, cap: usize },
}

impl fmt::Display for ProgramError {
    /// Formats a human-readable program construction error.
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

/// Stores the full mutable state for one world cell.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Cell {
    pub program: Option<Program>,
    pub free_energy: u32,
    pub free_mass: u32,
    pub bg_radiation: u32,
    pub bg_mass: u32,
}

impl Cell {
    /// Builds an empty cell with no program or resources.
    pub fn empty() -> Self {
        Self::default()
    }

    /// Builds a cell that starts with a program and default resources.
    pub fn with_program(program: Program) -> Self {
        Self {
            program: Some(program),
            ..Self::default()
        }
    }

    /// Reports whether the cell currently contains a program.
    pub fn has_program(&self) -> bool {
        self.program.is_some()
    }
}

/// Captures the immutable per-cell view used during Pass 1 sensing.
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
    /// Projects a cell into the snapshot fields read by Pass 1.
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

/// Represents one directed radiation packet moving between cells.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Packet {
    pub position: usize,
    pub direction: Direction,
    pub message: i16,
}

/// Represents one nonlocal action queued during Pass 1 for Pass 2 resolution.
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
