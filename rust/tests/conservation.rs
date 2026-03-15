#[macro_use]
mod helpers;

use helpers::{run_ticks, ProgramBuilder, WorldBuilder};
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
        .at(0, 0, ProgramBuilder::new().code(&[0x51, 0x50]))
        .bg_radiation_at(0, 0, 3)
        .at(1, 0, ProgramBuilder::new().code(&[0x53, 0x50]))
        .bg_mass_at(1, 0, 4)
        .at(
            2,
            0,
            ProgramBuilder::new().code(&[0x01, 0x61]).free_energy(5),
        )
        .at(
            3,
            0,
            ProgramBuilder::new().code(&[0x02, 0x54]).free_energy(5),
        )
        .at(4, 0, ProgramBuilder::new().code(&[0x52, 0x50]))
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
            config.r_energy = 1.0;
            config.r_mass = 1.0;
            config.d_energy = 0.0;
            config.d_mass = 0.0;
            config.maintenance_rate = 0.0;
            config.p_spawn = 0.0;
            config.mutation_base_log2 = 32;
            config.mutation_background_log2 = 32;
        })
        .at(0, 0, ProgramBuilder::new().code(&[0x51, 0x50]))
        .bg_radiation_at(0, 0, 3)
        .at(1, 0, ProgramBuilder::new().code(&[0x53, 0x50]))
        .bg_mass_at(1, 0, 2)
        .build_simulation();

    let initial_energy = total_energy(&simulation);
    let initial_mass = total_mass(&simulation);

    simulation.run_tick();

    assert_eq!(initial_energy, 3);
    assert_eq!(initial_mass, 6);
    assert_eq!(total_energy(&simulation), initial_energy + 2);
    assert_eq!(total_mass(&simulation), initial_mass + 2);
    assert_cell!(
        simulation.grid(),
        (0, 0),
        free_energy == 3,
        bg_radiation == 1,
        bg_mass == 1
    );
    assert_cell!(
        simulation.grid(),
        (1, 0),
        free_mass == 2,
        bg_radiation == 1,
        bg_mass == 1
    );
}

#[test]
fn forced_arrivals_and_decay_have_exact_accounting_through_pass3_ordering() {
    let mut simulation = WorldBuilder::new(3, 1)
        .configure(|config| {
            config.r_energy = 1.0;
            config.r_mass = 1.0;
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
            ProgramBuilder::new().code(&[0x51, 0x50]).free_energy(1),
        )
        .bg_radiation_at(0, 0, 4)
        .at(1, 0, ProgramBuilder::new().code(&[0x53, 0x50]).free_mass(1))
        .bg_mass_at(1, 0, 4)
        .bg_radiation_at(2, 0, 3)
        .bg_mass_at(2, 0, 2)
        .build_simulation();

    let initial_energy = total_energy(&simulation);
    let initial_mass = total_mass(&simulation);

    simulation.run_tick();

    assert_eq!(initial_energy, 8);
    assert_eq!(initial_mass, 11);
    assert_eq!(total_energy(&simulation), 5);
    assert_eq!(total_mass(&simulation), 9);
    assert_cell!(
        simulation.grid(),
        (0, 0),
        free_energy == 2,
        free_mass == 0,
        bg_radiation == 1,
        bg_mass == 1
    );
    assert_cell!(
        simulation.grid(),
        (1, 0),
        free_energy == 0,
        free_mass == 2,
        bg_radiation == 1,
        bg_mass == 1
    );
    assert_cell!(
        simulation.grid(),
        (2, 0),
        has_program == false,
        free_energy == 0,
        free_mass == 0,
        bg_radiation == 1,
        bg_mass == 1
    );
}
