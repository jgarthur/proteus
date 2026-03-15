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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TickScratch {
    snapshot: Vec<CellSnapshot>,
    live_set: Vec<bool>,
    existed_set: Vec<bool>,
}

impl TickScratch {
    pub fn new(cell_count: usize) -> Self {
        Self {
            snapshot: vec![CellSnapshot::default(); cell_count],
            live_set: vec![false; cell_count],
            existed_set: vec![false; cell_count],
        }
    }

    pub fn len(&self) -> usize {
        self.snapshot.len()
    }

    pub fn is_empty(&self) -> bool {
        self.snapshot.is_empty()
    }

    pub fn snapshot(&self) -> &[CellSnapshot] {
        &self.snapshot
    }

    pub fn live_set(&self) -> &[bool] {
        &self.live_set
    }

    pub fn existed_set(&self) -> &[bool] {
        &self.existed_set
    }

    fn resize(&mut self, cell_count: usize) {
        self.snapshot.resize(cell_count, CellSnapshot::default());
        self.live_set.resize(cell_count, false);
        self.existed_set.resize(cell_count, false);
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Simulation {
    grid: Grid,
    packets: Vec<Packet>,
    scratch: TickScratch,
    config: SimConfig,
    tick: u64,
    seed: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TickReport {
    pub births: u32,
    pub deaths: u32,
    pub mutations: u32,
}

impl Simulation {
    pub fn new(config: SimConfig) -> Result<Self, SimulationError> {
        config.validate().map_err(SimulationError::InvalidConfig)?;

        let grid = Grid::new(config.width, config.height).map_err(SimulationError::Grid)?;
        Self::from_grid(config, grid)
    }

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

    pub fn config(&self) -> &SimConfig {
        &self.config
    }

    pub fn grid(&self) -> &Grid {
        &self.grid
    }

    pub fn grid_mut(&mut self) -> &mut Grid {
        &mut self.grid
    }

    pub fn packets(&self) -> &[Packet] {
        &self.packets
    }

    pub fn extend_packets<I>(&mut self, packets: I)
    where
        I: IntoIterator<Item = Packet>,
    {
        self.packets.extend(packets);
    }

    pub fn tick(&self) -> u64 {
        self.tick
    }

    pub fn seed(&self) -> u64 {
        self.seed
    }

    pub fn scratch(&self) -> &TickScratch {
        &self.scratch
    }

    pub fn prepare_tick(&mut self) -> PreparedTick<'_> {
        self.scratch.resize(self.grid.len());

        for ((snapshot, live_slot), cell) in self
            .scratch
            .snapshot
            .iter_mut()
            .zip(self.scratch.live_set.iter_mut())
            .zip(self.scratch.existed_set.iter_mut())
            .zip(self.grid.cells().iter())
        {
            let ((snapshot, live_slot), existed_slot) = (snapshot, live_slot);
            *snapshot = CellSnapshot::from(cell);
            *existed_slot = cell.program.is_some();
            *live_slot = cell
                .program
                .as_ref()
                .is_some_and(|program| program.live && !program.tick.is_newborn);
        }

        PreparedTick {
            tick: self.tick,
            snapshot: &self.scratch.snapshot,
            live_set: &self.scratch.live_set,
            existed_set: &self.scratch.existed_set,
        }
    }

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

    pub fn run_pass2(&mut self, actions: &[QueuedAction]) -> Pass2Output {
        pass2_nonlocal(&mut self.grid, actions, self.tick, self.seed)
    }

    pub fn run_pass3_packets(&mut self) {
        pass3_packets(&mut self.grid, &mut self.packets, self.tick, self.seed);
    }

    pub fn run_pass3_ambient(&mut self) -> Pass3AmbientOutput {
        pass3_ambient(&mut self.grid, &self.config, self.tick, self.seed)
    }

    pub fn run_tick(&mut self) {
        let _ = self.run_tick_report();
    }

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
            deaths: tail.deaths,
            mutations,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PreparedTick<'a> {
    pub tick: u64,
    pub snapshot: &'a [CellSnapshot],
    pub live_set: &'a [bool],
    pub existed_set: &'a [bool],
}

fn clear_newborn_flags(grid: &mut Grid) {
    for cell in grid.cells_mut() {
        if let Some(program) = cell.program.as_mut() {
            program.tick.is_newborn = false;
        }
    }
}

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

#[cfg(test)]
mod tests {
    use super::{PreparedTick, Simulation, SimulationError};
    use crate::config::SimConfig;
    use crate::grid::Grid;
    use crate::model::{Cell, Direction, Program};

    #[test]
    fn simulation_prepares_snapshot_and_live_set_from_grid_state() {
        let config = SimConfig {
            width: 2,
            height: 1,
            ..SimConfig::default()
        };
        let mut cells = vec![Cell::default(), Cell::default()];

        let mut live = Program::new_live(vec![0x50, 0x51], Direction::Right, 9)
            .expect("live program should build");
        live.tick.is_newborn = false;
        cells[0].program = Some(live);
        cells[0].free_energy = 7;
        cells[0].free_mass = 3;
        cells[0].bg_radiation = 5;
        cells[0].bg_mass = 2;

        let mut newborn =
            Program::new_live(vec![0x50], Direction::Up, 3).expect("newborn program should build");
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
