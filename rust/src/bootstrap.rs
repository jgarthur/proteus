//! Defines deterministic initial-world program placement and resource preloads.

use std::collections::HashSet;
use std::error::Error;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::config::{SimConfig, PROGRAM_SIZE_CAP};
use crate::model::{Direction, Lineage, Program, ProgramOrigin, ProgramUid};
use crate::random::cell_rng;
use crate::Simulation;

/// Identifies the bootstrap RNG contract persisted in runner provenance.
pub const BOOTSTRAP_RNG_VERSION: &str = "seed-program-v1";
/// Separates initial program register draws from all simulation-tick RNG streams.
pub const BOOTSTRAP_RNG_SALT: u64 = 0x0d7e_8ef0_4268_33c1;

/// Holds exact program placements and environmental resource preloads.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BootstrapConfig {
    pub programs: Vec<SeedProgram>,
    pub environment: Vec<EnvironmentPreload>,
}

impl BootstrapConfig {
    /// Validates bootstrap coordinates, program sizes, and duplicate targets.
    pub fn validate(&self, config: &SimConfig) -> Result<(), BootstrapError> {
        let mut program_cells = HashSet::new();
        for program in &self.programs {
            validate_coordinates(program.x, program.y, config)?;
            if program.code.is_empty() {
                return Err(BootstrapError::EmptyProgram {
                    x: program.x,
                    y: program.y,
                });
            }
            if program.code.len() > usize::from(PROGRAM_SIZE_CAP) {
                return Err(BootstrapError::ProgramTooLarge {
                    x: program.x,
                    y: program.y,
                    size: program.code.len(),
                });
            }
            if !program_cells.insert((program.x, program.y)) {
                return Err(BootstrapError::DuplicateProgram {
                    x: program.x,
                    y: program.y,
                });
            }
        }

        let mut environment_cells = HashSet::new();
        for preload in &self.environment {
            validate_coordinates(preload.x, preload.y, config)?;
            if !environment_cells.insert((preload.x, preload.y)) {
                return Err(BootstrapError::DuplicateEnvironment {
                    x: preload.x,
                    y: preload.y,
                });
            }
        }
        Ok(())
    }

    /// Returns a canonical copy with independent entries sorted by `(y, x)`.
    pub fn normalized(&self) -> Self {
        let mut normalized = self.clone();
        normalized.programs.sort_by_key(|entry| (entry.y, entry.x));
        normalized
            .environment
            .sort_by_key(|entry| (entry.y, entry.x));
        normalized
    }
}

/// Describes one live program placed at tick 0.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SeedProgram {
    pub x: u32,
    pub y: u32,
    pub code: Vec<u8>,
    pub free_energy: u32,
    pub free_mass: u32,
}

/// Replaces all four resource pools on one cell before program placement.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnvironmentPreload {
    pub x: u32,
    pub y: u32,
    pub free_energy: u32,
    pub free_mass: u32,
    pub bg_radiation: u32,
    pub bg_mass: u32,
}

/// Applies exact environment preloads followed by deterministic program placement.
pub fn apply_bootstrap(
    simulation: &mut Simulation,
    bootstrap: &BootstrapConfig,
) -> Result<(), BootstrapError> {
    bootstrap.validate(simulation.config())?;

    for preload in &bootstrap.environment {
        let index = simulation.grid().index(preload.x, preload.y);
        let cell = simulation
            .grid_mut()
            .get_mut(index)
            .expect("validated bootstrap cell should exist");
        cell.free_energy = preload.free_energy;
        cell.free_mass = preload.free_mass;
        cell.bg_radiation = preload.bg_radiation;
        cell.bg_mass = preload.bg_mass;
    }

    let seed = simulation.seed();
    let cell_count = simulation.grid().len();
    for seed_program in &bootstrap.programs {
        let index = simulation.grid().index(seed_program.x, seed_program.y);
        let mut rng = cell_rng(seed ^ BOOTSTRAP_RNG_SALT, 0, index as u64);
        let direction = Direction::ALL[(rng.next_u32() % Direction::ALL.len() as u32) as usize];
        let id = rng.next_u32() as u8;
        // Bootstrap runs before tick 0 and seeds are always live, so a seed
        // program is a lineage root born at tick 0 (SPEC.md: bootstrap programs
        // are not births).
        let uid = ProgramUid::create(0, index, cell_count, ProgramOrigin::Seed);
        let program = Program::new_live(seed_program.code.clone(), direction, id)
            .map_err(|error| BootstrapError::Program(error.to_string()))?
            .with_lineage(Lineage::root(uid, 0));
        let cell = simulation
            .grid_mut()
            .get_mut(index)
            .expect("validated bootstrap cell should exist");
        cell.program = Some(program);
        cell.free_energy = seed_program.free_energy;
        cell.free_mass = seed_program.free_mass;
    }
    Ok(())
}

