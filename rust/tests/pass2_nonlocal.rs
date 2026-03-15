#[macro_use]
mod helpers;

use helpers::{ProgramBuilder, WorldBuilder};
use proteus::{pass2_nonlocal, Direction, Pass2Output, QueuedAction, PROGRAM_SIZE_CAP};

#[test]
fn two_read_adj_actions_against_same_target_both_succeed() {
    let mut simulation = WorldBuilder::new(3, 1)
        .at(0, 0, ProgramBuilder::new().code(&[0x50]).src(4))
        .at(1, 0, ProgramBuilder::new().code(&[0x11, 0x22]))
        .at(2, 0, ProgramBuilder::new().code(&[0x50]).src(9))
        .build_simulation();

    let output = simulation.run_pass2(&[
        QueuedAction::ReadAdj {
            source: 0,
            target: 1,
            src_cursor: 1,
        },
        QueuedAction::ReadAdj {
            source: 2,
            target: 1,
            src_cursor: 0,
        },
    ]);

    assert_eq!(
        output,
        Pass2Output {
            incoming_writes: vec![false, false, false],
            booted_programs: 0,
        }
    );
    assert_program!(
        simulation.grid(),
        (0, 0),
        stack == &[0x22][..],
        src == 5,
        flag == false
    );
    assert_program!(
        simulation.grid(),
        (2, 0),
        stack == &[0x11][..],
        src == 10,
        flag == false
    );
}

#[test]
fn read_adj_uses_pre_pass2_target_code() {
    let mut simulation = WorldBuilder::new(3, 1)
        .at(0, 0, ProgramBuilder::new().code(&[0x50]))
        .at(1, 0, ProgramBuilder::new().code(&[0x50]).free_energy(1))
        .at(2, 0, ProgramBuilder::new().code(&[0x10]).open(true))
        .build_simulation();

    simulation.run_pass2(&[
        QueuedAction::ReadAdj {
            source: 0,
            target: 2,
            src_cursor: 0,
        },
        QueuedAction::WriteAdj {
            source: 1,
            target: 2,
            value: 0x7f,
            dst_cursor: 0,
        },
    ]);

    assert_program!(
        simulation.grid(),
        (0, 0),
        stack == &[0x10][..],
        flag == false
    );
    assert_program!(simulation.grid(), (2, 0), code == &[0x7f][..]);
}

#[test]
fn multiple_give_e_actions_sum_into_the_same_target() {
    let mut simulation = WorldBuilder::new(3, 1)
        .at(0, 0, ProgramBuilder::new().code(&[0x50]).free_energy(5))
        .at(1, 0, ProgramBuilder::new().code(&[0x50]).free_energy(4))
        .free_energy_at(2, 0, 1)
        .build_simulation();

    simulation.run_pass2(&[
        QueuedAction::GiveE {
            source: 0,
            target: 2,
            amount: 3,
        },
        QueuedAction::GiveE {
            source: 1,
            target: 2,
            amount: 10,
        },
    ]);

    assert_cell!(simulation.grid(), (0, 0), free_energy == 2, free_mass == 0);
    assert_cell!(simulation.grid(), (1, 0), free_energy == 0, free_mass == 0);
    assert_cell!(simulation.grid(), (2, 0), free_energy == 8, free_mass == 0);
    assert_program!(simulation.grid(), (0, 0), flag == false);
    assert_program!(simulation.grid(), (1, 0), flag == false);
}

#[test]
fn additive_transfers_do_not_block_exclusive_writes() {
    let mut simulation = WorldBuilder::new(4, 1)
        .at(0, 0, ProgramBuilder::new().code(&[0x50]).free_energy(2))
        .at(1, 0, ProgramBuilder::new().code(&[0x50]).free_energy(3))
        .at(2, 0, ProgramBuilder::new().code(&[0x50]).free_energy(1))
        .at(3, 0, ProgramBuilder::new().code(&[0x10]).open(true))
        .build_simulation();

    simulation.run_pass2(&[
        QueuedAction::GiveE {
            source: 0,
            target: 3,
            amount: 2,
        },
        QueuedAction::GiveE {
            source: 1,
            target: 3,
            amount: 3,
        },
        QueuedAction::WriteAdj {
            source: 2,
            target: 3,
            value: 0x7f,
            dst_cursor: 0,
        },
    ]);

    assert_cell!(simulation.grid(), (3, 0), free_energy == 5, free_mass == 0);
    assert_program!(simulation.grid(), (2, 0), flag == false, dst == 1);
    assert_program!(simulation.grid(), (3, 0), code == &[0x7f][..]);
}

