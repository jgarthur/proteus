#[macro_use]
mod helpers;

use helpers::{diff_grids, GridDiff, ProgramBuilder, WorldBuilder};
use proteus::op;
use proteus::Direction;

#[test]
fn world_builder_drives_snapshot_and_live_set_tests() {
    let mut simulation = WorldBuilder::new(2, 1)
        .seed(42)
        .at(
            0,
            0,
            ProgramBuilder::new()
                .code(&[op::NOP, op::ABSORB])
                .dir(Direction::Right)
                .id(9)
                .free_energy(7)
                .free_mass(3)
                .bg_radiation(5)
                .bg_mass(2),
        )
        .at(
            1,
            0,
            ProgramBuilder::new()
                .code(&[op::NOP])
                .dir(Direction::Up)
                .id(3)
                .newborn(true),
        )
        .build_simulation();

    assert_cell!(simulation.grid(), (0, 0), free_energy == 7, free_mass == 3);
    assert_program!(
        simulation.grid(),
        (0, 0),
        id == 9,
        dir == Direction::Right,
        live == true
    );
    assert_program!(simulation.grid(), (1, 0), is_newborn == true);

    let prepared = simulation.prepare_tick();
    assert_eq!(prepared.tick, 0);
    assert_eq!(prepared.snapshot[0].free_energy, 7);
    assert_eq!(prepared.snapshot[0].free_mass, 3);
    assert_eq!(prepared.snapshot[0].bg_radiation, 5);
    assert_eq!(prepared.snapshot[0].bg_mass, 2);
    assert_eq!(prepared.snapshot[0].program_size, 2);
    assert_eq!(prepared.snapshot[0].program_id, 9);
    assert_eq!(prepared.live_set, &[true, false]);
}

#[test]
fn diff_grids_reports_only_changed_cells() {
    let left = WorldBuilder::new(2, 1)
        .at(0, 0, ProgramBuilder::new().id(1).free_energy(4))
        .build_simulation();
    let right = WorldBuilder::new(2, 1)
        .at(0, 0, ProgramBuilder::new().id(1).free_energy(5))
        .build_simulation();

    let diffs = diff_grids(left.grid(), right.grid());
    assert_eq!(
        diffs,
        vec![GridDiff {
            x: 0,
            y: 0,
            left: format!("{:?}", left.grid().get(0).expect("left cell should exist")),
            right: format!(
                "{:?}",
                right.grid().get(0).expect("right cell should exist")
            ),
        }]
    );
}
