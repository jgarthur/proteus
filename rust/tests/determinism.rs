mod helpers;

use helpers::{diff_grids, run_ticks, ProgramBuilder, WorldBuilder};
use proteus::op;
use proteus::Simulation;
#[cfg(feature = "rayon")]
use proteus::{Direction, TickReport};
#[cfg(feature = "rayon")]
use rayon::ThreadPoolBuilder;
#[cfg(feature = "rayon")]
use std::time::Instant;

fn build_deterministic_fixture() -> Simulation {
    WorldBuilder::new(5, 2)
        .seed(0x5eed)
        .configure(|config| {
            config.r_energy = 0.45;
            config.r_mass = 0.35;
            config.d_energy = 0.2;
            config.d_mass = 0.15;
            config.maintenance_rate = 0.25;
            config.maintenance_exponent = 1.0;
            config.p_spawn = 0.5;
            config.mutation_base_log2 = 3;
            config.mutation_background_log2 = 1;
        })
        .at(
            0,
            0,
            ProgramBuilder::new()
                .code(&[op::RAND, op::NOP])
                .free_energy(4)
                .free_mass(1)
                .bg_radiation(2),
        )
        .at(
            1,
            0,
            ProgramBuilder::new()
                .code(&[op::push(2), op::EMIT])
                .free_energy(6),
        )
        .at(
            2,
            0,
            ProgramBuilder::new()
                .code(&[op::LISTEN, op::NOP])
                .free_energy(1),
        )
        .at(
            4,
            0,
            ProgramBuilder::new()
                .code(&[op::ABSORB, op::COLLECT, op::NOP])
                .free_energy(2)
                .bg_radiation(3)
                .bg_mass(2),
        )
        .at(
            0,
            1,
            ProgramBuilder::new()
                .code(&[op::push(1), op::GIVE_E])
                .free_energy(4),
        )
        .at(
            1,
            1,
            ProgramBuilder::new()
                .code(&[op::push(1), op::GIVE_M])
                .free_mass(4),
        )
        .at(
            3,
            1,
            ProgramBuilder::new()
                .code(&[op::DUP, op::DROP])
                .live(false)
                .open(true)
                .abandonment_timer(4),
        )
        .build_simulation()
}

#[cfg(feature = "rayon")]
const FRONTEND_DEFAULT_LITHOTROPH_CODE: &[u8] = &[
    0x51, 0x51, 0x51, 0x51, 0x53, 0x40, 0x42, 0x30, 0x55, 0x5f, 0x31, 0x64,
];

#[cfg(feature = "rayon")]
fn run_tick_reports(simulation: &mut Simulation, ticks: u32) -> Vec<TickReport> {
    (0..ticks).map(|_| simulation.run_tick_report()).collect()
}