#[test]
fn give_e_can_feed_a_protected_target() {
    let mut simulation = WorldBuilder::new(2, 1)
        .at(0, 0, ProgramBuilder::new().code(&[0x50]).free_energy(4))
        .at(1, 0, ProgramBuilder::new().code(&[0x10]).flag(true))
        .build_simulation();

    simulation.run_pass2(&[QueuedAction::GiveE {
        source: 0,
        target: 1,
        amount: 3,
    }]);

    assert_cell!(simulation.grid(), (0, 0), free_energy == 1);
    assert_cell!(simulation.grid(), (1, 0), free_energy == 3);
    assert_program!(simulation.grid(), (0, 0), flag == false);
    assert_program!(simulation.grid(), (1, 0), flag == true, is_open == false);
}

#[test]
fn nonpositive_give_amounts_are_flag_neutral_no_ops() {
    let mut simulation = WorldBuilder::new(3, 1)
        .at(
            0,
            0,
            ProgramBuilder::new()
                .code(&[0x50])
                .flag(true)
                .free_energy(4)
                .free_mass(5),
        )
        .at(
            1,
            0,
            ProgramBuilder::new().code(&[0x50]).flag(true).free_mass(3),
        )
        .at(2, 0, ProgramBuilder::new().code(&[0x10]))
        .build_simulation();

    simulation.run_pass2(&[
        QueuedAction::GiveE {
            source: 0,
            target: 2,
            amount: -7,
        },
        QueuedAction::GiveM {
            source: 1,
            target: 2,
            amount: 0,
        },
    ]);

    assert_cell!(simulation.grid(), (0, 0), free_energy == 4, free_mass == 5);
    assert_cell!(simulation.grid(), (1, 0), free_energy == 0, free_mass == 3);
    assert_cell!(simulation.grid(), (2, 0), free_energy == 0, free_mass == 0);
    assert_program!(simulation.grid(), (0, 0), flag == true);
    assert_program!(simulation.grid(), (1, 0), flag == true);
}

#[test]
fn give_e_caps_transfer_at_available_energy_for_large_requested_amounts() {
    let mut simulation = WorldBuilder::new(2, 1)
        .at(0, 0, ProgramBuilder::new().code(&[0x50]).free_energy(7))
        .at(1, 0, ProgramBuilder::new().code(&[0x50]).free_energy(2))
        .build_simulation();

    simulation.run_pass2(&[QueuedAction::GiveE {
        source: 0,
        target: 1,
        amount: i16::MAX,
    }]);

    assert_cell!(simulation.grid(), (0, 0), free_energy == 0, free_mass == 0);
    assert_cell!(simulation.grid(), (1, 0), free_energy == 9, free_mass == 0);
    assert_program!(simulation.grid(), (0, 0), flag == false);
}

#[test]
fn multiple_give_m_actions_sum_into_the_same_target() {
    let mut simulation = WorldBuilder::new(3, 1)
        .at(0, 0, ProgramBuilder::new().code(&[0x50]).free_mass(5))
        .at(1, 0, ProgramBuilder::new().code(&[0x50]).free_mass(2))
        .free_mass_at(2, 0, 1)
        .build_simulation();

    simulation.run_pass2(&[
        QueuedAction::GiveM {
            source: 0,
            target: 2,
            amount: 2,
        },
        QueuedAction::GiveM {
            source: 1,
            target: 2,
            amount: 5,
        },
    ]);

    assert_cell!(simulation.grid(), (0, 0), free_energy == 0, free_mass == 3);
    assert_cell!(simulation.grid(), (1, 0), free_energy == 0, free_mass == 0);
    assert_cell!(simulation.grid(), (2, 0), free_energy == 0, free_mass == 5);
    assert_program!(simulation.grid(), (0, 0), flag == false);
    assert_program!(simulation.grid(), (1, 0), flag == false);
}

