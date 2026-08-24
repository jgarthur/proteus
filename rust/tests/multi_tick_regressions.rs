#[macro_use]
mod helpers;

use helpers::{run_ticks, ProgramBuilder, WorldBuilder};
use proteus::op;
use proteus::{Direction, EventTotals, Packet};

#[test]
fn packet_wraps_toroidally_under_full_run_tick() {
    let mut simulation = WorldBuilder::new(3, 1)
        .configure(|config| {
            config.r_energy = 0.0;
            config.r_mass = 0.0;
            config.d_energy_log2 = None;
            config.d_mass_log2 = None;
            config.maintenance_rate_log2 = None;
            config.p_spawn_log2 = None;
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
            config.d_energy_log2 = None;
            config.d_mass_log2 = None;
            config.maintenance_rate_log2 = None;
            config.p_spawn_log2 = None;
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

#[test]
fn mutation_cause_totals_sum_over_multiple_real_ticks() {
    let mut simulation = WorldBuilder::new(2, 1)
        .seed(0x4d55_5441)
        .configure(|config| {
            config.r_energy = 0.0;
            config.r_mass = 0.0;
            config.d_energy_log2 = None;
            config.d_mass_log2 = None;
            config.maintenance_rate_log2 = None;
            config.p_spawn_log2 = None;
            config.mutation_base_log2 = 0;
            config.mutation_background_log2 = 0;
        })
        .at(
            0,
            0,
            ProgramBuilder::new().code(&[op::NOP; 64]).free_energy(100),
        )
        .at(
            1,
            0,
            ProgramBuilder::new()
                .code(&[op::EMIT; 64])
                .bg_radiation(100),
        )
        .build_simulation();
    let mut totals = EventTotals::default();
    let mut saw_base = false;
    let mut saw_background = false;

    for _ in 0..4 {
        let report = simulation.run_tick_report();
        assert_eq!(
            report.mutations,
            report.base_mutations + report.background_mutations
        );
        saw_base |= report.base_mutations > 0;
        saw_background |= report.background_mutations > 0;
        totals.record(report);
        assert_eq!(
            totals.mutations,
            totals.base_mutations + totals.background_mutations
        );
    }

    assert!(
        saw_base,
        "the run must exercise baseline mutation attribution"
    );
    assert!(
        saw_background,
        "the run must exercise background-stressed mutation attribution"
    );
}