#[cfg(feature = "rayon")]
fn frontend_seed_benchmark_ticks() -> u32 {
    std::env::var("PROTEUS_FRONTEND_SEED_TICKS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(1_000)
}

#[test]
fn run_tick_replay_is_deterministic_for_same_seed_and_initial_state() {
    let mut left = build_deterministic_fixture();
    let mut right = build_deterministic_fixture();

    run_ticks(&mut left, 20);
    run_ticks(&mut right, 20);

    let diffs = diff_grids(left.grid(), right.grid());
    assert!(
        diffs.is_empty(),
        "grid divergence after deterministic replay: {diffs:#?}"
    );
    assert_eq!(left.packets(), right.packets(), "packet streams diverged");
    assert_eq!(left.tick(), right.tick(), "tick counters diverged");
}

#[cfg(feature = "rayon")]
fn run_simulation_in_pool<F>(
    thread_count: usize,
    builder: F,
    ticks: u32,
) -> (Simulation, Vec<TickReport>)
where
    F: Fn() -> Simulation + Send + Sync,
{
    ThreadPoolBuilder::new()
        .num_threads(thread_count)
        .build()
        .expect("rayon thread pool should build")
        .install(|| {
            let mut simulation = builder();
            let reports = run_tick_reports(&mut simulation, ticks);
            (simulation, reports)
        })
}

#[cfg(feature = "rayon")]
fn build_packet_order_fixture() -> Simulation {
    WorldBuilder::new(3, 1)
        .seed(0x4a11_ce55)
        .at(
            0,
            0,
            ProgramBuilder::new()
                .code(&[op::push(11), op::EMIT])
                .dir(Direction::Right)
                .free_energy(2),
        )
        .at(
            1,
            0,
            ProgramBuilder::new()
                .code(&[op::LISTEN])
                .dir(Direction::Up)
                .free_energy(1),
        )
        .at(
            2,
            0,
            ProgramBuilder::new()
                .code(&[op::push(22), op::EMIT])
                .dir(Direction::Left)
                .free_energy(2),
        )
        .build_simulation()
}

#[cfg(feature = "rayon")]
fn build_frontend_seed_fixture() -> Simulation {
    let seeds = [(28, 28), (36, 28), (28, 36), (36, 36)];
    let mut builder = WorldBuilder::new(64, 64).seed(1).configure(|config| {
        config.r_energy = 0.25;
        config.r_mass = 1.0;
        config.d_energy = 0.01;
        config.d_mass = 0.01;
        config.t_cap = 4.0;
        config.maintenance_rate = 0.0078125;
        config.maintenance_exponent = 1.0;
        config.local_action_exponent = 1.0;
        config.n_synth = 1;
        config.inert_grace_ticks = 10;
        config.p_spawn = 0.0;
        config.mutation_base_log2 = 16;
        config.mutation_background_log2 = 8;
    });

    for (x, y) in seeds {
        builder = builder.at(
            x,
            y,
            ProgramBuilder::new()
                .code(FRONTEND_DEFAULT_LITHOTROPH_CODE)
                .free_energy(20)
                .free_mass(12),
        );
    }

    builder.build_simulation()
}

#[cfg(feature = "rayon")]
#[test]
fn rayon_thread_counts_preserve_full_tick_determinism() {
    let (baseline_simulation, baseline_reports) =
        run_simulation_in_pool(1, build_deterministic_fixture, 20);

    for thread_count in [2, 4, 8] {
        let (candidate_simulation, candidate_reports) =
            run_simulation_in_pool(thread_count, build_deterministic_fixture, 20);

        let diffs = diff_grids(baseline_simulation.grid(), candidate_simulation.grid());
        assert!(
            diffs.is_empty(),
            "grid divergence with rayon thread count {thread_count}: {diffs:#?}"
        );
        assert_eq!(
            baseline_simulation.packets(),
            candidate_simulation.packets(),
            "packet streams diverged with rayon thread count {thread_count}"
        );
        assert_eq!(
            baseline_simulation.tick(),
            candidate_simulation.tick(),
            "tick counters diverged with rayon thread count {thread_count}"
        );
        assert_eq!(
            baseline_reports, candidate_reports,
            "tick reports diverged with rayon thread count {thread_count}"
        );
    }
}

#[cfg(feature = "rayon")]
#[test]
fn listener_packet_capture_is_stable_across_rayon_thread_counts() {
    let (baseline_simulation, baseline_reports) =
        run_simulation_in_pool(1, build_packet_order_fixture, 1);
    let baseline_listener = baseline_simulation
        .grid()
        .get(baseline_simulation.grid().index(1, 0))
        .expect("listener cell should exist")
        .clone();

    for thread_count in [2, 4, 8] {
        let (candidate_simulation, candidate_reports) =
            run_simulation_in_pool(thread_count, build_packet_order_fixture, 1);
        let candidate_listener = candidate_simulation
            .grid()
            .get(candidate_simulation.grid().index(1, 0))
            .expect("listener cell should exist");

        assert_eq!(
            &baseline_listener, candidate_listener,
            "listener state diverged with rayon thread count {thread_count}"
        );
        assert_eq!(
            baseline_simulation.packets(),
            candidate_simulation.packets(),
            "packet survivors diverged with rayon thread count {thread_count}"
        );
        assert_eq!(
            baseline_reports, candidate_reports,
            "tick reports diverged with rayon thread count {thread_count}"
        );
    }
}

#[cfg(feature = "rayon")]
#[test]
fn frontend_default_seed_ecology_is_deterministic_for_long_rayon_replay() {
    let ticks = frontend_seed_benchmark_ticks();

    let baseline_start = Instant::now();
    let (baseline_simulation, baseline_reports) =
        run_simulation_in_pool(1, build_frontend_seed_fixture, ticks);
    let baseline_elapsed = baseline_start.elapsed();

    let candidate_start = Instant::now();
    let (candidate_simulation, candidate_reports) =
        run_simulation_in_pool(4, build_frontend_seed_fixture, ticks);
    let candidate_elapsed = candidate_start.elapsed();

    eprintln!(
        "frontend-seed determinism benchmark: ticks={ticks} single_thread={baseline_elapsed:?} rayon_4={candidate_elapsed:?} speedup={:.2}x",
        baseline_elapsed.as_secs_f64() / candidate_elapsed.as_secs_f64()
    );

    let diffs = diff_grids(baseline_simulation.grid(), candidate_simulation.grid());
    assert!(
        diffs.is_empty(),
        "frontend-seed grid divergence after {ticks} ticks under rayon: {diffs:#?}"
    );
    assert_eq!(
        baseline_simulation.packets(),
        candidate_simulation.packets(),
        "frontend-seed packet streams diverged after {ticks} ticks"
    );
    assert_eq!(
        baseline_simulation.tick(),
        candidate_simulation.tick(),
        "frontend-seed tick counters diverged after {ticks} ticks"
    );
    assert_eq!(
        baseline_reports, candidate_reports,
        "frontend-seed tick reports diverged after {ticks} ticks"
    );
}
