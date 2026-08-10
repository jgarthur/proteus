//! Prints order-independent digests of full simulation replays.
//!
//! The serial and Rayon execution paths are mutually exclusive at compile time
//! (`#[cfg(feature = "rayon")]` / `#[cfg(not(...))]`), so no single test binary can
//! compare them. This example is built twice — once with default features and once
//! with `--features rayon` — and the digests are diffed. See
//! `scripts/check-rayon-parity.sh` and `docs/BACKEND-TESTING.md` section 6.
//!
//! Digests go to stdout so they can be diffed directly; build configuration goes to
//! stderr so it does not perturb the comparison.

use proteus::{op, Cell, Direction, Grid, Program, SimConfig, Simulation};

const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// The lithotroph the frontend seeds by default.
const LITHOTROPH: &[u8] = &[
    0x51, 0x51, 0x51, 0x51, 0x53, 0x40, 0x42, 0x30, 0x55, 0x5f, 0x31, 0x64,
];

fn fnv1a(bytes: &[u8], mut hash: u64) -> u64 {
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

/// Replays one fixture and prints digests of the grid, tick reports, and packets.
fn digest_fixture(name: &str, mut simulation: Simulation, ticks: u32) {
    let mut reports = FNV_OFFSET_BASIS;
    for _ in 0..ticks {
        let report = simulation.run_tick_report();
        reports = fnv1a(format!("{report:?}").as_bytes(), reports);
    }

    let mut grid = FNV_OFFSET_BASIS;
    for cell in simulation.grid().cells() {
        grid = fnv1a(format!("{cell:?}").as_bytes(), grid);
    }

    let packets = fnv1a(
        format!("{:?}", simulation.packets()).as_bytes(),
        FNV_OFFSET_BASIS,
    );

    println!(
        "{name:<16} ticks={ticks:<5} grid={grid:016x} reports={reports:016x} packets={packets:016x}"
    );
}

/// Builds a grid from a per-cell constructor and wraps it in a simulation.
fn build(config: SimConfig, cell_at: impl Fn(usize) -> Cell) -> Simulation {
    let cell_count = (config.width * config.height) as usize;
    let cells = (0..cell_count).map(cell_at).collect();
    let grid = Grid::from_cells(config.width, config.height, cells)
        .expect("parity fixture should produce a valid grid");
    Simulation::from_grid(config, grid).expect("parity fixture should produce a valid simulation")
}

/// Places one live program with the supplied code and starting resources.
fn program_cell(code: &[u8], dir: Direction, id: u8, free_energy: u32, free_mass: u32) -> Cell {
    Cell {
        program: Some(
            Program::new_live(code.to_vec(), dir, id).expect("fixture program should be valid"),
        ),
        free_energy,
        free_mass,
        ..Cell::default()
    }
}

const MIXED_CODES: [&[u8]; 6] = [
    &[op::RAND, op::NOP],
    &[op::push(2), op::EMIT],
    &[op::LISTEN, op::NOP],
    &[op::ABSORB, op::COLLECT, op::NOP],
    &[op::push(1), op::GIVE_E],
    &[op::push(1), op::GIVE_M],
];

const DIRS: [Direction; 4] = [
    Direction::Up,
    Direction::Down,
    Direction::Left,
    Direction::Right,
];

/// Sparse 8x8 grid with aggressive spawn and mutation rates.
///
/// Small enough that Rayon splits into tiny chunks, and the only fixture where
/// spontaneous creation fires often.
fn sparse_8x8() -> Simulation {
    let config = SimConfig {
        width: 8,
        height: 8,
        seed: 0x5eed,
        r_energy: 0.45,
        r_mass: 0.35,
        d_energy: 0.2,
        d_mass: 0.15,
        maintenance_rate: 0.25,
        maintenance_exponent: 1.0,
        p_spawn: 0.5,
        mutation_base_log2: 3,
        mutation_background_log2: 1,
        ..SimConfig::default()
    };

    build(config, |index| {
        if index % 5 != 0 {
            return Cell::default();
        }
        let pick = index % MIXED_CODES.len();
        program_cell(
            MIXED_CODES[pick],
            DIRS[index % DIRS.len()],
            pick as u8,
            (index % 9) as u32,
            (index % 6) as u32,
        )
    })
}

/// Moderate-density 64x64 replay of the frontend's default lithotroph seeding.
///
/// Every `SimConfig` field is set explicitly, so this fixture is hermetic against
/// changes to `SimConfig::default()`. Adding a config field will stop this compiling,
/// which is the intended prompt to decide what the parity fixture should use.
fn frontend_64x64() -> Simulation {
    let config = SimConfig {
        width: 64,
        height: 64,
        seed: 1,
        r_energy: 0.25,
        r_mass: 1.0,
        d_energy: 0.01,
        d_mass: 0.01,
        t_cap: 4.0,
        maintenance_rate: 0.007_812_5,
        maintenance_exponent: 1.0,
        local_action_exponent: 1.0,
        n_synth: 1,
        inert_grace_ticks: 10,
        p_spawn: 0.0,
        mutation_base_log2: 16,
        mutation_background_log2: 8,
    };

    let seeds = [(28_u32, 28_u32), (36, 28), (28, 36), (36, 36)];
    build(config, |index| {
        let (x, y) = (index as u32 % 64, index as u32 / 64);
        if seeds.contains(&(x, y)) {
            program_cell(LITHOTROPH, Direction::Right, 0, 20, 12)
        } else {
            Cell::default()
        }
    })
}

/// Dense 32x32 grid where every cell holds a program, roughly a tenth of them inert.
///
/// This is the highest-contention fixture: every parallel chunk does real work.
fn dense_32x32() -> Simulation {
    let config = SimConfig {
        width: 32,
        height: 32,
        seed: 0xdead_beef,
        r_energy: 0.6,
        r_mass: 0.6,
        d_energy: 0.05,
        d_mass: 0.05,
        maintenance_rate: 0.02,
        maintenance_exponent: 1.0,
        local_action_exponent: 1.0,
        p_spawn: 0.25,
        inert_grace_ticks: 3,
        mutation_base_log2: 4,
        mutation_background_log2: 2,
        ..SimConfig::default()
    };

    build(config, |index| {
        let pick = index % MIXED_CODES.len();
        let mut cell = program_cell(
            MIXED_CODES[pick],
            DIRS[index % DIRS.len()],
            pick as u8,
            (index % 9) as u32,
            (index % 6) as u32,
        );
        cell.bg_radiation = (index % 4) as u32;
        cell.bg_mass = (index % 3) as u32;
        if let Some(program) = cell.program.as_mut() {
            program.live = index % 11 != 0;
        }
        cell
    })
}

fn main() {
    let ticks: u32 = std::env::args()
        .nth(1)
        .and_then(|value| value.parse().ok())
        .unwrap_or(300);

    eprintln!(
        "parity digest: rayon={} RAYON_NUM_THREADS={}",
        cfg!(feature = "rayon"),
        std::env::var("RAYON_NUM_THREADS").unwrap_or_else(|_| "unset".to_owned()),
    );

    digest_fixture("sparse-8x8", sparse_8x8(), ticks);
    digest_fixture("frontend-64x64", frontend_64x64(), ticks);
    digest_fixture("dense-32x32", dense_32x32(), ticks);
}