fn validate_coordinates(x: u32, y: u32, config: &SimConfig) -> Result<(), BootstrapError> {
    if x >= config.width || y >= config.height {
        return Err(BootstrapError::OutOfBounds {
            x,
            y,
            width: config.width,
            height: config.height,
        });
    }
    Ok(())
}

/// Describes an invalid bootstrap definition.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BootstrapError {
    OutOfBounds {
        x: u32,
        y: u32,
        width: u32,
        height: u32,
    },
    EmptyProgram {
        x: u32,
        y: u32,
    },
    ProgramTooLarge {
        x: u32,
        y: u32,
        size: usize,
    },
    DuplicateProgram {
        x: u32,
        y: u32,
    },
    DuplicateEnvironment {
        x: u32,
        y: u32,
    },
    Program(String),
}

impl fmt::Display for BootstrapError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OutOfBounds {
                x,
                y,
                width,
                height,
            } => write!(
                formatter,
                "bootstrap cell ({x}, {y}) is outside the {width}x{height} grid"
            ),
            Self::EmptyProgram { x, y } => {
                write!(formatter, "seed program at ({x}, {y}) must not be empty")
            }
            Self::ProgramTooLarge { x, y, size } => write!(
                formatter,
                "seed program at ({x}, {y}) has size {size}, exceeding {PROGRAM_SIZE_CAP}"
            ),
            Self::DuplicateProgram { x, y } => {
                write!(formatter, "multiple seed programs target ({x}, {y})")
            }
            Self::DuplicateEnvironment { x, y } => {
                write!(formatter, "multiple environment preloads target ({x}, {y})")
            }
            Self::Program(message) => formatter.write_str(message),
        }
    }
}

impl Error for BootstrapError {}

#[cfg(test)]
mod tests {
    use super::{apply_bootstrap, BootstrapConfig, EnvironmentPreload, SeedProgram};
    use crate::model::{Lineage, ProgramOrigin, ProgramSite, ProgramUid};
    use crate::observe::inspect_cell;
    use crate::{SimConfig, Simulation};

    /// Returns the lineage of the program seeded into one cell.
    fn lineage_at(simulation: &Simulation, index: usize) -> Lineage {
        simulation
            .grid()
            .get(index)
            .expect("cell should exist")
            .program
            .as_ref()
            .expect("cell should hold a seed program")
            .lineage
    }

