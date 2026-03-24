//! Orchestrates pass execution and owns long-lived simulation state.

use std::convert::TryFrom;
use std::error::Error;
use std::fmt;

use crate::config::{ConfigError, SimConfig};
use crate::grid::{Grid, GridError};
use crate::model::{CellSnapshot, Packet, QueuedAction};
use crate::pass1::{pass1_local, Pass1Output};
use crate::pass2::{pass2_nonlocal, Pass2Output};
use crate::pass3::{
    mutate_end_of_tick, pass3_ambient, pass3_packets, pass3_tail, Pass3AmbientOutput,
    Pass3TailContext,
};
use crate::random::{cell_rng, poisson};
const INITIAL_BG_RADIATION_SALT: u64 = 0x7400_f3bb_9241_b8d7;
const INITIAL_BG_MASS_SALT: u64 = 0x2f61_5dce_0840_13a9;
#[cfg(feature = "rayon")]
use rayon::prelude::*;

/// Owns reusable buffers that are rebuilt at the start of each tick.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TickScratch {
    snapshot: Vec<CellSnapshot>,
    live_set: Vec<bool>,
    existed_set: Vec<bool>,
}

impl TickScratch {
    /// Allocates the reusable per-tick scratch buffers for one grid size.
    pub fn new(cell_count: usize) -> Self {
        Self {
            snapshot: vec![CellSnapshot::default(); cell_count],
            live_set: vec![false; cell_count],
            existed_set: vec![false; cell_count],
        }
    }

    /// Returns the number of cells these scratch buffers are sized for.
    pub fn len(&self) -> usize {
        self.snapshot.len()
    }

    /// Reports whether the scratch buffers are empty.
    pub fn is_empty(&self) -> bool {
        self.snapshot.is_empty()
    }

    /// Returns the frozen Pass-0 cell snapshot for the prepared tick.
    pub fn snapshot(&self) -> &[CellSnapshot] {
        &self.snapshot
    }

    /// Returns the live-at-tick-start mask for the prepared tick.
    pub fn live_set(&self) -> &[bool] {
        &self.live_set
    }

    /// Returns the occupied-at-tick-start mask for the prepared tick.
    pub fn existed_set(&self) -> &[bool] {
        &self.existed_set
    }

    /// Resizes the reusable scratch buffers to match the current grid.
    fn resize(&mut self, cell_count: usize) {
        self.snapshot.resize(cell_count, CellSnapshot::default());
        self.live_set.resize(cell_count, false);
        self.existed_set.resize(cell_count, false);
    }
}

/// Owns one simulation instance, its packets, and the tick driver state.
#[derive(Clone, Debug, PartialEq)]
pub struct Simulation {
    grid: Grid,
    packets: Vec<Packet>,
    scratch: TickScratch,
    config: SimConfig,
    tick: u64,
    seed: u64,
}

/// Reports per-tick population and mutation statistics, including total births,
/// boot vs. spawn birth breakdown, deaths, mutations, and the packet count.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TickReport {
    pub births: u32,
    pub boot_births: u32,
    pub spawn_births: u32,
    pub deaths: u32,
    pub mutations: u32,
    pub packet_count: u32,
}

impl Simulation {
    /// Creates a simulation from a validated config and a fresh empty grid.
    pub fn new(config: SimConfig) -> Result<Self, SimulationError> {
        config.validate().map_err(SimulationError::InvalidConfig)?;

        let grid = Grid::new(config.width, config.height).map_err(SimulationError::Grid)?;
        let mut simulation = Self::from_grid(config, grid)?;
        let seed = simulation.seed;
        let config = simulation.config.clone();
        initialize_background_steady_state(&mut simulation.grid, &config, seed);
        Ok(simulation)
    }

    /// Creates a simulation from a config and an existing grid state.
    pub fn from_grid(config: SimConfig, grid: Grid) -> Result<Self, SimulationError> {
        config.validate().map_err(SimulationError::InvalidConfig)?;

        if grid.width() != config.width || grid.height() != config.height {
            return Err(SimulationError::GridShapeMismatch {
                config_width: config.width,
                config_height: config.height,
                grid_width: grid.width(),
                grid_height: grid.height(),
            });
        }

        let cell_count = config
            .cell_count()
            .expect("validated config must have a usable cell count");

        Ok(Self {
            grid,
            packets: Vec::new(),
            scratch: TickScratch::new(cell_count),
            seed: config.seed,
            config,
            tick: 0,
        })
    }

