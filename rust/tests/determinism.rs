mod helpers;

use helpers::{diff_grids, run_ticks, ProgramBuilder, WorldBuilder};
use proteus::Simulation;

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
                .code(&[0x14, 0x50])
                .free_energy(4)
                .free_mass(1)
                .bg_radiation(2),
        )
        .at(
            1,
            0,
            ProgramBuilder::new().code(&[0x02, 0x54]).free_energy(6),
        )
        .at(
            2,
            0,
            ProgramBuilder::new().code(&[0x52, 0x50]).free_energy(1),
        )
        .at(
            4,
            0,
            ProgramBuilder::new()
                .code(&[0x51, 0x53, 0x50])
                .free_energy(2)
                .bg_radiation(3)
                .bg_mass(2),
        )
        .at(
            0,
            1,
            ProgramBuilder::new().code(&[0x01, 0x61]).free_energy(4),
        )
        .at(1, 1, ProgramBuilder::new().code(&[0x01, 0x62]).free_mass(4))
        .at(
            3,
            1,
            ProgramBuilder::new()
                .code(&[0x10, 0x11])
                .live(false)
                .open(true)
                .abandonment_timer(4),
        )
        .build_simulation()
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