#[test]
fn give_m_caps_transfer_at_available_mass_for_large_requested_amounts() {
    let mut simulation = WorldBuilder::new(2, 1)
        .at(0, 0, ProgramBuilder::new().code(&[0x50]).free_mass(6))
        .at(1, 0, ProgramBuilder::new().code(&[0x50]).free_mass(3))
        .build_simulation();

    simulation.run_pass2(&[QueuedAction::GiveM {
        source: 0,
        target: 1,
        amount: i16::MAX,
    }]);

    assert_cell!(simulation.grid(), (0, 0), free_energy == 0, free_mass == 0);
    assert_cell!(simulation.grid(), (1, 0), free_energy == 0, free_mass == 9);
    assert_program!(simulation.grid(), (0, 0), flag == false);
}

#[test]
fn one_hundred_give_e_actions_to_the_same_target_all_succeed() {
    let mut builder = WorldBuilder::new(101, 1);
    for x in 0..100_u32 {
        builder = builder.at(x, 0, ProgramBuilder::new().code(&[0x50]).free_energy(1));
    }
    let mut simulation = builder.build_simulation();

    let actions = (0..100_usize)
        .map(|source| QueuedAction::GiveE {
            source,
            target: 100,
            amount: 1,
        })
        .collect::<Vec<_>>();

    simulation.run_pass2(&actions);

    for x in 0..100_u32 {
        assert_cell!(simulation.grid(), (x, 0), free_energy == 0, free_mass == 0);
        assert_program!(simulation.grid(), (x, 0), flag == false);
    }
    assert_cell!(
        simulation.grid(),
        (100, 0),
        free_energy == 100,
        free_mass == 0
    );
}

#[test]
fn write_adj_and_append_adj_conflict_on_the_same_target() {
    let mut simulation = WorldBuilder::new(3, 1)
        .at(0, 0, ProgramBuilder::new().code(&[0x50]).free_energy(1))
        .at(
            1,
            0,
            ProgramBuilder::new()
                .code(&[0x50, 0x50, 0x50])
                .free_energy(3)
                .free_mass(1),
        )
        .at(2, 0, ProgramBuilder::new().code(&[0x10]).open(true))
        .build_simulation();

    let output = simulation.run_pass2(&[
        QueuedAction::WriteAdj {
            source: 0,
            target: 2,
            value: 0x7f,
            dst_cursor: 0,
        },
        QueuedAction::AppendAdj {
            source: 1,
            target: 2,
            value: 0x33,
        },
    ]);

    assert_eq!(output.incoming_writes, vec![false, false, true]);
    assert_program!(simulation.grid(), (0, 0), flag == true, dst == 0);
    assert_program!(
        simulation.grid(),
        (1, 0),
        flag == false,
        code == &[0x50, 0x50, 0x50][..]
    );
    assert_cell!(simulation.grid(), (1, 0), free_mass == 0);
    assert_program!(simulation.grid(), (2, 0), code == &[0x10, 0x33][..]);
}

#[test]
fn write_adj_fails_against_a_protected_target() {
    let mut simulation = WorldBuilder::new(2, 1)
        .at(0, 0, ProgramBuilder::new().code(&[0x50]).free_energy(1))
        .at(1, 0, ProgramBuilder::new().code(&[0x10]).dst(4))
        .build_simulation();

    simulation.run_pass2(&[QueuedAction::WriteAdj {
        source: 0,
        target: 1,
        value: 0x7f,
        dst_cursor: 0,
    }]);

    assert_program!(simulation.grid(), (0, 0), flag == true, dst == 0);
    assert_program!(simulation.grid(), (1, 0), code == &[0x10][..], dst == 4);
}