    /// Returns the immutable simulation config.
    pub fn config(&self) -> &SimConfig {
        &self.config
    }

    /// Returns the immutable world grid.
    pub fn grid(&self) -> &Grid {
        &self.grid
    }

    /// Returns the mutable world grid.
    pub fn grid_mut(&mut self) -> &mut Grid {
        &mut self.grid
    }

    /// Returns the persistent directed-radiation packets.
    pub fn packets(&self) -> &[Packet] {
        &self.packets
    }

    /// Appends externally supplied packets to the persistent packet list.
    pub fn extend_packets<I>(&mut self, packets: I)
    where
        I: IntoIterator<Item = Packet>,
    {
        self.packets.extend(packets);
    }

    /// Returns the current simulation tick counter.
    pub fn tick(&self) -> u64 {
        self.tick
    }

    /// Returns the master seed used for deterministic random draws.
    pub fn seed(&self) -> u64 {
        self.seed
    }

    /// Returns the reusable scratch buffers for the latest prepared tick.
    pub fn scratch(&self) -> &TickScratch {
        &self.scratch
    }

    /// Freezes the snapshot and start-of-tick masks used by the passes.
    pub fn prepare_tick(&mut self) -> PreparedTick<'_> {
        self.scratch.resize(self.grid.len());
        populate_prepared_tick(
            self.grid.cells(),
            &mut self.scratch.snapshot,
            &mut self.scratch.live_set,
            &mut self.scratch.existed_set,
        );

        PreparedTick {
            tick: self.tick,
            snapshot: &self.scratch.snapshot,
            live_set: &self.scratch.live_set,
            existed_set: &self.scratch.existed_set,
        }
    }

    /// Runs Pass 1 against the current grid state.
    pub fn run_pass1(&mut self) -> Pass1Output {
        self.prepare_tick();
        pass1_local(
            &mut self.grid,
            self.scratch.snapshot(),
            self.scratch.live_set(),
            &self.config,
            self.tick,
            self.seed,
        )
    }

    /// Runs Pass 2 for a supplied queue of nonlocal actions.
    pub fn run_pass2(&mut self, actions: &[QueuedAction]) -> Pass2Output {
        pass2_nonlocal(&mut self.grid, actions, self.tick, self.seed)
    }

    /// Runs the packet sub-phase of Pass 3 against the persistent packet list.
    pub fn run_pass3_packets(&mut self) {
        pass3_packets(&mut self.grid, &mut self.packets, self.tick, self.seed);
    }

    /// Runs the ambient-resource sub-phase of Pass 3.
    pub fn run_pass3_ambient(&mut self) -> Pass3AmbientOutput {
        pass3_ambient(&mut self.grid, &self.config, self.tick, self.seed)
    }

    /// Advances the simulation by one full tick and discards the report.
    pub fn run_tick(&mut self) {
        let _ = self.run_tick_report();
    }

    /// Advances the simulation by one full tick and returns observer counts.
    pub fn run_tick_report(&mut self) -> TickReport {
        self.prepare_tick();

        let pass1 = pass1_local(
            &mut self.grid,
            self.scratch.snapshot(),
            self.scratch.live_set(),
            &self.config,
            self.tick,
            self.seed,
        );
        let pass2 = pass2_nonlocal(&mut self.grid, &pass1.actions, self.tick, self.seed);
        self.packets.extend(pass1.emitted_packets);
        pass3_packets(&mut self.grid, &mut self.packets, self.tick, self.seed);
        let ambient = pass3_ambient(&mut self.grid, &self.config, self.tick, self.seed);
        let tail = pass3_tail(
            &mut self.grid,
            Pass3TailContext {
                existed_set: self.scratch.existed_set(),
                live_set: self.scratch.live_set(),
                incoming_writes: &pass2.incoming_writes,
                spawn_candidates: &ambient.spawn_candidates,
                config: &self.config,
                tick: self.tick,
                seed: self.seed,
            },
        );
        let mutations = mutate_end_of_tick(
            &mut self.grid,
            self.scratch.live_set(),
            &self.config,
            self.tick,
            self.seed,
        );
        clear_newborn_flags(&mut self.grid);
        self.tick = self.tick.wrapping_add(1);

        TickReport {
            // API-SPEC §10 treats any transition to live within the tick as a birth,
            // whether it came from boot or spontaneous spawn.
            births: pass2.booted_programs + tail.spontaneous_births,
            boot_births: pass2.booted_programs,
            spawn_births: tail.spontaneous_births,
            deaths: tail.deaths,
            mutations,
            packet_count: u32::try_from(self.packets.len()).unwrap_or(u32::MAX),
        }
    }
}

