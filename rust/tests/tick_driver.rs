#[macro_use]
mod helpers;

use helpers::{run_ticks, ProgramBuilder, WorldBuilder};
use proteus::op;
use proteus::{Direction, ProgramOrigin};

#[test]
fn abandoned_inert_program_pays_maintenance_and_can_die() {
    let mut simulation = WorldBuilder::new(1, 1)
        .configure(|config| {
            config.maintenance_rate_log2 = Some(0);
            config.maintenance_exponent = 1.0;
            config.d_energy_log2 = None;
            config.d_mass_log2 = None;
            config.r_energy = 0.0;
            config.r_mass = 0.0;
            config.p_spawn_log2 = None;
        })
        .at(
            0,
            0,
            ProgramBuilder::new()
                .code(&[op::DUP, op::DROP])
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
            config.maintenance_rate_log2 = Some(0);
            config.maintenance_exponent = 1.0;
            config.d_energy_log2 = None;
            config.d_mass_log2 = None;
            config.r_energy = 0.0;
            config.r_mass = 0.0;
            config.p_spawn_log2 = None;
        })
        .at(
            0,
            0,
            ProgramBuilder::new()
                .code(&[op::push(1), op::WRITE_ADJ])
                .free_energy(1),
        )
        .at(
            1,
            0,
            ProgramBuilder::new()
                .code(&[op::DUP, op::DROP])
                .live(false)
                .abandonment_timer(9)
                .open(true),
        )
        .build_simulation();

    simulation.run_tick();

    assert_program!(
        simulation.grid(),
        (1, 0),
        code == &[op::push(1), op::DROP][..],
        abandonment_timer == 0
    );
}

#[test]
fn spontaneous_spawn_waits_until_next_tick_to_act_and_age() {
    let mut simulation = WorldBuilder::new(1, 1)
        .configure(|config| {
            config.r_mass = 100.0;
            config.d_mass_log2 = None;
            config.p_spawn_log2 = Some(0);
            config.r_energy = 0.0;
            config.d_energy_log2 = None;
            config.maintenance_rate_log2 = None;
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
    let cell = simulation
        .grid()
        .get(simulation.grid().index(0, 0))
        .expect("cell should exist");
    assert!(cell.free_mass > 1);
    assert_eq!(cell.bg_mass, 0);

    run_ticks(&mut simulation, 1);

    assert_program!(simulation.grid(), (0, 0), age == 1, did_nop == true);
}

#[test]
fn tick_report_counts_spontaneous_spawn_as_birth() {
    let mut simulation = WorldBuilder::new(1, 1)
        .configure(|config| {
            config.r_mass = 100.0;
            config.d_mass_log2 = None;
            config.p_spawn_log2 = Some(0);
            config.r_energy = 0.0;
            config.d_energy_log2 = None;
            config.maintenance_rate_log2 = None;
            config.mutation_base_log2 = 32;
            config.mutation_background_log2 = 32;
        })
        .build_simulation();

    let report = simulation.run_tick_report();

    assert_eq!(report.births, 1);
    assert_eq!(report.spawn_births, 1);
    assert_eq!(report.boot_births, 0);
    assert_eq!(report.deaths, 0);
    assert_eq!(report.mutations, 0);
}

#[test]
fn booted_abandoned_inert_program_skips_maintenance_on_boot_tick() {
    let mut simulation = WorldBuilder::new(2, 1)
        .configure(|config| {
            config.maintenance_rate_log2 = Some(0);
            config.maintenance_exponent = 1.0;
            config.inert_grace_ticks = 0;
            config.d_energy_log2 = None;
            config.d_mass_log2 = None;
            config.r_energy = 0.0;
            config.r_mass = 0.0;
            config.p_spawn_log2 = None;
            config.mutation_base_log2 = 32;
            config.mutation_background_log2 = 32;
        })
        .at(0, 0, ProgramBuilder::new().code(&[op::BOOT]))
        .at(
            1,
            0,
            ProgramBuilder::new()
                .code(&[op::NOP])
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
            config.d_energy_log2 = Some(0);
            config.d_mass_log2 = Some(0);
            config.r_energy = 0.0;
            config.r_mass = 0.0;
            config.maintenance_rate_log2 = None;
            config.t_cap = 2.5;
            config.mutation_base_log2 = 32;
            config.mutation_background_log2 = 32;
        })
        .at(
            0,
            0,
            ProgramBuilder::new()
                .code(&[op::NOP])
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
            config.maintenance_rate_log2 = None;
            config.d_energy_log2 = None;
            config.d_mass_log2 = None;
            config.r_energy = 0.0;
            config.r_mass = 0.0;
            config.p_spawn_log2 = None;
            config.mutation_base_log2 = 0;
            config.mutation_background_log2 = 0;
        })
        .at(0, 0, ProgramBuilder::new().code(&[op::NOP, op::NOP]))
        .build_simulation();

    simulation.run_tick();

    let cell = simulation
        .grid()
        .get(simulation.grid().index(0, 0))
        .expect("cell should exist");
    let program = cell.program.as_ref().expect("program should exist");
    assert_ne!(program.code, vec![op::NOP, op::NOP]);
    assert_eq!(program.age, 1);
}

