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

use proteus::{
    op, Cell, Direction, Grid, Opcode, Program, SimConfig, Simulation, SPEC_OPCODE_COUNT,
};

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
    let mut births = 0_u64;
    let mut boot_births = 0_u64;
    let mut spawn_births = 0_u64;
    let mut deaths = 0_u64;
    let mut mutations = 0_u64;
    let mut max_packets = 0_u32;
    for _ in 0..ticks {
        let report = simulation.run_tick_report();
        // Hash the whole report. Every `TickReport` field must reach this digest:
        // it is the only serial-vs-Rayon comparison in the repo, so a field left
        // out is a field whose two accumulations are never compared. Adding a
        // field therefore changes `reports=` by design — reconcile that against
        // the `grid=` / `packets=` digests and the totals below, which do not
        // depend on `TickReport`'s Debug layout.
        reports = fnv1a(format!("{report:?}").as_bytes(), reports);
        births += u64::from(report.births);
        boot_births += u64::from(report.boot_births);
        spawn_births += u64::from(report.spawn_births);
        deaths += u64::from(report.deaths);
        mutations += u64::from(report.mutations);
        max_packets = max_packets.max(report.packet_count);
    }

    let mut grid = FNV_OFFSET_BASIS;
    for cell in simulation.grid().cells() {
        grid = fnv1a(format!("{cell:?}").as_bytes(), grid);
    }

    let packets = fnv1a(
        format!("{:?}", simulation.packets()).as_bytes(),
        FNV_OFFSET_BASIS,
    );
    let final_programs = simulation
        .grid()
        .cells()
        .iter()
        .filter(|cell| cell.program.is_some())
        .count();
    let final_live_programs = simulation
        .grid()
        .cells()
        .iter()
        .filter(|cell| cell.program.as_ref().is_some_and(|program| program.live))
        .count();

    println!(
        "{name:<16} ticks={ticks:<5} grid={grid:016x} reports={reports:016x} packets={packets:016x} births={births} boot={boot_births} spawn={spawn_births} deaths={deaths} mutations={mutations} max_packets={max_packets} final_programs={final_programs} final_live={final_live_programs}"
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
        d_energy_log2: Some(2),
        d_mass_log2: Some(3),
        maintenance_rate_log2: Some(2),
        maintenance_exponent: 1.0,
        p_spawn_log2: Some(1),
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
        d_energy_log2: Some(7),
        d_mass_log2: Some(7),
        t_cap: 4.0,
        maintenance_rate_log2: Some(7),
        maintenance_exponent: 1.0,
        local_action_exponent: 1.0,
        n_synth: 1,
        inert_grace_ticks: 10,
        p_spawn_log2: None,
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
        d_energy_log2: Some(4),
        d_mass_log2: Some(4),
        maintenance_rate_log2: Some(6),
        maintenance_exponent: 1.0,
        local_action_exponent: 1.0,
        p_spawn_log2: Some(2),
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

/// Moving 8x1 grid where every program relocates into an empty neighbor.
///
/// Exercises the program-carried tick-start eligibility used by maintenance, aging,
/// and mutation after Pass 2 changes a program's cell index.
fn moving_8x1() -> Simulation {
    let config = SimConfig {
        width: 8,
        height: 1,
        seed: 0x110e,
        r_energy: 0.0,
        r_mass: 0.0,
        d_energy_log2: None,
        d_mass_log2: None,
        maintenance_rate_log2: Some(2),
        mutation_base_log2: 3,
        mutation_background_log2: 3,
        ..SimConfig::default()
    };

    build(config, |index| {
        if index.is_multiple_of(2) {
            program_cell(&[op::MOVE, op::NOP], Direction::Right, index as u8, 32, 2)
        } else {
            Cell::default()
        }
    })
}

/// One live carrier for every spec-defined opcode byte.
///
/// Every carrier is eligible on tick 0, has enough stack operands and resources
/// to enter its dispatch arm, and cannot die from maintenance. This is an
/// execution census, while the focused integration tests cover branch outcomes.
fn all_opcodes_71x1() -> Simulation {
    let opcode_bytes = (u8::MIN..=u8::MAX)
        .filter(|byte| !Opcode::decode(*byte).is_noop())
        .collect::<Vec<_>>();
    assert_eq!(opcode_bytes.len(), SPEC_OPCODE_COUNT);

    let config = SimConfig {
        width: u32::try_from(opcode_bytes.len()).expect("opcode count should fit in u32"),
        height: 1,
        seed: 0xc0de_ce05,
        r_energy: 0.0,
        r_mass: 0.0,
        d_energy_log2: None,
        d_mass_log2: None,
        maintenance_rate_log2: None,
        p_spawn_log2: None,
        mutation_base_log2: 31,
        mutation_background_log2: 31,
        ..SimConfig::default()
    };

    build(config, |index| {
        let mut cell = program_cell(
            &[opcode_bytes[index]],
            Direction::Right,
            index as u8,
            1_000,
            1_000,
        );
        cell.bg_radiation = 1_000;
        cell.bg_mass = 1_000;
        cell.program
            .as_mut()
            .expect("opcode carrier should contain a program")
            .stack = vec![1, 2, 3, 4];
        cell
    })
}

/// Successful representatives of every Pass 2 action plus one mixed conflict.
fn exclusive_actions_5x9() -> Simulation {
    let config = SimConfig {
        width: 5,
        height: 9,
        seed: 0xe7c1_051e,
        r_energy: 0.0,
        r_mass: 0.0,
        d_energy_log2: None,
        d_mass_log2: None,
        maintenance_rate_log2: None,
        p_spawn_log2: None,
        mutation_base_log2: 31,
        mutation_background_log2: 31,
        ..SimConfig::default()
    };

    build(config, |index| {
        let x = index % 5;
        let y = index / 5;
        match (x, y) {
            (1, 0) => program_cell(&[op::READ_ADJ], Direction::Right, 1, 10, 10),
            (2, 0) => program_cell(&[op::DUP], Direction::Right, 2, 0, 0),
            (1, 1) => program_cell(&[op::push(7), op::WRITE_ADJ], Direction::Right, 3, 10, 10),
            (2, 1) => program_cell(&[op::NOP], Direction::Right, 4, 0, 0),
            (1, 2) => program_cell(&[op::push(7), op::APPEND_ADJ], Direction::Right, 5, 10, 10),
            (2, 2) => program_cell(&[op::NOP], Direction::Right, 6, 0, 0),
            (1, 3) => program_cell(&[op::DEL_ADJ], Direction::Right, 7, 10, 10),
            (2, 3) => program_cell(&[op::NOP, op::NOP], Direction::Right, 8, 2, 0),
            (1, 4) => program_cell(&[op::push(3), op::GIVE_E], Direction::Right, 9, 10, 10),
            (2, 4) => program_cell(&[op::NOP], Direction::Right, 10, 0, 0),
            (1, 5) => program_cell(&[op::push(3), op::GIVE_M], Direction::Right, 11, 10, 10),
            (2, 5) => program_cell(&[op::NOP], Direction::Right, 12, 0, 0),
            (1, 6) => program_cell(&[op::MOVE], Direction::Right, 13, 10, 2),
            (1, 7) => program_cell(&[op::BOOT], Direction::Right, 14, 10, 10),
            (2, 7) => Cell {
                program: Some(
                    Program::new_inert(vec![op::NOP], Direction::Right, 15)
                        .expect("boot target should be valid"),
                ),
                ..Cell::default()
            },
            (1, 8) => program_cell(&[op::push(1), op::WRITE_ADJ], Direction::Right, 16, 10, 10),
            (2, 8) => program_cell(&[op::NOP], Direction::Right, 17, 0, 0),
            (3, 8) => program_cell(&[op::push(2), op::APPEND_ADJ], Direction::Left, 18, 10, 10),
            _ => Cell::default(),
        }
    })
}

/// Proves the exclusive-action fixture reaches the intended success/conflict paths.
fn verify_exclusive_fixture() {
    let mut simulation = exclusive_actions_5x9();
    let report = simulation.run_tick_report();
    let cell = |x, y| {
        simulation
            .grid()
            .get(simulation.grid().index(x, y))
            .expect("exclusive fixture cell should exist")
    };

    assert_eq!(
        cell(1, 0)
            .program
            .as_ref()
            .expect("read source should survive")
            .stack,
        vec![i16::from(op::DUP)]
    );
    assert_eq!(
        cell(2, 1)
            .program
            .as_ref()
            .expect("write target should survive")
            .code,
        vec![7]
    );
    assert_eq!(
        cell(2, 2)
            .program
            .as_ref()
            .expect("append target should survive")
            .code,
        vec![op::NOP, 7]
    );
    assert_eq!(
        cell(2, 3)
            .program
            .as_ref()
            .expect("delete target should survive")
            .code
            .len(),
        1
    );
    assert_eq!(cell(2, 4).free_energy, 3);
    assert_eq!(cell(2, 5).free_mass, 3);
    assert!(cell(1, 6).program.is_none());
    assert!(cell(2, 6).program.is_some());
    assert!(cell(2, 7)
        .program
        .as_ref()
        .is_some_and(|program| program.live));
    assert_eq!(report.boot_births, 1);
    assert_ne!(
        cell(2, 8)
            .program
            .as_ref()
            .expect("conflict target should survive")
            .code,
        vec![op::NOP]
    );
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

    verify_exclusive_fixture();

    digest_fixture("sparse-8x8", sparse_8x8(), ticks);
    digest_fixture("frontend-64x64", frontend_64x64(), ticks);
    digest_fixture("dense-32x32", dense_32x32(), ticks);
    digest_fixture("moving-8x1", moving_8x1(), ticks);
    digest_fixture("all-opcodes-71x1", all_opcodes_71x1(), ticks);
    digest_fixture("exclusive-5x9", exclusive_actions_5x9(), ticks);
}
