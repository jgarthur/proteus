#![forbid(unsafe_code)]

pub mod config;
pub mod grid;
pub mod model;
pub mod opcode;
pub mod pass1;
pub mod pass2;
pub mod pass3;
pub mod random;
pub mod simulation;

pub use config::{ConfigError, SimConfig, PROGRAM_SIZE_CAP, SPEC_VERSION};
pub use grid::{Grid, GridError};
pub use model::{
    Cell, CellSnapshot, Direction, Packet, Program, ProgramError, QueuedAction, Registers,
    TickState,
};
pub use opcode::{AdditionalCost, Locality, Opcode, SPEC_OPCODE_COUNT};
pub use pass1::{local_action_budget, pass1_local, Pass1Output};
pub use pass2::{pass2_nonlocal, Pass2Output};
pub use pass3::{
    mutate_end_of_tick, pass3_ambient, pass3_packets, pass3_tail, Pass3AmbientOutput,
    Pass3TailContext,
};
pub use random::{binomial, cell_rng, splitmix64, WyRand};
pub use simulation::{PreparedTick, Simulation, SimulationError, TickScratch};
