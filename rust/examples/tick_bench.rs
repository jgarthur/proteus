//! Benchmarks whole-tick and per-phase timings for representative workloads.
//!
//! Build and run this under both feature configurations to compare the true
//! serial build against the Rayon build (a one-thread Rayon pool is not a
//! serial baseline):
//!
//! ```bash
//! cargo run --release --example tick_bench
//! RAYON_NUM_THREADS=4 cargo run --release --features rayon --example tick_bench
//! ```
//!
//! Usage: `tick_bench [ticks] [repetitions] [fixture-substring] [start-tick]`.
//! `start-tick` replays each fixture without observation before timing begins,
//! which makes it possible to benchmark a bounded window of a grown ecology.
//!
//! Fixtures include the synthetic cases from
//! `docs/analysis/2026-08-11_rayon-performance-profile.md` plus the exact
//! 256x256 web scenario recorded in the 2026-08-12 results note. Timings are
//! wall-clock means and are machine-specific; treat them as guidance, not as
//! stable thresholds.

use std::time::{Duration, Instant};

use proteus::observe::{collect_census, collect_metrics};
use proteus::{
    apply_bootstrap, op, BootstrapConfig, Cell, Direction, EventTotals, Grid, Program, SeedProgram,
    SimConfig, Simulation, TickObserver, TickPhase, TickReport,
};

/// The lithotroph the frontend seeds by default.
const LITHOTROPH: &[u8] = &[
    0x51, 0x51, 0x51, 0x51, 0x53, 0x40, 0x42, 0x30, 0x55, 0x5f, 0x31, 0x64,
];

const DIRS: [Direction; 4] = [
    Direction::Up,
    Direction::Down,
    Direction::Left,
    Direction::Right,
];

/// Accumulates per-phase durations across observed ticks.
struct PhaseSums {
    sums: [Duration; TickPhase::ALL.len()],
    ticks: u64,
}

impl PhaseSums {
    fn new() -> Self {
        Self {
            sums: [Duration::ZERO; TickPhase::ALL.len()],
            ticks: 0,
        }
    }

    fn mean_micros(&self, phase: TickPhase) -> f64 {
        if self.ticks == 0 {
            return 0.0;
        }
        self.sums[phase as usize].as_secs_f64() * 1e6 / self.ticks as f64
    }
}

impl TickObserver for PhaseSums {
    const ENABLED: bool = true;

    fn record(&mut self, phase: TickPhase, elapsed: Duration) {
        self.sums[phase as usize] += elapsed;
        if phase == TickPhase::Total {
            self.ticks += 1;
        }
    }
}

