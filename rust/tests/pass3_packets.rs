#[macro_use]
mod helpers;

use helpers::{ProgramBuilder, WorldBuilder};
use proteus::{Direction, Packet};

#[test]
fn emitted_packet_is_captured_by_a_listener_after_propagation() {
    let mut simulation = WorldBuilder::new(2, 1)
        .at(
            0,
            0,
            ProgramBuilder::new()
                .code(&[0x02, 0x54])
                .dir(Direction::Right)
                .free_energy(1),
        )
        .at(1, 0, ProgramBuilder::new().code(&[0x52]))
        .build_simulation();

    let pass1 = simulation.run_pass1();
    simulation.extend_packets(pass1.emitted_packets);
    simulation.run_pass3_packets();

    assert!(simulation.packets().is_empty());
    assert_cell!(simulation.grid(), (0, 0), free_energy == 0);
    assert_cell!(simulation.grid(), (1, 0), free_energy == 1);
    assert_program!(
        simulation.grid(),
        (1, 0),
        did_listen == true,
        msg == 2,
        dir == Direction::Left,
        flag == true
    );
}

#[test]
fn listen_without_packets_leaves_flag_unchanged_in_packet_phase() {
    let mut simulation = WorldBuilder::new(1, 1)
        .at(0, 0, ProgramBuilder::new().code(&[0x52]).flag(true))
        .build_simulation();

    let pass1 = simulation.run_pass1();
    simulation.extend_packets(pass1.emitted_packets);
    simulation.run_pass3_packets();

    assert!(simulation.packets().is_empty());
    assert_cell!(simulation.grid(), (0, 0), free_energy == 0);
    assert_program!(
        simulation.grid(),
        (0, 0),
        did_listen == true,
        flag == true,
        msg == 0,
        dir == Direction::Right
    );
}

#[test]
fn uncaptured_packets_collide_into_free_energy() {
    let mut simulation = WorldBuilder::new(3, 1).build_simulation();
    simulation.extend_packets([
        Packet {
            position: 0,
            direction: Direction::Right,
            message: 11,
        },
        Packet {
            position: 2,
            direction: Direction::Left,
            message: 22,
        },
    ]);

    simulation.run_pass3_packets();

    assert!(simulation.packets().is_empty());
    assert_cell!(simulation.grid(), (1, 0), free_energy == 2);
}

#[test]
fn single_uncaptured_packet_persists_to_the_next_tick_position() {
    let mut simulation = WorldBuilder::new(3, 1).build_simulation();
    simulation.extend_packets([Packet {
        position: 0,
        direction: Direction::Right,
        message: 7,
    }]);

    simulation.run_pass3_packets();

    assert_eq!(
        simulation.packets(),
        &[Packet {
            position: 1,
            direction: Direction::Right,
            message: 7,
        }]
    );
}
