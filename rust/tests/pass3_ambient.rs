#[macro_use]
mod helpers;

use helpers::{ProgramBuilder, WorldBuilder};
use proteus::op;
use proteus::{Direction, Pass3AmbientOutput};

#[test]
fn overlapping_absorb_footprints_split_background_radiation() {
    let mut simulation = WorldBuilder::new(3, 1)
        .configure(|config| {
            config.d_energy = 0.0;
            config.r_energy = 0.0;
            config.d_mass = 0.0;
            config.r_mass = 0.0;
        })
        .at(
            0,
            0,
            ProgramBuilder::new()
                .code(&[op::ABSORB])
                .absorb_count(2)
                .absorb_dir(Direction::Right),
        )
        .bg_radiation_at(1, 0, 5)
        .at(
            2,
            0,
            ProgramBuilder::new()
                .code(&[op::ABSORB])
                .absorb_count(2)
                .absorb_dir(Direction::Left),
        )
        .build_simulation();

    let output = simulation.run_pass3_ambient();

    assert_eq!(
        output,
        Pass3AmbientOutput {
            spawn_candidates: vec![false, false, false]
        }
    );
    assert_cell!(
        simulation.grid(),
        (0, 0),
        free_energy == 2,
        bg_radiation == 0
    );
    assert_cell!(
        simulation.grid(),
        (1, 0),
        free_energy == 0,
        bg_radiation == 1
    );
    assert_cell!(
        simulation.grid(),
        (2, 0),
        free_energy == 2,
        bg_radiation == 0
    );
}

#[test]
fn collect_converts_background_mass_into_free_mass_before_mass_arrival() {
    let mut simulation = WorldBuilder::new(1, 1)
        .configure(|config| {
            config.d_energy = 0.0;
            config.r_energy = 0.0;
            config.d_mass = 0.0;
            config.r_mass = 1.0;
        })
        .at(0, 0, ProgramBuilder::new().code(&[op::COLLECT]).did_collect(true))
        .bg_mass_at(0, 0, 4)
        .build_simulation();

    let output = simulation.run_pass3_ambient();

    assert_eq!(
        output,
        Pass3AmbientOutput {
            spawn_candidates: vec![false]
        }
    );
    assert_cell!(simulation.grid(), (0, 0), free_mass == 4, bg_mass == 1);
    assert_program!(simulation.grid(), (0, 0), did_collect == true);
}

#[test]
fn empty_cell_mass_arrival_marks_spawn_candidate() {
    let mut simulation = WorldBuilder::new(2, 1)
        .configure(|config| {
            config.d_energy = 0.0;
            config.r_energy = 0.0;
            config.d_mass = 0.0;
            config.r_mass = 1.0;
        })
        .at(1, 0, ProgramBuilder::new().code(&[op::NOP]))
        .build_simulation();

    let output = simulation.run_pass3_ambient();

    assert_eq!(output.spawn_candidates, vec![true, false]);
    assert_cell!(
        simulation.grid(),
        (0, 0),
        bg_mass == 1,
        has_program == false
    );
    assert_cell!(simulation.grid(), (1, 0), bg_mass == 1, has_program == true);
}

#[test]
fn background_radiation_decay_then_arrival_uses_post_absorb_remainder() {
    let mut simulation = WorldBuilder::new(1, 1)
        .configure(|config| {
            config.d_energy = 1.0;
            config.r_energy = 1.0;
            config.d_mass = 0.0;
            config.r_mass = 0.0;
        })
        .bg_radiation_at(0, 0, 3)
        .build_simulation();

    let output = simulation.run_pass3_ambient();

    assert_eq!(
        output,
        Pass3AmbientOutput {
            spawn_candidates: vec![false]
        }
    );
    assert_cell!(
        simulation.grid(),
        (0, 0),
        bg_radiation == 1,
        free_energy == 0
    );
}
