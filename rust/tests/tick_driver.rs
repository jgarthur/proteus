#[macro_use]
mod helpers;

use helpers::{run_ticks, ProgramBuilder, WorldBuilder};

#[test]
fn abandoned_inert_program_pays_maintenance_and_can_die() {
    let mut simulation = WorldBuilder::new(1, 1)
        .configure(|config| {
            config.maintenance_rate = 1.0;
            config.maintenance_exponent = 1.0;
            config.d_energy = 0.0;
            config.d_mass = 0.0;
            config.r_energy = 0.0;
            config.r_mass = 0.0;
            config.p_spawn = 0.0;
        })
        .at(
            0,
            0,
            ProgramBuilder::new()
                .code(&[0x10, 0x11])
                .live(false)
                .abandonment_timer(10),
        )
        .build_simulation();

    simulation.run_tick();

    assert_cell!(
        simulation.grid(),
        (0, 0),
        has_program == false,
        free_energy == 0,
        free_mass == 0
    );
}

#[test]
fn incoming_write_resets_inert_abandonment_timer_and_skips_maintenance() {
    let mut simulation = WorldBuilder::new(2, 1)
        .configure(|config| {
            config.inert_grace_ticks = 3;
            config.maintenance_rate = 1.0;
            config.maintenance_exponent = 1.0;
            config.d_energy = 0.0;
            config.d_mass = 0.0;
            config.r_energy = 0.0;
            config.r_mass = 0.0;
            config.p_spawn = 0.0;
        })
        .at(
            0,
            0,
            ProgramBuilder::new().code(&[0x01, 0x5e]).free_energy(1),
        )
        .at(
            1,
            0,
            ProgramBuilder::new()
                .code(&[0x10, 0x11])
                .live(false)
                .abandonment_timer(9)
                .open(true),
        )
        .build_simulation();

    simulation.run_tick();

    assert_program!(
        simulation.grid(),
        (1, 0),
        code == &[0x01, 0x11][..],
        abandonment_timer == 0
    );
}

#[test]
fn spontaneous_spawn_waits_until_next_tick_to_act_and_age() {
    let mut simulation = WorldBuilder::new(1, 1)
        .configure(|config| {
            config.r_mass = 1.0;
            config.d_mass = 0.0;
            config.p_spawn = 1.0;
            config.r_energy = 0.0;
            config.d_energy = 0.0;
            config.maintenance_rate = 0.0;
            config.mutation_base_log2 = 32;
            config.mutation_background_log2 = 32;
        })
        .build_simulation();

    simulation.run_tick();

    assert_program!(
        simulation.grid(),
        (0, 0),
        live == true,
        age == 0,
        did_nop == false
    );
    assert_cell!(simulation.grid(), (0, 0), free_mass == 1, bg_mass == 0);

    run_ticks(&mut simulation, 1);

    assert_program!(simulation.grid(), (0, 0), age == 1, did_nop == true);
}

#[test]
fn tick_report_counts_spontaneous_spawn_as_birth() {
    let mut simulation = WorldBuilder::new(1, 1)
        .configure(|config| {
            config.r_mass = 1.0;
            config.d_mass = 0.0;
            config.p_spawn = 1.0;
            config.r_energy = 0.0;
            config.d_energy = 0.0;
            config.maintenance_rate = 0.0;
            config.mutation_base_log2 = 32;
            config.mutation_background_log2 = 32;
        })
        .build_simulation();

    let report = simulation.run_tick_report();

    assert_eq!(report.births, 1);
    assert_eq!(report.deaths, 0);
    assert_eq!(report.mutations, 0);
}

#[test]
fn booted_abandoned_inert_program_skips_maintenance_on_boot_tick() {
    let mut simulation = WorldBuilder::new(2, 1)
        .configure(|config| {
            config.maintenance_rate = 1.0;
            config.maintenance_exponent = 1.0;
            config.inert_grace_ticks = 0;
            config.d_energy = 0.0;
            config.d_mass = 0.0;
            config.r_energy = 0.0;
            config.r_mass = 0.0;
            config.p_spawn = 0.0;
            config.mutation_base_log2 = 32;
            config.mutation_background_log2 = 32;
        })
        .at(0, 0, ProgramBuilder::new().code(&[0x64]))
        .at(
            1,
            0,
            ProgramBuilder::new()
                .code(&[0x50])
                .live(false)
                .abandonment_timer(4)
                .open(true),
        )
        .build_simulation();

    simulation.run_tick();

    assert_program!(simulation.grid(), (1, 0), live == true, age == 0);

    simulation.run_tick();

    assert_cell!(
        simulation.grid(),
        (1, 0),
        has_program == false,
        free_mass == 0
    );
}

#[test]
fn free_resource_decay_only_hits_excess_above_threshold() {
    let mut simulation = WorldBuilder::new(1, 1)
        .configure(|config| {
            config.d_energy = 1.0;
            config.d_mass = 1.0;
            config.r_energy = 0.0;
            config.r_mass = 0.0;
            config.maintenance_rate = 0.0;
            config.t_cap = 2.5;
            config.mutation_base_log2 = 32;
            config.mutation_background_log2 = 32;
        })
        .at(
            0,
            0,
            ProgramBuilder::new()
                .code(&[0x50])
                .free_energy(5)
                .free_mass(4),
        )
        .build_simulation();

    simulation.run_tick();

    assert_cell!(simulation.grid(), (0, 0), free_energy == 3, free_mass == 3);
}

#[test]
fn forced_mutation_changes_a_live_program_that_started_the_tick() {
    let mut simulation = WorldBuilder::new(1, 1)
        .configure(|config| {
            config.maintenance_rate = 0.0;
            config.d_energy = 0.0;
            config.d_mass = 0.0;
            config.r_energy = 0.0;
            config.r_mass = 0.0;
            config.p_spawn = 0.0;
            config.mutation_base_log2 = 0;
            config.mutation_background_log2 = 0;
        })
        .at(0, 0, ProgramBuilder::new().code(&[0x50, 0x50]))
        .build_simulation();

    simulation.run_tick();

    let cell = simulation
        .grid()
        .get(simulation.grid().index(0, 0))
        .expect("cell should exist");
    let program = cell.program.as_ref().expect("program should exist");
    assert_ne!(program.code, vec![0x50, 0x50]);
    assert_eq!(program.age, 1);
}

#[test]
fn maintenance_destroyed_instructions_do_not_become_free_mass() {
    let mut simulation = WorldBuilder::new(1, 1)
        .configure(|config| {
            config.maintenance_rate = 1.0;
            config.maintenance_exponent = 1.0;
            config.d_energy = 0.0;
            config.d_mass = 0.0;
            config.r_energy = 0.0;
            config.r_mass = 0.0;
            config.p_spawn = 0.0;
        })
        .at(0, 0, ProgramBuilder::new().code(&[0x50]))
        .build_simulation();

    simulation.run_tick();

    assert_cell!(
        simulation.grid(),
        (0, 0),
        has_program == false,
        free_mass == 0
    );
}
