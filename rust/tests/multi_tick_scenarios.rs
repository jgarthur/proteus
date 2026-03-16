#[macro_use]
mod helpers;

use helpers::{ProgramBuilder, WorldBuilder};
use proteus::op;

#[test]
fn absorb_loop_accumulates_energy_after_the_initial_arrival_lag() {
    let mut simulation = WorldBuilder::new(1, 1)
        .configure(|config| {
            config.r_energy = 1.0;
            config.d_energy = 0.0;
            config.r_mass = 0.0;
            config.d_mass = 0.0;
            config.maintenance_rate = 0.0;
            config.p_spawn = 0.0;
            config.mutation_base_log2 = 32;
            config.mutation_background_log2 = 32;
        })
        .at(0, 0, ProgramBuilder::new().code(&[op::ABSORB, op::NOP]))
        .build_simulation();

    simulation.run_tick();
    assert_cell!(
        simulation.grid(),
        (0, 0),
        free_energy == 0,
        bg_radiation == 1
    );

    for expected_energy in 1..=6 {
        simulation.run_tick();
        assert_cell!(
            simulation.grid(),
            (0, 0),
            free_energy == expected_energy,
            bg_radiation == 1
        );
    }
}

#[test]
fn inert_program_only_pays_maintenance_after_grace_window_expires() {
    let mut simulation = WorldBuilder::new(1, 1)
        .configure(|config| {
            config.inert_grace_ticks = 3;
            config.maintenance_rate = 1.0;
            config.maintenance_exponent = 1.0;
            config.r_energy = 0.0;
            config.d_energy = 0.0;
            config.r_mass = 0.0;
            config.d_mass = 0.0;
            config.p_spawn = 0.0;
            config.mutation_base_log2 = 32;
            config.mutation_background_log2 = 32;
        })
        .at(
            0,
            0,
            ProgramBuilder::new()
                .code(&[op::NOP, op::NOP, op::NOP, op::NOP, op::NOP])
                .live(false)
                .free_energy(4),
        )
        .build_simulation();

    simulation.run_tick();
    assert_program!(
        simulation.grid(),
        (0, 0),
        live == false,
        abandonment_timer == 1,
        code == &[op::NOP, op::NOP, op::NOP, op::NOP, op::NOP][..]
    );
    assert_cell!(simulation.grid(), (0, 0), free_energy == 4);

    simulation.run_tick();
    assert_program!(
        simulation.grid(),
        (0, 0),
        live == false,
        abandonment_timer == 2,
        code == &[op::NOP, op::NOP, op::NOP, op::NOP, op::NOP][..]
    );
    assert_cell!(simulation.grid(), (0, 0), free_energy == 4);

    simulation.run_tick();
    assert_program!(
        simulation.grid(),
        (0, 0),
        live == false,
        abandonment_timer == 3,
        code == &[op::NOP, op::NOP, op::NOP, op::NOP][..]
    );
    assert_cell!(simulation.grid(), (0, 0), free_energy == 0);
}

#[test]
fn repeated_deladj_pressure_grows_predator_until_the_size_one_guard_stops_it() {
    let mut simulation = WorldBuilder::new(2, 1)
        .configure(|config| {
            config.r_energy = 0.0;
            config.d_energy = 0.0;
            config.r_mass = 0.0;
            config.d_mass = 0.0;
            config.maintenance_rate = 0.0;
            config.p_spawn = 0.0;
            config.mutation_base_log2 = 32;
            config.mutation_background_log2 = 32;
        })
        .at(
            0,
            0,
            ProgramBuilder::new().code(&[op::DEL_ADJ]).free_energy(3),
        )
        .at(
            1,
            0,
            ProgramBuilder::new()
                .code(&[op::DUP, op::DROP, op::SWAP])
                .live(false)
                .open(true),
        )
        .build_simulation();

    simulation.run_tick();
    assert_program!(simulation.grid(), (0, 0), dst == 1, flag == false);
    assert_cell!(simulation.grid(), (0, 0), free_energy == 2, free_mass == 1);
    assert_program!(
        simulation.grid(),
        (1, 0),
        code == &[op::DROP, op::SWAP][..],
        live == false
    );

    simulation.run_tick();
    assert_program!(simulation.grid(), (0, 0), dst == 2, flag == false);
    assert_cell!(simulation.grid(), (0, 0), free_energy == 1, free_mass == 2);
    assert_program!(
        simulation.grid(),
        (1, 0),
        code == &[op::DROP][..],
        live == false
    );

    simulation.run_tick();
    assert_program!(simulation.grid(), (0, 0), dst == 2, flag == true);
    assert_cell!(simulation.grid(), (0, 0), free_energy == 0, free_mass == 2);
    assert_program!(
        simulation.grid(),
        (1, 0),
        code == &[op::DROP][..],
        live == false
    );
}

#[test]
fn extinct_cell_can_respawn_on_a_later_tick_when_mass_arrives() {
    let mut simulation = WorldBuilder::new(1, 1)
        .configure(|config| {
            config.maintenance_rate = 1.0;
            config.maintenance_exponent = 1.0;
            config.r_energy = 0.0;
            config.d_energy = 0.0;
            config.r_mass = 1.0;
            config.d_mass = 0.0;
            config.p_spawn = 1.0;
            config.mutation_base_log2 = 32;
            config.mutation_background_log2 = 32;
        })
        .at(0, 0, ProgramBuilder::new().code(&[op::NOP]))
        .build_simulation();

    simulation.run_tick();
    assert_cell!(
        simulation.grid(),
        (0, 0),
        has_program == false,
        free_mass == 0,
        bg_mass == 1
    );

    simulation.run_tick();
    assert_program!(
        simulation.grid(),
        (0, 0),
        live == true,
        age == 0,
        did_nop == false
    );
    assert_cell!(simulation.grid(), (0, 0), free_mass == 2, bg_mass == 0);
}