/// Builds a grid from a per-cell constructor and wraps it in a simulation.
fn build(config: SimConfig, cell_at: impl Fn(usize) -> Cell) -> Simulation {
    let cell_count = (config.width * config.height) as usize;
    let cells = (0..cell_count).map(cell_at).collect();
    let grid = Grid::from_cells(config.width, config.height, cells)
        .expect("bench fixture should produce a valid grid");
    Simulation::from_grid(config, grid).expect("bench fixture should produce a valid simulation")
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

/// Returns the frontend-style ambient rates used by most fixtures.
fn frontend_config(width: u32, height: u32, seed: u64) -> SimConfig {
    SimConfig {
        width,
        height,
        seed,
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
    }
}

/// Moderate-density 64x64 replay of the frontend's default lithotroph seeding.
fn frontend_64x64() -> Simulation {
    let seeds = [(28_u32, 28_u32), (36, 28), (28, 36), (36, 36)];
    build(frontend_config(64, 64, 1), |index| {
        let (x, y) = (index as u32 % 64, index as u32 / 64);
        if seeds.contains(&(x, y)) {
            program_cell(LITHOTROPH, Direction::Right, 0, 20, 12)
        } else {
            Cell::default()
        }
    })
}

/// Exact 256x256 scenario exported by the web frontend on 2026-08-12.
///
/// Unlike the synthetic fixtures built with `Simulation::from_grid`, this uses
/// the production initialization and bootstrap path so tick-zero background
/// resources and randomized program registers match the web controller.
fn web_256x256_single() -> Simulation {
    let config = frontend_config(256, 256, 6_846_702_536_457_205);
    let mut simulation = Simulation::new(config).expect("web fixture should build");
    apply_bootstrap(
        &mut simulation,
        &BootstrapConfig {
            programs: vec![SeedProgram {
                x: 32,
                y: 32,
                code: LITHOTROPH.to_vec(),
                free_energy: 20,
                free_mass: 12,
            }],
            environment: Vec::new(),
        },
    )
    .expect("web fixture bootstrap should apply");
    simulation
}

/// Empty grid isolating full-grid ambient work and fixed scheduling costs.
fn empty(width: u32, height: u32) -> Simulation {
    build(frontend_config(width, height, 3), |_| Cell::default())
}

/// Dense additive 128x128: every cell queues one `GIVE_E` per tick, no exclusives.
///
/// Everyone gives one energy rightward and receives one from the left, so free
/// resources hold steady; maintenance is off to keep the workload stationary.
fn dense_additive_128x128() -> Simulation {
    let config = SimConfig {
        maintenance_rate_log2: None,
        mutation_base_log2: 24,
        ..frontend_config(128, 128, 5)
    };
    build(config, |index| {
        program_cell(
            &[op::push(1), op::GIVE_E],
            Direction::Right,
            index as u8,
            8,
            4,
        )
    })
}

/// Dense emit 64x64: every cell emits one packet per tick, stressing the
/// serial packet phase with thousands of live packets and collisions.
fn dense_emit_64x64() -> Simulation {
    let config = SimConfig {
        maintenance_rate_log2: None,
        mutation_base_log2: 24,
        ..frontend_config(64, 64, 7)
    };
    build(config, |index| {
        program_cell(
            &[op::push(2), op::EMIT],
            DIRS[index % DIRS.len()],
            index as u8,
            4,
            2,
        )
    })
}

/// Sums tick reports so fixture activity can be sanity-checked in the output.
#[derive(Default)]
struct ReportTotals {
    births: u64,
    deaths: u64,
    mutations: u64,
    final_packets: u32,
    final_cells: usize,
    start_programs: usize,
    start_live_programs: usize,
    final_programs: usize,
    final_live_programs: usize,
}

impl ReportTotals {
    fn absorb(&mut self, report: &TickReport) {
        self.births += u64::from(report.births);
        self.deaths += u64::from(report.deaths);
        self.mutations += u64::from(report.mutations);
        self.final_packets = report.packet_count;
    }
}

fn program_counts(simulation: &Simulation) -> (usize, usize) {
    let programs = simulation
        .grid()
        .cells()
        .iter()
        .filter(|cell| cell.program.is_some())
        .count();
    let live_programs = simulation
        .grid()
        .cells()
        .iter()
        .filter(|cell| cell.program.as_ref().is_some_and(|program| program.live))
        .count();
    (programs, live_programs)
}

fn run_fixture(name: &str, make: &dyn Fn() -> Simulation, ticks: u32, reps: u32, start_tick: u32) {
    // Warm up code paths and the Rayon pool outside the measured window.
    let mut warmup = make();
    for _ in 0..ticks.min(100) {
        warmup.run_tick_report();
    }

    let mut sums = PhaseSums::new();
    let mut rep_totals = Vec::with_capacity(reps as usize);
    let mut totals = ReportTotals::default();
    let mut final_simulation = None;
    let mut checkpoint = make();
    for _ in 0..start_tick {
        checkpoint.run_tick();
    }
    (totals.start_programs, totals.start_live_programs) = program_counts(&checkpoint);
    println!(
        "checkpoint ready: tick={start_tick} programs={} live_programs={}",
        totals.start_programs, totals.start_live_programs
    );
    for _ in 0..reps {
        let mut simulation = checkpoint.clone();
        let mut rep_sums = PhaseSums::new();
        for _ in 0..ticks {
            let report = simulation.run_tick_report_observed(&mut rep_sums);
            totals.absorb(&report);
        }
        (totals.final_programs, totals.final_live_programs) = program_counts(&simulation);
        totals.final_cells = simulation.grid().len();
        rep_totals.push(rep_sums.mean_micros(TickPhase::Total));
        for phase in TickPhase::ALL {
            sums.sums[phase as usize] += rep_sums.sums[phase as usize];
        }
        sums.ticks += rep_sums.ticks;
        final_simulation = Some(simulation);
    }

    println!("fixture={name} start_tick={start_tick} ticks={ticks} reps={reps}");
    for phase in TickPhase::ALL {
        println!(
            "  {:<14} {:>10.1} us/tick",
            phase.label(),
            sums.mean_micros(phase)
        );
    }
    let reps_line = rep_totals
        .iter()
        .map(|mean| format!("{mean:.1}"))
        .collect::<Vec<_>>()
        .join(" ");
    println!("  rep total means: {reps_line}");
    println!(
        "  activity: births={} deaths={} mutations={} final_packets={} start_programs={} start_live_programs={} start_occupancy={:.1}% final_programs={} final_live_programs={} final_occupancy={:.1}%",
        totals.births,
        totals.deaths,
        totals.mutations,
        totals.final_packets,
        totals.start_programs,
        totals.start_live_programs,
        totals.start_programs as f64 * 100.0 / totals.final_cells as f64,
        totals.final_programs,
        totals.final_live_programs,
        totals.final_programs as f64 * 100.0 / totals.final_cells as f64,
    );

    if let Some(simulation) = final_simulation.as_ref() {
        report_census(simulation);
    }
}

/// Times one census against one metrics snapshot and reports bucket saturation.
///
/// Strictly a one-shot post-run probe: it runs after the measured window closes,
/// so it cannot perturb the tick timings above. It exists to answer two
/// questions on a real ecology - what a sample costs, and whether the fixed
/// histogram widths are wide enough that overflow bins stay near-empty.
fn report_census(simulation: &Simulation) {
    let metrics_start = Instant::now();
    let metrics = collect_metrics(
        simulation.grid(),
        0,
        simulation.tick(),
        TickReport::default(),
        EventTotals::default(),
    );
    let metrics_elapsed = metrics_start.elapsed();

    let census_start = Instant::now();
    let census = collect_census(simulation.grid());
    let census_elapsed = census_start.elapsed();

    println!(
        "  sample cost: collect_metrics={:.0} us collect_census={:.0} us population={}",
        metrics_elapsed.as_secs_f64() * 1e6,
        census_elapsed.as_secs_f64() * 1e6,
        metrics.population,
    );
    println!(
        "  census overflow: live_sizes={}/{} inert_sizes={}/{} generation={}/{} offspring={}/{} stack_depths={}/{}",
        census.live_sizes.overflow,
        census.live_sizes.count,
        census.inert_sizes.overflow,
        census.inert_sizes.count,
        census.lineage.generation.overflow,
        census.lineage.generation.count,
        census.lineage.offspring.overflow,
        census.lineage.offspring.count,
        census.stack_depths.overflow,
        census.stack_depths.count,
    );
    let stack = &census.stack_depths;
    let populated: Vec<String> = stack
        .counts
        .iter()
        .enumerate()
        .filter(|(_, count)| **count > 0)
        .map(|(index, count)| {
            let label = if index == 0 {
                "0".to_owned()
            } else {
                format!("2^{}", index - 1)
            };
            format!("{label}:{count}")
        })
        .collect();
    let first = stack.counts.iter().position(|count| *count > 0);
    let last = stack.counts.iter().rposition(|count| *count > 0);
    println!(
        "  stack_depths log2 buckets [{}..{}] of {}: {}",
        first.map_or("-".to_owned(), |index| index.to_string()),
        last.map_or("-".to_owned(), |index| index.to_string()),
        stack.counts.len(),
        populated.join(" "),
    );
    println!(
        "  census maxima: live_size={} inert_size={} generation={} offspring={} stack_depth={} roots={} orphans={}",
        census.live_sizes.max,
        census.inert_sizes.max,
        census.lineage.generation.max,
        census.lineage.offspring.max,
        census.stack_depths.max,
        census.lineage.roots,
        census.lineage.orphans,
    );
}

fn main() {
    let mut args = std::env::args().skip(1);
    let ticks: u32 = args
        .next()
        .and_then(|value| value.parse().ok())
        .unwrap_or(1000);
    let reps: u32 = args
        .next()
        .and_then(|value| value.parse().ok())
        .unwrap_or(5);
    let filter = args.next().unwrap_or_default();
    let start_tick: u32 = args
        .next()
        .and_then(|value| value.parse().ok())
        .unwrap_or(0);

    if cfg!(debug_assertions) {
        eprintln!("warning: debug assertions are enabled; use --release for stable numbers");
    }
    println!(
        "tick_bench: rayon={} RAYON_NUM_THREADS={} start_tick={start_tick} ticks={ticks} reps={reps}",
        cfg!(feature = "rayon"),
        std::env::var("RAYON_NUM_THREADS").unwrap_or_else(|_| "unset".to_owned()),
    );

    let fixtures: [(&str, &dyn Fn() -> Simulation); 6] = [
        ("frontend-64x64", &frontend_64x64),
        ("web-256x256-single", &web_256x256_single),
        ("empty-64x64", &|| empty(64, 64)),
        ("empty-256x256", &|| empty(256, 256)),
        ("dense-additive-128x128", &dense_additive_128x128),
        ("dense-emit-64x64", &dense_emit_64x64),
    ];

    for (name, make) in fixtures {
        if !filter.is_empty() && !name.contains(&filter) {
            continue;
        }
        run_fixture(name, make, ticks, reps, start_tick);
    }
}
