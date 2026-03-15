#![forbid(unsafe_code)]

pub mod config;
pub mod grid;
pub mod model;
pub mod opcode;
pub mod random;
pub mod simulation;

pub use config::{ConfigError, SimConfig, PROGRAM_SIZE_CAP, SPEC_VERSION};
pub use grid::{Grid, GridError};
pub use model::{
    Cell, CellSnapshot, Direction, Packet, Program, ProgramError, QueuedAction, Registers,
    TickState,
};
pub use opcode::{AdditionalCost, Locality, Opcode, SPEC_OPCODE_COUNT};
pub use random::{binomial, cell_rng, splitmix64, WyRand};
pub use simulation::{PreparedTick, Simulation, SimulationError, TickScratch};
