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
            config.r_energy = 100.0;
            config.r_mass = 100.0;
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

    simulation.run_tick();
    let first_energy_arrival = simulation
        .grid()
        .get(simulation.grid().index(0, 0))
        .expect("cell should exist")
        .bg_radiation;
    let first_mass_arrival = simulation
        .grid()
        .get(simulation.grid().index(1, 0))
        .expect("cell should exist")
        .bg_mass;
    assert_eq!(
        simulation
            .grid()
            .get(simulation.grid().index(0, 0))
            .expect("cell should exist")
            .free_energy,
        0
    );
    assert_eq!(
        simulation
            .grid()
            .get(simulation.grid().index(1, 0))
            .expect("cell should exist")
            .free_mass,
        0
    );
    assert!(first_energy_arrival > 1);
    assert!(first_mass_arrival > 1);

    simulation.run_tick();
    let second_energy_cell = simulation
        .grid()
        .get(simulation.grid().index(0, 0))
        .expect("cell should exist");
    let second_mass_cell = simulation
        .grid()
        .get(simulation.grid().index(1, 0))
        .expect("cell should exist");
    let second_energy_arrival = second_energy_cell.bg_radiation;
    let second_mass_arrival = second_mass_cell.bg_mass;
    assert_eq!(second_energy_cell.free_energy, first_energy_arrival);
    assert_eq!(second_mass_cell.free_mass, first_mass_arrival);

    simulation.run_tick();
    let third_energy_cell = simulation
        .grid()
        .get(simulation.grid().index(0, 0))
        .expect("cell should exist");
    let third_mass_cell = simulation
        .grid()
        .get(simulation.grid().index(1, 0))
        .expect("cell should exist");
    assert_eq!(
        third_energy_cell.free_energy,
        first_energy_arrival + second_energy_arrival
    );
    assert_eq!(
        third_mass_cell.free_mass,
        first_mass_arrival + second_mass_arrival
    );
    assert!(third_energy_cell.bg_radiation > 1);
    assert!(third_mass_cell.bg_mass > 1);
}