/// Exposes the frozen Pass-0 state for one prepared tick.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PreparedTick<'a> {
    pub tick: u64,
    pub snapshot: &'a [CellSnapshot],
    pub live_set: &'a [bool],
    pub existed_set: &'a [bool],
}

/// Clears the newborn marker once a full tick has completed.
fn clear_newborn_flags(grid: &mut Grid) {
    #[cfg(feature = "rayon")]
    {
        grid.cells_mut().par_iter_mut().for_each(|cell| {
            if let Some(program) = cell.program.as_mut() {
                program.tick.is_newborn = false;
            }
        });
    }

    #[cfg(not(feature = "rayon"))]
    {
        for cell in grid.cells_mut() {
            if let Some(program) = cell.program.as_mut() {
                program.tick.is_newborn = false;
            }
        }
    }
}

fn populate_prepared_tick(
    cells: &[crate::model::Cell],
    snapshot: &mut [CellSnapshot],
    live_set: &mut [bool],
    existed_set: &mut [bool],
) {
    #[cfg(feature = "rayon")]
    {
        snapshot
            .par_iter_mut()
            .zip(live_set.par_iter_mut())
            .zip(existed_set.par_iter_mut())
            .zip(cells.par_iter())
            .for_each(|(((snapshot_slot, live_slot), existed_slot), cell)| {
                *snapshot_slot = CellSnapshot::from(cell);
                *existed_slot = cell.program.is_some();
                *live_slot = cell
                    .program
                    .as_ref()
                    .is_some_and(|program| program.live && !program.tick.is_newborn);
            });
    }

    #[cfg(not(feature = "rayon"))]
    {
        for (((snapshot_slot, live_slot), existed_slot), cell) in snapshot
            .iter_mut()
            .zip(live_set.iter_mut())
            .zip(existed_set.iter_mut())
            .zip(cells.iter())
        {
            *snapshot_slot = CellSnapshot::from(cell);
            *existed_slot = cell.program.is_some();
            *live_slot = cell
                .program
                .as_ref()
                .is_some_and(|program| program.live && !program.tick.is_newborn);
        }
    }
}

/// Describes why a simulation could not be constructed or started.
#[derive(Clone, Debug, PartialEq)]
pub enum SimulationError {
    InvalidConfig(ConfigError),
    Grid(GridError),
    GridShapeMismatch {
        config_width: u32,
        config_height: u32,
        grid_width: u32,
        grid_height: u32,
    },
}

impl fmt::Display for SimulationError {
    /// Formats a human-readable simulation construction error.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidConfig(err) => write!(f, "{err}"),
            Self::Grid(err) => write!(f, "{err}"),
            Self::GridShapeMismatch {
                config_width,
                config_height,
                grid_width,
                grid_height,
            } => write!(
                f,
                "config expects {config_width}x{config_height} grid but received {grid_width}x{grid_height}"
            ),
        }
    }
}

impl Error for SimulationError {}

fn initialize_background_steady_state(grid: &mut Grid, config: &SimConfig, seed: u64) {
    for cell_index in 0..grid.len() {
        let cell = grid.get_mut(cell_index).expect("cell should exist");
        cell.bg_radiation = stationary_background_sample(
            seed ^ INITIAL_BG_RADIATION_SALT,
            cell_index as u64,
            config.r_energy,
            config.d_energy,
        );
        cell.bg_mass = stationary_background_sample(
            seed ^ INITIAL_BG_MASS_SALT,
            cell_index as u64,
            config.r_mass,
            config.d_mass,
        );
    }
}

fn stationary_background_sample(seed: u64, cell_index: u64, rate: f64, decay: f64) -> u32 {
    if rate <= 0.0 || decay <= 0.0 {
        return 0;
    }

    let mean = (rate / decay).min(f64::from(u32::MAX));
    let mut rng = cell_rng(seed, 0, cell_index);
    poisson(&mut rng, mean)
}

#[cfg(test)]
mod initialization_tests {
    use crate::config::SimConfig;
    use crate::grid::Grid;

    use super::{
        stationary_background_sample, Simulation, INITIAL_BG_MASS_SALT, INITIAL_BG_RADIATION_SALT,
    };