#[test]
fn occupied_append_adj_fails_against_a_protected_target_without_spending_mass() {
    let mut simulation = WorldBuilder::new(2, 1)
        .at(
            0,
            0,
            ProgramBuilder::new()
                .code(&[0x50])
                .free_energy(1)
                .free_mass(2),
        )
        .at(1, 0, ProgramBuilder::new().code(&[0x10]))
        .build_simulation();

    simulation.run_pass2(&[QueuedAction::AppendAdj {
        source: 0,
        target: 1,
        value: 0x33,
    }]);

    assert_program!(simulation.grid(), (0, 0), flag == true);
    assert_cell!(simulation.grid(), (0, 0), free_mass == 2);
    assert_program!(simulation.grid(), (1, 0), code == &[0x10][..]);
}

#[test]
fn append_adj_size_cap_failure_sets_flag_without_spending_mass() {
    let full_program = vec![0x10; usize::from(PROGRAM_SIZE_CAP)];
    let mut simulation = WorldBuilder::new(2, 1)
        .at(
            0,
            0,
            ProgramBuilder::new()
                .code(&[0x50])
                .free_energy(1)
                .free_mass(1),
        )
        .at(1, 0, ProgramBuilder::new().code(&full_program).open(true))
        .build_simulation();

    simulation.run_pass2(&[QueuedAction::AppendAdj {
        source: 0,
        target: 1,
        value: 0x33,
    }]);

    assert_program!(simulation.grid(), (0, 0), flag == true);
    assert_cell!(simulation.grid(), (0, 0), free_mass == 1);
    assert_program!(simulation.grid(), (1, 0), code == full_program.as_slice());
}

#[test]
fn append_into_empty_cell_cannot_be_booted_in_the_same_tick() {
    let mut simulation = WorldBuilder::new(3, 1)
        .at(
            0,
            0,
            ProgramBuilder::new()
                .code(&[0x50])
                .free_energy(1)
                .free_mass(1),
        )
        .at(1, 0, ProgramBuilder::new().code(&[0x50]))
        .build_simulation();

    simulation.run_pass2(&[
        QueuedAction::AppendAdj {
            source: 0,
            target: 2,
            value: 0x33,
        },
        QueuedAction::Boot {
            source: 1,
            target: 2,
        },
    ]);

    assert_program!(simulation.grid(), (0, 0), flag == false);
    assert_program!(simulation.grid(), (1, 0), flag == true);
    assert_program!(
        simulation.grid(),
        (2, 0),
        code == &[0x33][..],
        live == false,
        is_open == true,
        is_newborn == false
    );
}

#[test]
fn boot_plus_boot_on_same_inert_target_all_succeed() {
    let mut simulation = WorldBuilder::new(3, 1)
        .at(0, 0, ProgramBuilder::new().code(&[0x50]).flag(true))
        .at(1, 0, ProgramBuilder::new().code(&[0x50]).flag(true))
        .at(2, 0, ProgramBuilder::new().code(&[0x10]).live(false))
        .build_simulation();

    simulation.run_pass2(&[
        QueuedAction::Boot {
            source: 0,
            target: 2,
        },
        QueuedAction::Boot {
            source: 1,
            target: 2,
        },
    ]);

    assert_program!(simulation.grid(), (0, 0), flag == false);
    assert_program!(simulation.grid(), (1, 0), flag == false);
    assert_program!(
        simulation.grid(),
        (2, 0),
        live == true,
        ip == 0,
        is_newborn == true,
        is_open == false
    );
}

#[test]
fn del_adj_additional_cost_failure_has_no_fallback_winner() {
    let mut simulation = WorldBuilder::new(3, 1)
        .at(
            0,
            0,
            ProgramBuilder::new()
                .code(&[0x50, 0x50, 0x50, 0x50])
                .free_energy(2),
        )
        .at(1, 0, ProgramBuilder::new().code(&[0x50]).free_energy(1))
        .at(
            2,
            0,
            ProgramBuilder::new()
                .code(&[0x20, 0x21, 0x22])
                .free_energy(3)
                .open(true),
        )
        .build_simulation();

    simulation.run_pass2(&[
        QueuedAction::DelAdj {
            source: 0,
            target: 2,
            dst_cursor: 0,
        },
        QueuedAction::WriteAdj {
            source: 1,
            target: 2,
            value: 0x7f,
            dst_cursor: 0,
        },
    ]);

    assert_program!(simulation.grid(), (0, 0), flag == true, dst == 0);
    assert_program!(simulation.grid(), (1, 0), flag == true, dst == 0);
    assert_cell!(simulation.grid(), (0, 0), free_energy == 2, free_mass == 0);
    assert_program!(simulation.grid(), (2, 0), code == &[0x20, 0x21, 0x22][..]);
}

