#[macro_use]
mod helpers;

use helpers::{run_ticks, ProgramBuilder, WorldBuilder};
use proteus::op;
use proteus::Simulation;

fn total_energy(simulation: &Simulation) -> u32 {
    let grid_energy = simulation
        .grid()
        .cells()
        .iter()
        .map(|cell| cell.free_energy + cell.bg_radiation)
        .sum::<u32>();
    grid_energy + u32::try_from(simulation.packets().len()).expect("packet count should fit in u32")
}

fn total_mass(simulation: &Simulation) -> u32 {
    simulation
        .grid()
        .cells()
        .iter()
        .map(|cell| {
            let code_mass = cell.program.as_ref().map_or(0_u32, |program| {
                u32::try_from(program.code.len()).expect("code len should fit")
            });
            cell.free_mass + cell.bg_mass + code_mass
        })
        .sum()
}

#[test]
fn zero_rate_world_preserves_total_energy_and_mass_under_internal_transfers() {
    let mut simulation = WorldBuilder::new(5, 1)
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
        .at(0, 0, ProgramBuilder::new().code(&[op::ABSORB, op::NOP]))
        .bg_radiation_at(0, 0, 3)
        .at(1, 0, ProgramBuilder::new().code(&[op::COLLECT, op::NOP]))
        .bg_mass_at(1, 0, 4)
        .at(
            2,
            0,
            ProgramBuilder::new()
                .code(&[op::push(1), op::GIVE_E])
                .free_energy(5),
        )
        .at(
            3,
            0,
            ProgramBuilder::new()
                .code(&[op::push(2), op::EMIT])
                .free_energy(5),
        )
        .at(4, 0, ProgramBuilder::new().code(&[op::LISTEN, op::NOP]))
        .build_simulation();

    let initial_energy = total_energy(&simulation);
    let initial_mass = total_mass(&simulation);

    for _ in 0..5 {
        simulation.run_tick();
        assert_eq!(total_energy(&simulation), initial_energy);
        assert_eq!(total_mass(&simulation), initial_mass);
    }

    run_ticks(&mut simulation, 5);
    assert_eq!(total_energy(&simulation), initial_energy);
    assert_eq!(total_mass(&simulation), initial_mass);
}

#[test]
fn forced_arrivals_with_absorb_and_collect_have_exact_accounting() {
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
        .bg_radiation_at(0, 0, 3)
        .at(1, 0, ProgramBuilder::new().code(&[op::COLLECT, op::NOP]))
        .bg_mass_at(1, 0, 2)
        .build_simulation();

    let initial_energy = total_energy(&simulation);
    let initial_mass = total_mass(&simulation);

    simulation.run_tick();

    let left = simulation
        .grid()
        .get(simulation.grid().index(0, 0))
        .expect("cell should exist");
    let right = simulation
        .grid()
        .get(simulation.grid().index(1, 0))
        .expect("cell should exist");
    let arrived_energy = left.bg_radiation + right.bg_radiation;
    let arrived_mass = left.bg_mass + right.bg_mass;

    assert_eq!(initial_energy, 3);
    assert_eq!(initial_mass, 6);
    assert_eq!(left.free_energy, 3);
    assert_eq!(right.free_mass, 2);
    assert_eq!(total_energy(&simulation), initial_energy + arrived_energy);
    assert_eq!(total_mass(&simulation), initial_mass + arrived_mass);
    assert!(left.bg_radiation > 1);
    assert!(left.bg_mass > 1);
    assert!(right.bg_radiation > 1);
    assert!(right.bg_mass > 1);
}

#[test]
fn forced_arrivals_and_decay_have_exact_accounting_through_pass3_ordering() {
    let mut simulation = WorldBuilder::new(3, 1)
        .configure(|config| {
            config.r_energy = 100.0;
            config.r_mass = 100.0;
            config.d_energy = 1.0;
            config.d_mass = 1.0;
            config.maintenance_rate = 0.0;
            config.p_spawn = 0.0;
            config.t_cap = 1.0;
            config.mutation_base_log2 = 32;
            config.mutation_background_log2 = 32;
        })
        .at(
            0,
            0,
            ProgramBuilder::new()
                .code(&[op::ABSORB, op::NOP])
                .free_energy(1),
        )
        .bg_radiation_at(0, 0, 4)
        .at(
            1,
            0,
            ProgramBuilder::new()
                .code(&[op::COLLECT, op::NOP])
                .free_mass(1),
        )
        .bg_mass_at(1, 0, 4)
        .bg_radiation_at(2, 0, 3)
        .bg_mass_at(2, 0, 2)
        .build_simulation();

    let initial_energy = total_energy(&simulation);
    let initial_mass = total_mass(&simulation);

    simulation.run_tick();

    let left = simulation
        .grid()
        .get(simulation.grid().index(0, 0))
        .expect("cell should exist");
    let center = simulation
        .grid()
        .get(simulation.grid().index(1, 0))
        .expect("cell should exist");
    let right = simulation
        .grid()
        .get(simulation.grid().index(2, 0))
        .expect("cell should exist");
    let arrived_energy = left.bg_radiation + center.bg_radiation + right.bg_radiation;
    let arrived_mass = left.bg_mass + center.bg_mass + right.bg_mass;

    assert_eq!(initial_energy, 8);
    assert_eq!(initial_mass, 11);
    assert_eq!(left.free_energy, 2);
    assert_eq!(left.free_mass, 0);
    assert_eq!(center.free_energy, 0);
    assert_eq!(center.free_mass, 2);
    assert!(!right.has_program());
    assert_eq!(right.free_energy, 0);
    assert_eq!(right.free_mass, 0);
    assert_eq!(total_energy(&simulation), 2 + arrived_energy);
    assert_eq!(total_mass(&simulation), 6 + arrived_mass);
    assert!(left.bg_radiation > 1);
    assert!(left.bg_mass > 1);
    assert!(center.bg_radiation > 1);
    assert!(center.bg_mass > 1);
    assert!(right.bg_radiation > 1);
    assert!(right.bg_mass > 1);
}