    #[test]
    fn seed_programs_are_tick_zero_lineage_roots_with_distinct_uids() {
        let config = SimConfig {
            width: 2,
            height: 2,
            r_energy: 0.0,
            r_mass: 0.0,
            ..SimConfig::default()
        };
        let mut simulation = Simulation::new(config).expect("simulation should build");
        let bootstrap = BootstrapConfig {
            programs: vec![
                SeedProgram {
                    x: 0,
                    y: 0,
                    code: vec![80],
                    free_energy: 20,
                    free_mass: 12,
                },
                SeedProgram {
                    x: 1,
                    y: 1,
                    code: vec![80, 100],
                    free_energy: 20,
                    free_mass: 12,
                },
            ],
            environment: Vec::new(),
        };

        apply_bootstrap(&mut simulation, &bootstrap).expect("bootstrap should apply");

        let cell_count = simulation.grid().len();
        for (index, expected_cell_index) in [(0_usize, 0_usize), (3, 3)] {
            let lineage = lineage_at(&simulation, index);
            assert_eq!(lineage.parent, ProgramUid::NONE, "seeds are lineage roots");
            assert_eq!(lineage.generation, 0);
            assert_eq!(
                lineage.birth_tick(),
                Some(0),
                "bootstrap runs before tick 0 and seeds are always live"
            );
            assert_eq!(
                lineage.uid.site(cell_count),
                Some(ProgramSite {
                    tick: 0,
                    cell_index: expected_cell_index,
                    origin: ProgramOrigin::Seed,
                })
            );
        }

        assert_ne!(
            lineage_at(&simulation, 0).uid,
            lineage_at(&simulation, 3).uid,
            "seeds in different cells must get different uids"
        );
    }

    #[test]
    fn program_resources_override_environment_free_pools_only() {
        let config = SimConfig {
            width: 1,
            height: 1,
            r_energy: 0.0,
            r_mass: 0.0,
            ..SimConfig::default()
        };
        let mut simulation = Simulation::new(config).expect("simulation should build");
        let bootstrap = BootstrapConfig {
            programs: vec![SeedProgram {
                x: 0,
                y: 0,
                code: vec![80],
                free_energy: 20,
                free_mass: 12,
            }],
            environment: vec![EnvironmentPreload {
                x: 0,
                y: 0,
                free_energy: 99,
                free_mass: 98,
                bg_radiation: 7,
                bg_mass: 6,
            }],
        };

        apply_bootstrap(&mut simulation, &bootstrap).expect("bootstrap should apply");
        let cell = simulation.grid().get(0).expect("cell should exist");
        assert_eq!(cell.free_energy, 20);
        assert_eq!(cell.free_mass, 12);
        assert_eq!(cell.bg_radiation, 7);
        assert_eq!(cell.bg_mass, 6);
        assert!(cell.program.is_some());
    }

    #[test]
    fn independent_entry_order_does_not_change_the_tick_zero_world() {
        let config = SimConfig {
            width: 2,
            height: 2,
            r_energy: 0.0,
            r_mass: 0.0,
            ..SimConfig::default()
        };
        let first = BootstrapConfig {
            programs: vec![
                SeedProgram {
                    x: 1,
                    y: 1,
                    code: vec![80, 100],
                    free_energy: 20,
                    free_mass: 12,
                },
                SeedProgram {
                    x: 0,
                    y: 0,
                    code: vec![0],
                    free_energy: 3,
                    free_mass: 2,
                },
            ],
            environment: vec![
                EnvironmentPreload {
                    x: 1,
                    y: 0,
                    free_energy: 4,
                    free_mass: 5,
                    bg_radiation: 6,
                    bg_mass: 7,
                },
                EnvironmentPreload {
                    x: 0,
                    y: 1,
                    free_energy: 8,
                    free_mass: 9,
                    bg_radiation: 10,
                    bg_mass: 11,
                },
            ],
        };
        let mut reversed = first.clone();
        reversed.programs.reverse();
        reversed.environment.reverse();

        let mut first_simulation =
            Simulation::new(config.clone()).expect("simulation should build");
        let mut reversed_simulation = Simulation::new(config).expect("simulation should build");
        apply_bootstrap(&mut first_simulation, &first).expect("bootstrap should apply");
        apply_bootstrap(&mut reversed_simulation, &reversed).expect("bootstrap should apply");

        let first_cells = (0..4)
            .map(|index| inspect_cell(first_simulation.grid(), index))
            .collect::<Vec<_>>();
        let reversed_cells = (0..4)
            .map(|index| inspect_cell(reversed_simulation.grid(), index))
            .collect::<Vec<_>>();
        assert_eq!(first_cells, reversed_cells);
    }
}