#[test]
fn del_adj_adjusts_target_ip_only_when_deletion_is_strictly_before_it() {
    let mut simulation = WorldBuilder::new(4, 1)
        .at(
            0,
            0,
            ProgramBuilder::new().code(&[0x50]).free_energy(5).dst(9),
        )
        .at(
            1,
            0,
            ProgramBuilder::new()
                .code(&[0x10, 0x11, 0x12])
                .ip(2)
                .open(true),
        )
        .at(
            2,
            0,
            ProgramBuilder::new().code(&[0x50]).free_energy(5).dst(1),
        )
        .at(
            3,
            0,
            ProgramBuilder::new()
                .code(&[0x20, 0x21, 0x22])
                .ip(1)
                .open(true),
        )
        .build_simulation();

    simulation.run_pass2(&[
        QueuedAction::DelAdj {
            source: 0,
            target: 1,
            dst_cursor: 1,
        },
        QueuedAction::DelAdj {
            source: 2,
            target: 3,
            dst_cursor: 1,
        },
    ]);

    assert_program!(simulation.grid(), (0, 0), flag == false, dst == 10);
    assert_program!(simulation.grid(), (2, 0), flag == false, dst == 2);
    assert_program!(
        simulation.grid(),
        (1, 0),
        code == &[0x10, 0x12][..],
        ip == 1
    );
    assert_program!(
        simulation.grid(),
        (3, 0),
        code == &[0x20, 0x22][..],
        ip == 1
    );
}

#[test]
fn del_adj_fails_against_size_one_target() {
    let mut simulation = WorldBuilder::new(2, 1)
        .at(0, 0, ProgramBuilder::new().code(&[0x50]).free_energy(3))
        .at(1, 0, ProgramBuilder::new().code(&[0x10]).open(true))
        .build_simulation();

    simulation.run_pass2(&[QueuedAction::DelAdj {
        source: 0,
        target: 1,
        dst_cursor: 0,
    }]);

    assert_program!(simulation.grid(), (0, 0), flag == true, dst == 0);
    assert_cell!(simulation.grid(), (0, 0), free_energy == 3, free_mass == 0);
    assert_program!(simulation.grid(), (1, 0), code == &[0x10][..]);
}

#[test]
fn opposing_moves_do_not_swap() {
    let mut simulation = WorldBuilder::new(2, 1)
        .at(
            0,
            0,
            ProgramBuilder::new()
                .code(&[0x10])
                .dir(Direction::Right)
                .free_energy(2)
                .free_mass(1),
        )
        .at(
            1,
            0,
            ProgramBuilder::new()
                .code(&[0x20])
                .dir(Direction::Left)
                .free_energy(3)
                .free_mass(2),
        )
        .build_simulation();

    simulation.run_pass2(&[
        QueuedAction::Move {
            source: 0,
            target: 1,
        },
        QueuedAction::Move {
            source: 1,
            target: 0,
        },
    ]);

    assert_program!(simulation.grid(), (0, 0), code == &[0x10][..], flag == true);
    assert_program!(simulation.grid(), (1, 0), code == &[0x20][..], flag == true);
    assert_cell!(simulation.grid(), (0, 0), free_energy == 2, free_mass == 1);
    assert_cell!(simulation.grid(), (1, 0), free_energy == 3, free_mass == 2);
}

