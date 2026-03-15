#[macro_use]
mod helpers;

use helpers::{ProgramBuilder, WorldBuilder};
use proteus::{Direction, Packet, Pass1Output, QueuedAction, PROGRAM_SIZE_CAP};

#[test]
fn listen_is_flag_neutral_and_opens_the_cell() {
    let mut simulation = WorldBuilder::new(1, 1)
        .at(0, 0, ProgramBuilder::new().code(&[0x52]).flag(true))
        .build_simulation();

    let output = simulation.run_pass1();

    assert_eq!(output, Pass1Output::default());
    assert_program!(
        simulation.grid(),
        (0, 0),
        flag == true,
        did_listen == true,
        is_open == true,
        ip == 0
    );
}

#[test]
fn absorb_expands_footprint_without_opening_the_cell() {
    let mut simulation = WorldBuilder::new(1, 1)
        .at(
            0,
            0,
            ProgramBuilder::new()
                .code(&[0x51, 0x40, 0x51, 0x51, 0x51, 0x51])
                .dir(Direction::Up),
        )
        .build_simulation();

    simulation.run_pass1();

    assert_program!(
        simulation.grid(),
        (0, 0),
        absorb_count == 4,
        absorb_dir == Some(Direction::Up),
        is_open == false,
        ip == 0
    );
}

#[test]
fn synthesize_additional_cost_failure_halts_without_advancing_ip() {
    let mut simulation = WorldBuilder::new(1, 1)
        .configure(|config| config.n_synth = 2)
        .at(0, 0, ProgramBuilder::new().code(&[0x58]).free_energy(1))
        .build_simulation();

    let output = simulation.run_pass1();

    assert_eq!(output, Pass1Output::default());
    assert_cell!(simulation.grid(), (0, 0), free_energy == 0, free_mass == 0);
    assert_program!(
        simulation.grid(),
        (0, 0),
        flag == true,
        is_open == true,
        ip == 0
    );
}

#[test]
fn failed_nonlocal_operand_capture_consumes_base_cost_and_stops_tick() {
    let mut simulation = WorldBuilder::new(1, 1)
        .at(
            0,
            0,
            ProgramBuilder::new().code(&[0x5e, 0x01]).free_energy(1),
        )
        .build_simulation();

    let output = simulation.run_pass1();

    assert_eq!(output.actions, Vec::<QueuedAction>::new());
    assert_eq!(output.emitted_packets, Vec::<Packet>::new());
    assert_cell!(simulation.grid(), (0, 0), free_energy == 0);
    assert_program!(
        simulation.grid(),
        (0, 0),
        flag == true,
        is_open == false,
        ip == 1
    );
}

#[test]
fn successful_nonlocal_queue_attempt_stops_before_later_local_work() {
    let mut simulation = WorldBuilder::new(2, 1)
        .at(
            0,
            0,
            ProgramBuilder::new()
                .code(&[0x01, 0x5f, 0x07])
                .free_energy(1)
                .free_mass(3),
        )
        .build_simulation();

    let output = simulation.run_pass1();

    assert_eq!(
        output.actions,
        vec![QueuedAction::AppendAdj {
            source: 0,
            target: 1,
            value: 1
        }]
    );
    assert_program!(simulation.grid(), (0, 0), ip == 2, stack == &[][..]);
}

#[test]
fn emit_creates_packet_and_consumes_base_energy() {
    let mut simulation = WorldBuilder::new(1, 1)
        .at(
            0,
            0,
            ProgramBuilder::new()
                .code(&[0x02, 0x54])
                .dir(Direction::Left)
                .free_energy(1),
        )
        .build_simulation();

    let output = simulation.run_pass1();

    assert_eq!(
        output.emitted_packets,
        vec![Packet {
            position: 0,
            direction: Direction::Left,
            message: 2
        }]
    );
    assert_cell!(simulation.grid(), (0, 0), free_energy == 0);
    assert_program!(simulation.grid(), (0, 0), ip == 0, stack == &[][..]);
}

#[test]
fn push_literal_at_stack_capacity_sets_flag_and_leaves_stack_unchanged() {
    let full_stack: Vec<i16> = (0..usize::from(PROGRAM_SIZE_CAP))
        .map(|value| i16::try_from(value).expect("stack test values should fit in i16"))
        .collect();
    let mut simulation = WorldBuilder::new(1, 1)
        .at(0, 0, ProgramBuilder::new().code(&[0x07]).stack(&full_stack))
        .build_simulation();

    simulation.run_pass1();

    let program = simulation
        .grid()
        .get(simulation.grid().index(0, 0))
        .expect("cell should exist")
        .program
        .as_ref()
        .expect("program should exist");
    assert!(program.registers.flag);
    assert_eq!(program.registers.ip, 0);
    assert_eq!(program.stack, full_stack);
}

#[test]
fn drop_on_empty_stack_sets_flag_and_leaves_stack_empty() {
    let mut simulation = WorldBuilder::new(1, 1)
        .at(0, 0, ProgramBuilder::new().code(&[0x11]))
        .build_simulation();

    simulation.run_pass1();

    assert_program!(
        simulation.grid(),
        (0, 0),
        flag == true,
        ip == 0,
        stack == &[][..]
    );
}
