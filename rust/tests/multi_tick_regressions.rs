#[macro_use]
mod helpers;

use helpers::{run_ticks, ProgramBuilder, WorldBuilder};
use proteus::op;
use proteus::{Direction, Packet};

#[test]
fn packet_wraps_toroidally_under_full_run_tick() {
    let mut simulation = WorldBuilder::new(3, 1)
        .configure(|config| {
            config.r_energy = 0.0;
            config.r_mass = 0.0;
            config.d_energy = 0.0;
            config.d_mass = 0.0;
            config.maintenance_rate = 0.0;
            config.p_spawn = 0.0;
            config.mutation_base_log2 = 32;
            config.mutation_background_log2 = 32;
        })
        .build_simulation();
    simulation.extend_packets([Packet {
        position: 0,
        direction: Direction::Right,
        message: 9,
    }]);

    run_ticks(&mut simulation, 3);

    assert_eq!(
        simulation.packets(),
        &[Packet {
            position: 0,
            direction: Direction::Right,
            message: 9,
        }]
    );
}

#[test]
fn absorb_and_collect_accumulate_with_one_tick_arrival_lag() {
    let mut simulation = WorldBuilder::new(2, 1)
        .configure(|config| {
            config.r_energy = 1.0;
            config.r_mass = 1.0;
            config.d_energy = 0.0;
            config.d_mass = 0.0;
            config.maintenance_rate = 0.0;
            config.p_spawn = 0.0;
            config.mutation_base_log2 = 32;
            config.mutation_background_log2 = 32;
        })
        .at(0, 0, ProgramBuilder::new().code(&[op::ABSORB, op::NOP]))
        .at(1, 0, ProgramBuilder::new().code(&[op::COLLECT, op::NOP]))
        .build_simulation();

    run_ticks(&mut simulation, 3);

    assert_cell!(
        simulation.grid(),
        (0, 0),
        free_energy == 2,
        bg_radiation == 1
    );
    assert_cell!(simulation.grid(), (1, 0), free_mass == 2, bg_mass == 1);
}