#[test]
fn maintenance_destroyed_instructions_do_not_become_free_mass() {
    let mut simulation = WorldBuilder::new(1, 1)
        .configure(|config| {
            config.maintenance_rate_log2 = Some(0);
            config.maintenance_exponent = 1.0;
            config.d_energy_log2 = None;
            config.d_mass_log2 = None;
            config.r_energy = 0.0;
            config.r_mass = 0.0;
            config.p_spawn_log2 = None;
        })
        .at(0, 0, ProgramBuilder::new().code(&[op::NOP]))
        .build_simulation();

    simulation.run_tick();

    assert_cell!(
        simulation.grid(),
        (0, 0),
        has_program == false,
        free_mass == 0
    );
}

#[test]
fn tick_report_counts_boot_birth_separately() {
    let mut simulation = WorldBuilder::new(2, 1)
        .configure(|config| {
            config.maintenance_rate_log2 = None;
            config.d_energy_log2 = None;
            config.d_mass_log2 = None;
            config.r_energy = 0.0;
            config.r_mass = 0.0;
            config.p_spawn_log2 = None;
            config.mutation_base_log2 = 32;
            config.mutation_background_log2 = 32;
        })
        .at(0, 0, ProgramBuilder::new().code(&[op::BOOT]))
        .at(
            1,
            0,
            ProgramBuilder::new()
                .code(&[op::NOP])
                .live(false)
                .open(true),
        )
        .build_simulation();

    let report = simulation.run_tick_report();

    assert_eq!(report.births, 1);
    assert_eq!(report.boot_births, 1);
    assert_eq!(report.spawn_births, 0);
}

#[test]
fn tick_report_tracks_packet_count() {
    // emit sends a directed radiation packet; after one tick the packet
    // should be in-flight and counted in the report.
    let mut simulation = WorldBuilder::new(3, 1)
        .configure(|config| {
            config.maintenance_rate_log2 = None;
            config.d_energy_log2 = None;
            config.d_mass_log2 = None;
            config.r_energy = 0.0;
            config.r_mass = 0.0;
            config.p_spawn_log2 = None;
            config.mutation_base_log2 = 32;
            config.mutation_background_log2 = 32;
        })
        .at(
            0,
            0,
            ProgramBuilder::new()
                .code(&[op::push(1), op::EMIT])
                .free_energy(1),
        )
        .build_simulation();

    let report = simulation.run_tick_report();

    assert_eq!(report.packet_count, 1);
}

#[test]
fn built_and_booted_neighbor_records_its_parent_generation_and_birth_tick() {
    // Tick 0 queues appendAdj, which materializes an inert body in the empty
    // neighbor; the nonlocal instruction advances the ip, so tick 1 executes
    // boot against that body.
    let mut simulation = WorldBuilder::new(2, 1)
        .configure(|config| {
            config.maintenance_rate_log2 = None;
            config.d_energy_log2 = None;
            config.d_mass_log2 = None;
            config.r_energy = 0.0;
            config.r_mass = 0.0;
            config.p_spawn_log2 = None;
            config.mutation_base_log2 = 32;
            config.mutation_background_log2 = 32;
        })
        .at(
            0,
            0,
            ProgramBuilder::new()
                .code(&[op::push(op::NOP as i16), op::APPEND_ADJ, op::BOOT])
                .dir(Direction::Right)
                .free_energy(20)
                .free_mass(4),
        )
        .build_simulation();

    let cell_count = simulation.grid().len();
    let parent = simulation
        .grid()
        .get(0)
        .expect("creator cell should exist")
        .program
        .as_ref()
        .expect("creator should exist")
        .lineage;

    simulation.run_tick();

    let offspring = simulation
        .grid()
        .get(1)
        .expect("target cell should exist")
        .program
        .as_ref()
        .expect("appendAdj should have created an inert body")
        .clone();
    assert!(!offspring.live, "a fresh appendAdj body starts inert");
    assert_eq!(offspring.lineage.parent, parent.uid);
    assert_eq!(offspring.lineage.generation, parent.generation + 1);
    assert_eq!(
        offspring.lineage.birth_tick(),
        None,
        "an inert body has never been live"
    );
    let site = offspring
        .lineage
        .uid
        .site(cell_count)
        .expect("the offspring uid should decode");
    assert_eq!(site.tick, 0);
    assert_eq!(site.cell_index, 1);
    assert_eq!(site.origin, ProgramOrigin::Append);

    simulation.run_tick();

    let booted = simulation
        .grid()
        .get(1)
        .expect("target cell should exist")
        .program
        .as_ref()
        .expect("the booted program should still exist")
        .clone();
    assert!(booted.live, "boot should have made the body live");
    assert_eq!(
        booted.lineage.birth_tick(),
        Some(1),
        "boot stamps the tick the body came alive"
    );
    assert_eq!(
        booted.lineage.uid, offspring.lineage.uid,
        "boot keeps the uid"
    );
    assert_eq!(booted.lineage.parent, parent.uid);
    assert_eq!(booted.lineage.generation, parent.generation + 1);
}