    #[test]
    fn new_simulation_seeds_background_from_stationary_distribution() {
        let config = SimConfig {
            width: 2,
            height: 1,
            seed: 7,
            r_energy: 4.0,
            r_mass: 3.0,
            d_energy: 0.5,
            d_mass: 0.25,
            ..SimConfig::default()
        };

        let simulation = Simulation::new(config.clone()).expect("simulation should build");

        for cell_index in 0..simulation.grid().len() {
            let cell = simulation
                .grid()
                .get(cell_index)
                .expect("cell should exist");
            assert_eq!(
                cell.bg_radiation,
                stationary_background_sample(
                    config.seed ^ INITIAL_BG_RADIATION_SALT,
                    cell_index as u64,
                    config.r_energy,
                    config.d_energy,
                )
            );
            assert_eq!(
                cell.bg_mass,
                stationary_background_sample(
                    config.seed ^ INITIAL_BG_MASS_SALT,
                    cell_index as u64,
                    config.r_mass,
                    config.d_mass,
                )
            );
        }
    }

    #[test]
    fn new_simulation_starts_without_background_when_decay_has_no_finite_steady_state() {
        let config = SimConfig {
            width: 2,
            height: 1,
            seed: 7,
            r_energy: 4.0,
            r_mass: 3.0,
            d_energy: 0.0,
            d_mass: 0.0,
            ..SimConfig::default()
        };

        let simulation = Simulation::new(config).expect("simulation should build");

        for cell in simulation.grid().cells() {
            assert_eq!(cell.bg_radiation, 0);
            assert_eq!(cell.bg_mass, 0);
        }
    }

    #[test]
    fn from_grid_preserves_explicit_background_state() {
        let config = SimConfig {
            width: 1,
            height: 1,
            r_energy: 4.0,
            r_mass: 3.0,
            d_energy: 0.5,
            d_mass: 0.25,
            ..SimConfig::default()
        };
        let grid = Grid::from_cells(
            1,
            1,
            vec![crate::Cell {
                bg_radiation: 9,
                bg_mass: 11,
                ..crate::Cell::default()
            }],
        )
        .expect("grid should build");

        let simulation = Simulation::from_grid(config, grid).expect("simulation should build");
        let cell = simulation.grid().get(0).expect("cell should exist");
        assert_eq!(cell.bg_radiation, 9);
        assert_eq!(cell.bg_mass, 11);
    }
}

#[cfg(test)]
mod tests {
    use super::{PreparedTick, Simulation, SimulationError};
    use crate::config::SimConfig;
    use crate::grid::Grid;
    use crate::model::{Cell, Direction, Program};
    use crate::opcode::op;

    #[test]
    fn simulation_prepares_snapshot_and_live_set_from_grid_state() {
        let config = SimConfig {
            width: 2,
            height: 1,
            ..SimConfig::default()
        };
        let mut cells = vec![Cell::default(), Cell::default()];

        let mut live = Program::new_live(vec![op::NOP, op::ABSORB], Direction::Right, 9)
            .expect("live program should build");
        live.tick.is_newborn = false;
        cells[0].program = Some(live);
        cells[0].free_energy = 7;
        cells[0].free_mass = 3;
        cells[0].bg_radiation = 5;
        cells[0].bg_mass = 2;

        let mut newborn = Program::new_live(vec![op::NOP], Direction::Up, 3)
            .expect("newborn program should build");
        newborn.tick.is_newborn = true;
        cells[1].program = Some(newborn);

        let grid = Grid::from_cells(2, 1, cells).expect("grid should build");
        let mut sim = Simulation::from_grid(config, grid).expect("simulation should build");

        let PreparedTick {
            tick,
            snapshot,
            live_set,
            existed_set,
        } = sim.prepare_tick();

        assert_eq!(tick, 0);
        assert_eq!(snapshot[0].free_energy, 7);
        assert_eq!(snapshot[0].free_mass, 3);
        assert_eq!(snapshot[0].bg_radiation, 5);
        assert_eq!(snapshot[0].bg_mass, 2);
        assert_eq!(snapshot[0].program_size, 2);
        assert_eq!(snapshot[0].program_id, 9);
        assert_eq!(live_set, &[true, false]);
        assert_eq!(existed_set, &[true, true]);
    }

    #[test]
    fn simulation_rejects_grid_shape_mismatch() {
        let config = SimConfig {
            width: 2,
            height: 2,
            ..SimConfig::default()
        };
        let grid = Grid::new(2, 1).expect("grid should build");

        let result = Simulation::from_grid(config, grid);
        assert_eq!(
            result,
            Err(SimulationError::GridShapeMismatch {
                config_width: 2,
                config_height: 2,
                grid_width: 2,
                grid_height: 1
            })
        );
    }
}