#[test]
fn exclusive_conflict_tiebreak_is_deterministic_under_action_reordering() {
    let mut left = WorldBuilder::new(3, 1)
        .seed(17)
        .at(
            0,
            0,
            ProgramBuilder::new().code(&[0x50, 0x50]).free_energy(2),
        )
        .at(
            1,
            0,
            ProgramBuilder::new().code(&[0x50, 0x50]).free_energy(2),
        )
        .at(2, 0, ProgramBuilder::new().code(&[0x10]).open(true))
        .build_simulation();
    let mut right = WorldBuilder::new(3, 1)
        .seed(17)
        .at(
            0,
            0,
            ProgramBuilder::new().code(&[0x50, 0x50]).free_energy(2),
        )
        .at(
            1,
            0,
            ProgramBuilder::new().code(&[0x50, 0x50]).free_energy(2),
        )
        .at(2, 0, ProgramBuilder::new().code(&[0x10]).open(true))
        .build_simulation();

    let left_actions = [
        QueuedAction::WriteAdj {
            source: 0,
            target: 2,
            value: 0x11,
            dst_cursor: 0,
        },
        QueuedAction::WriteAdj {
            source: 1,
            target: 2,
            value: 0x22,
            dst_cursor: 0,
        },
    ];
    let right_actions = [left_actions[1], left_actions[0]];

    left.run_pass2(&left_actions);
    right.run_pass2(&right_actions);

    assert_eq!(left.grid(), right.grid());
}

#[test]
fn move_transfers_program_and_free_resources_but_leaves_background_behind() {
    let mut simulation = WorldBuilder::new(2, 1)
        .at(
            0,
            0,
            ProgramBuilder::new()
                .code(&[0x10])
                .free_energy(2)
                .free_mass(3)
                .bg_radiation(4)
                .bg_mass(5),
        )
        .free_energy_at(1, 0, 1)
        .free_mass_at(1, 0, 1)
        .bg_radiation_at(1, 0, 7)
        .bg_mass_at(1, 0, 8)
        .build_simulation();

    simulation.run_pass2(&[QueuedAction::Move {
        source: 0,
        target: 1,
    }]);

    assert_cell!(
        simulation.grid(),
        (0, 0),
        has_program == false,
        free_energy == 0,
        free_mass == 0,
        bg_radiation == 4,
        bg_mass == 5
    );
    assert_program!(
        simulation.grid(),
        (1, 0),
        code == &[0x10][..],
        flag == false
    );
    assert_cell!(
        simulation.grid(),
        (1, 0),
        free_energy == 3,
        free_mass == 4,
        bg_radiation == 7,
        bg_mass == 8
    );
}

#[test]
fn weighted_tie_break_prefers_larger_program_over_many_trials() {
    let (base_grid, _) = WorldBuilder::new(3, 1)
        .at(
            0,
            0,
            ProgramBuilder::new().code(&[0x50, 0x50]).free_energy(2),
        )
        .at(
            1,
            0,
            ProgramBuilder::new()
                .code(&[0x50, 0x50, 0x50, 0x50, 0x50, 0x50])
                .free_energy(2),
        )
        .at(2, 0, ProgramBuilder::new().code(&[0x00]).open(true))
        .build();

    let actions = [
        QueuedAction::WriteAdj {
            source: 0,
            target: 2,
            value: 0x11,
            dst_cursor: 0,
        },
        QueuedAction::WriteAdj {
            source: 1,
            target: 2,
            value: 0x22,
            dst_cursor: 0,
        },
    ];

    let mut larger_program_wins = 0_u32;
    let trials = 2048_u32;

    for trial in 0..trials {
        let mut grid = base_grid.clone();
        pass2_nonlocal(
            &mut grid,
            &actions,
            u64::from(trial),
            u64::from(trial).wrapping_mul(17),
        );

        let winning_value = grid
            .get(grid.index(2, 0))
            .expect("target cell should exist")
            .program
            .as_ref()
            .expect("target program should exist")
            .code[0];
        if winning_value == 0x22 {
            larger_program_wins += 1;
        }
    }

    let win_ratio = f64::from(larger_program_wins) / f64::from(trials);
    assert!(
        larger_program_wins > (trials / 2),
        "larger program should win more often than the smaller program, ratio={win_ratio:.3}"
    );
    assert!(
        (0.68..0.82).contains(&win_ratio),
        "weighted tie ratio should stay near the expected 0.75, got {win_ratio:.3}"
    );
}
