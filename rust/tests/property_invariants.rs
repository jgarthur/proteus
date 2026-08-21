mod helpers;

use helpers::{run_ticks, ProgramBuilder, WorldBuilder};
use proteus::op;
use proteus::{pass1_local, pass2_nonlocal, Direction, Grid, QueuedAction, WyRand};

#[test]
fn neighbor_lookup_matches_toroidal_coordinates_for_small_grids() {
    for width in 1..=6_u32 {
        for height in 1..=6_u32 {
            let grid = Grid::new(width, height).expect("grid should build");
            for y in 0..height {
                for x in 0..width {
                    let index = grid.index(x, y);
                    for dir in Direction::ALL {
                        let (expected_x, expected_y) = match dir {
                            Direction::Right => ((x + 1) % width, y),
                            Direction::Up => (x, (y + (height - 1)) % height),
                            Direction::Left => ((x + (width - 1)) % width, y),
                            Direction::Down => (x, (y + 1) % height),
                        };
                        assert_eq!(
                            grid.neighbor(index, dir),
                            grid.index(expected_x, expected_y),
                            "neighbor mismatch on {width}x{height} grid at ({x}, {y}) heading {dir:?}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn sensing_uses_frozen_snapshot_after_target_cell_mutates() {
    let mut rng = WyRand::with_seed(0x7c9b_b41d_28ef_63a0);

    for case_idx in 0..128 {
        let initial_has_program = rng.bernoulli(0.5);
        let initial_energy = rng.next_u32() % 200;
        let initial_mass = rng.next_u32() % 200;
        let initial_id = (rng.next_u32() & 0xff) as u8;
        let initial_size =
            usize::try_from((rng.next_u32() % 6) + 1).expect("program size should fit in usize");

        let mut builder = WorldBuilder::new(2, 1).at(
            0,
            0,
            ProgramBuilder::new()
                .code(&[op::SENSE_E, op::SENSE_M, op::SENSE_SIZE, op::SENSE_ID])
                .dir(Direction::Right),
        );

        if initial_has_program {
            builder = builder.at(
                1,
                0,
                ProgramBuilder::new()
                    .code(&vec![op::NOP; initial_size])
                    .id(initial_id)
                    .free_energy(initial_energy)
                    .free_mass(initial_mass),
            );
        } else {
            builder = builder
                .free_energy_at(1, 0, initial_energy)
                .free_mass_at(1, 0, initial_mass);
        }

        let mut simulation = builder.build_simulation();
        let (snapshot, live_set, tick) = {
            let prepared = simulation.prepare_tick();
            (
                prepared.snapshot.to_vec(),
                prepared.live_set.to_vec(),
                prepared.tick,
            )
        };
        let config = simulation.config().clone();
        let seed = simulation.seed();
        let target_index = simulation.grid().index(1, 0);

        let mutated_energy = initial_energy + 31;
        let mutated_mass = initial_mass + 29;
        let mutated_size = if initial_size == 6 {
            1
        } else {
            initial_size + 1
        };
        let target_cell = simulation
            .grid_mut()
            .get_mut(target_index)
            .expect("target cell should exist");
        *target_cell = ProgramBuilder::new()
            .code(&vec![op::NOP; mutated_size])
            .id(initial_id.wrapping_add(1))
            .free_energy(mutated_energy)
            .free_mass(mutated_mass)
            .build();

        pass1_local(
            simulation.grid_mut(),
            &snapshot,
            &live_set,
            &config,
            tick,
            seed,
        );

        let source_program = simulation
            .grid()
            .get(simulation.grid().index(0, 0))
            .expect("source cell should exist")
            .program
            .as_ref()
            .expect("source program should exist");
        let expected_stack = if initial_has_program {
            vec![
                i16::try_from(initial_energy).expect("energy should fit in i16"),
                i16::try_from(initial_mass).expect("mass should fit in i16"),
                i16::try_from(initial_size).expect("size should fit in i16"),
                i16::from(initial_id),
            ]
        } else {
            vec![
                i16::try_from(initial_energy).expect("energy should fit in i16"),
                i16::try_from(initial_mass).expect("mass should fit in i16"),
                0,
                0,
            ]
        };

        assert_eq!(
            source_program.stack, expected_stack,
            "snapshot isolation stack mismatch in randomized case {case_idx}"
        );
        assert_eq!(
            source_program.registers.flag, !initial_has_program,
            "senseId flag mismatch in randomized case {case_idx}"
        );
        assert_eq!(
            source_program.registers.ip, 0,
            "IP should wrap after four local sensing ops in randomized case {case_idx}"
        );
    }
}

#[test]
fn additive_transfer_resolution_is_order_independent_across_randomized_cases() {
    let mut rng = WyRand::with_seed(0x5d64_9f31_22a7_118c);

    for case_idx in 0..128 {
        let source0_energy = rng.next_u32() % 40;
        let source1_energy = rng.next_u32() % 40;
        let source2_energy = rng.next_u32() % 40;
        let source0_mass = rng.next_u32() % 40;
        let source1_mass = rng.next_u32() % 40;
        let source2_mass = rng.next_u32() % 40;
        let target_energy = rng.next_u32() % 40;
        let target_mass = rng.next_u32() % 40;

        let (base_grid, _) = WorldBuilder::new(4, 1)
            .at(
                0,
                0,
                ProgramBuilder::new()
                    .code(&[op::NOP])
                    .free_energy(source0_energy)
                    .free_mass(source0_mass),
            )
            .at(
                1,
                0,
                ProgramBuilder::new()
                    .code(&[op::NOP])
                    .free_energy(source1_energy)
                    .free_mass(source1_mass),
            )
            .at(
                2,
                0,
                ProgramBuilder::new()
                    .code(&[op::NOP])
                    .free_energy(source2_energy)
                    .free_mass(source2_mass),
            )
            .at(
                3,
                0,
                ProgramBuilder::new()
                    .code(&[op::NOP])
                    .free_energy(target_energy)
                    .free_mass(target_mass),
            )
            .build();

        let mut actions = vec![
            QueuedAction::GiveE {
                source: 0,
                target: 3,
                amount: ((rng.next_u32() % 61) as i16) - 15,
            },
            QueuedAction::GiveM {
                source: 0,
                target: 3,
                amount: ((rng.next_u32() % 61) as i16) - 15,
            },
            QueuedAction::GiveE {
                source: 1,
                target: 3,
                amount: ((rng.next_u32() % 61) as i16) - 15,
            },
            QueuedAction::GiveM {
                source: 1,
                target: 3,
                amount: ((rng.next_u32() % 61) as i16) - 15,
            },
            QueuedAction::GiveE {
                source: 2,
                target: 3,
                amount: ((rng.next_u32() % 61) as i16) - 15,
            },
            QueuedAction::GiveM {
                source: 2,
                target: 3,
                amount: ((rng.next_u32() % 61) as i16) - 15,
            },
        ];
        let mut shuffled = actions.clone();
        shuffle(&mut actions, &mut rng);
        shuffle(&mut shuffled, &mut rng);

        let mut left_grid = base_grid.clone();
        let mut right_grid = base_grid.clone();
        let left_output = pass2_nonlocal(&mut left_grid, &actions, 17, 91);
        let right_output = pass2_nonlocal(&mut right_grid, &shuffled, 17, 91);

        assert_eq!(
            left_grid, right_grid,
            "additive Pass-2 order changed the resolved grid in randomized case {case_idx}"
        );
        assert_eq!(
            left_output, right_output,
            "additive Pass-2 order changed the write markers in randomized case {case_idx}"
        );
    }
}

fn shuffle<T>(items: &mut [T], rng: &mut WyRand) {
    for index in (1..items.len()).rev() {
        let swap_index =
            (rng.next_u64() % u64::try_from(index + 1).expect("index should fit")) as usize;
        items.swap(index, swap_index);
    }
}

#[test]
fn lineage_invariants_hold_after_a_seeded_replay() {
    let mut simulation = WorldBuilder::new(6, 4)
        .seed(0xbead_5eed)
        .configure(|config| {
            config.r_energy = 0.4;
            config.r_mass = 0.4;
            // Probabilities must be 0, 1, or exactly 2^-k (SPEC v0.3.0).
            // 0.05 sits between 2^-5 and 2^-4; 2^-4 keeps the maintenance
            // pressure that makes programs die and orphans appear.
            config.maintenance_rate = 0.0625;
            config.p_spawn = 0.25;
            config.mutation_base_log2 = 4;
        })
        .at(
            1,
            1,
            ProgramBuilder::new()
                .code(&[op::push(1), op::APPEND_ADJ, op::BOOT])
                .free_energy(40)
                .free_mass(40),
        )
        .at(
            4,
            2,
            ProgramBuilder::new()
                .code(&[op::push(1), op::APPEND_ADJ, op::BOOT])
                .free_energy(40)
                .free_mass(40),
        )
        .build_simulation();

    run_ticks(&mut simulation, 60);

    let cell_count = simulation.grid().len();
    let mut seen_any = false;

    for (cell_index, cell) in simulation.grid().cells().iter().enumerate() {
        let Some(program) = cell.program.as_ref() else {
            continue;
        };
        seen_any = true;
        let lineage = program.lineage;

        assert!(
            !lineage.uid.is_none(),
            "program in cell {cell_index} has no uid"
        );
        let site = lineage
            .uid
            .site(cell_count)
            .expect("a non-NONE uid must decode to a creation site");

        if program.live {
            assert!(
                lineage.birth_tick().is_some(),
                "live program in cell {cell_index} has no birth tick"
            );
        }

        // A parent must have been created no later than its child. Both uids
        // decode to the tick their program first materialized, so this is a
        // direct causality check on the recorded lineage.
        if !lineage.parent.is_none() {
            let parent_site = lineage
                .parent
                .site(cell_count)
                .expect("a non-NONE parent uid must decode to a creation site");
            assert!(
                parent_site.tick <= site.tick,
                "program in cell {cell_index} claims a parent created at tick {} but was itself created at tick {}",
                parent_site.tick,
                site.tick
            );
            assert!(
                lineage.generation >= 1,
                "a program with a parent must have generation >= 1"
            );
        } else {
            assert_eq!(
                lineage.generation, 0,
                "a lineage root must have generation 0"
            );
        }
    }

    assert!(seen_any, "the fixture should leave programs to inspect");
}
