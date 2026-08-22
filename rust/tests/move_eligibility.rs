//! Tick-start eligibility must follow a program that moves during Pass 2.
//!
//! `SPEC.md` line 252 scopes end-of-tick eligibility to *programs*:
//!
//! > Also record the set of programs that are **live at tick start**. Only those
//! > programs are eligible to execute in Pass 1, pay maintenance this tick, age at
//! > end of tick, and mutate at end of tick.
//!
//! The engine previously used only position-indexed `Vec<bool>` masks over cells
//! (`live_set` / `existed_set`). A successful `move` therefore relocated the program
//! to a cell whose masks described the previous occupant (empty), causing it to skip
//! maintenance, aging, and mutation. Eligibility now also lives on `TickState`, which
//! travels with the program through Pass 2.

mod helpers;

use helpers::{ProgramBuilder, WorldBuilder};
use proteus::{op, Simulation};

const SOURCE: usize = 0;
const TARGET: usize = 1;
const START_AGE: u32 = 7;
const START_ENERGY: u32 = 64;

/// Builds a two-cell world running `code`, with maintenance forced to `maintenance_rate_log2`.
///
/// Ambient input and decay are disabled so the only energy movements are the
/// instruction cost and maintenance.
fn build_world(code: &[u8], maintenance_rate_log2: Option<u32>) -> Simulation {
    WorldBuilder::new(2, 1)
        .seed(0x_11_0e)
        .configure(move |config| {
            config.r_energy = 0.0;
            config.r_mass = 0.0;
            config.d_energy_log2 = None;
            config.d_mass_log2 = None;
            config.p_spawn_log2 = None;
            config.maintenance_rate_log2 = maintenance_rate_log2;
            config.maintenance_exponent = 1.0;
            config.local_action_exponent = 1.0;
            config.inert_grace_ticks = 0;
            // Mutate every live program on every tick.
            config.mutation_base_log2 = 0;
            config.mutation_background_log2 = 0;
        })
        .at(
            0,
            0,
            ProgramBuilder::new()
                .code(code)
                .free_energy(START_ENERGY)
                .age(START_AGE),
        )
        .build_simulation()
}

/// Runs one tick and reports the energy spent by the program's occupied cell.
///
/// Comparing this across two maintenance rates isolates the maintenance charge from
/// the instruction's own cost, which a bare `energy_after < energy_before` assertion
/// cannot do - a `move` costs energy whether or not maintenance was applied.
fn energy_spent_in_one_tick(
    code: &[u8],
    maintenance_rate_log2: Option<u32>,
    occupied: usize,
) -> u32 {
    let mut simulation = build_world(code, maintenance_rate_log2);
    simulation.run_tick_report();
    let cell = simulation.grid().get(occupied).expect("cell should exist");
    assert!(
        cell.program.is_some(),
        "program should occupy cell {occupied} after the tick"
    );
    START_ENERGY - cell.free_energy
}

/// Control: a program that stays put is charged maintenance, ages, and mutates.
#[test]
fn stationary_program_is_charged_maintenance_and_ages_and_mutates() {
    let mut simulation = build_world(&[op::NOP, op::NOP], Some(0));
    let report = simulation.run_tick_report();

    let cell = simulation.grid().get(SOURCE).expect("cell should exist");
    let program = cell.program.as_ref().expect("program should still exist");

    assert_eq!(program.age, START_AGE + 1, "stationary program should age");
    assert_eq!(report.mutations, 1, "stationary program should mutate");

    let without = energy_spent_in_one_tick(&[op::NOP, op::NOP], None, SOURCE);
    let with = energy_spent_in_one_tick(&[op::NOP, op::NOP], Some(0), SOURCE);
    assert!(
        with > without,
        "stationary program should be charged maintenance \
         (spent {with} with maintenance vs {without} without)"
    );
}

/// The move itself works: the program relocates and is not treated as a newborn.
#[test]
fn move_relocates_the_program_without_marking_it_newborn() {
    let mut simulation = build_world(&[op::MOVE, op::NOP], Some(0));
    simulation.run_tick_report();

    assert!(
        simulation
            .grid()
            .get(SOURCE)
            .expect("cell should exist")
            .program
            .is_none(),
        "source cell should be empty after a successful move"
    );
    let program = simulation
        .grid()
        .get(TARGET)
        .expect("cell should exist")
        .program
        .as_ref()
        .expect("program should have moved into the target cell")
        .clone();
    assert!(
        !program.tick.is_newborn,
        "a moved program is not newborn; it existed before the move"
    );
}

#[test]
fn moved_program_ages_and_mutates() {
    let mut simulation = build_world(&[op::MOVE, op::NOP], Some(0));
    let report = simulation.run_tick_report();

    let program = simulation
        .grid()
        .get(TARGET)
        .expect("cell should exist")
        .program
        .as_ref()
        .expect("program should have moved into the target cell")
        .clone();

    assert_eq!(
        program.age,
        START_AGE + 1,
        "moved program should age: SPEC.md:326 ages all programs live at tick start"
    );
    assert_eq!(
        report.mutations, 1,
        "moved program should mutate: SPEC.md:483 scopes mutation to programs live at tick start"
    );
}

#[test]
fn moved_program_is_charged_maintenance() {
    let without = energy_spent_in_one_tick(&[op::MOVE, op::NOP], None, TARGET);
    let with = energy_spent_in_one_tick(&[op::MOVE, op::NOP], Some(0), TARGET);

    assert!(
        with > without,
        "moved program should be charged maintenance: SPEC.md:324 charges every program \
         that existed at tick start (spent {with} with maintenance vs {without} without)"
    );
}

#[test]
fn appended_inert_program_skips_maintenance_on_its_creation_tick() {
    let mut simulation = WorldBuilder::new(2, 1)
        .configure(|config| {
            config.r_energy = 0.0;
            config.r_mass = 0.0;
            config.d_energy_log2 = None;
            config.d_mass_log2 = None;
            config.p_spawn_log2 = None;
            config.maintenance_rate_log2 = Some(0);
            config.inert_grace_ticks = 0;
        })
        .at(
            SOURCE as u32,
            0,
            ProgramBuilder::new()
                .code(&[op::APPEND_ADJ])
                .stack(&[i16::from(op::NOP)])
                .free_energy(1)
                .free_mass(1),
        )
        .build_simulation();

    let report = simulation.run_tick_report();

    let target = simulation
        .grid()
        .get(TARGET)
        .expect("target cell should exist");
    let program = target.program.as_ref().unwrap_or_else(|| {
        panic!(
            "new inert program should survive its creation tick: report={report:?}, grid={:?}",
            simulation.grid().cells()
        )
    });
    assert!(!program.live);
    assert!(!program.tick.is_newborn);
    assert!(!program.tick.existed_at_tick_start);
}
