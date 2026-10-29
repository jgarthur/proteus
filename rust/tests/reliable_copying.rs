//! Exercises candidate seed protocols through ordinary VM instructions and interference.

use proteus::genome_observe::{GenomeAudit, GenomeEvent};
use proteus::{op, Cell, Direction, Grid, Program, ProgramUid, SimConfig, Simulation};

const WIDTH: usize = 7;
const PARENT: usize = 2 * WIDTH + 3;
const TARGET: usize = PARENT + 1;
const INTERFERER: usize = TARGET + 1;

fn genome(name: &str) -> Vec<u8> {
    let genomes: serde_json::Value = serde_json::from_str(include_str!(
        "../../docs/analysis/reliable-copying/genomes.json"
    ))
    .unwrap();
    serde_json::from_value(genomes[name].clone()).unwrap()
}

fn fixture(name: &str, mass: u32) -> Vec<Cell> {
    let mut cells = vec![Cell::empty(); WIDTH * WIDTH];
    let mut parent = Program::new_live(genome(name), Direction::Right, 0).unwrap();
    // Start the mechanistic assay at the copy entry, after Guardstep's foraging move.
    // Population experiments instead start every genome at IP 0.
    if name.starts_with("guardstep") {
        parent.registers.ip = 2;
    }
    cells[PARENT].program = Some(parent);
    cells[PARENT].free_energy = 10_000;
    cells[PARENT].free_mass = mass;
    cells
}

fn simulation(cells: Vec<Cell>, alpha: f64, seed: u64) -> Simulation {
    let config = SimConfig {
        width: WIDTH as u32,
        height: WIDTH as u32,
        seed,
        r_energy: 0.0,
        r_mass: 0.0,
        d_energy_log2: None,
        d_mass_log2: None,
        maintenance_rate_log2: None,
        mutation_base_log2: 63,
        mutation_background_log2: 63,
        local_action_exponent: alpha,
        ..SimConfig::default()
    };
    Simulation::from_grid(
        config,
        Grid::from_cells(WIDTH as u32, WIDTH as u32, cells).unwrap(),
    )
    .unwrap()
}

/// Queue one action after `delay` yields, then spin locally without more world actions.
fn actor(delay: usize, action: u8, operand: Option<i16>, direction: Direction) -> Cell {
    let mut code = vec![];
    for _ in 0..delay {
        // A nonpositive transfer yields one tick without changing target state.
        code.extend([op::push(0), op::GIVE_E]);
    }
    code.extend([action, op::push(-1), op::JMP]);
    let mut program = Program::new_live(code, direction, 0).unwrap();
    if let Some(value) = operand {
        program.stack.push(value);
    }
    Cell {
        program: Some(program),
        free_energy: 10_000,
        free_mass: 1_000,
        ..Cell::empty()
    }
}

fn parent_uid(simulation: &Simulation) -> ProgramUid {
    simulation.grid().cells()[PARENT]
        .program
        .as_ref()
        .unwrap()
        .lineage
        .uid
}

fn run(mut simulation: Simulation, ticks: u64) -> GenomeAudit {
    let mut audit = GenomeAudit::new(0, ticks);
    for _ in 0..ticks {
        simulation.run_tick_report_with_genomes(&mut audit);
    }
    assert_eq!(
        audit.totals.base_mutations + audit.totals.background_mutations,
        0
    );
    audit
}

fn parent_births(audit: &GenomeAudit, uid: ProgramUid) -> Vec<&GenomeEvent> {
    audit
        .births
        .iter()
        .filter(|b| b.actor == Some(uid))
        .collect()
}

fn verification_timing(alpha: f64) -> (u64, u64) {
    let code = genome("guardstep");
    let after_read = code.iter().position(|&byte| byte == op::READ_ADJ).unwrap() + 1;
    let mut sim = simulation(fixture("guardstep", 1_000), alpha, 1);
    let uid = parent_uid(&sim);
    let mut audit = GenomeAudit::new(0, 1_000);
    let mut first_read = None;
    for tick in 0..1_000 {
        sim.run_tick_report_with_genomes(&mut audit);
        if sim.grid().cells().iter().any(|cell| {
            cell.program
                .as_ref()
                .is_some_and(|p| p.lineage.uid == uid && usize::from(p.registers.ip) == after_read)
        }) {
            first_read.get_or_insert(tick);
        }
        if let Some(birth) = parent_births(&audit, uid).first() {
            return (first_read.unwrap(), birth.tick);
        }
    }
    panic!("the timing control must complete a birth");
}

/// All four genomes have a positive exact-birth control with ample resources.
#[test]
fn funded_isolated_copiers_produce_exact_offspring() {
    for alpha in [0.5, 0.55, 1.0] {
        for name in ["guardstep", "retrystep", "squarestep", "wildstep"] {
            let mut cells = fixture(name, 1_000);
            cells[PARENT].program.as_mut().unwrap().registers.ip = 0;
            let sim = simulation(cells, alpha, 1);
            let uid = parent_uid(&sim);
            let audit = run(sim, 1_000);
            let births = parent_births(&audit, uid);
            assert!(!births.is_empty(), "{name}, alpha={alpha}");
            assert_eq!(births[0].code, Some(genome(name)), "{name}, alpha={alpha}");
        }
    }
}

/// A delayed donor restores mass after real append failures; guarded/retry copies recover.
#[test]
fn retrying_copiers_wait_for_mass_instead_of_booting_a_partial_body() {
    for name in ["guardstep", "retrystep", "squarestep", "wildstep"] {
        let mut cells = fixture(name, 1);
        cells[PARENT - 1] = actor(40, op::GIVE_M, Some(300), Direction::Right);
        let sim = simulation(cells, 0.55, 1);
        let uid = parent_uid(&sim);
        let audit = run(sim, 1_000);
        assert!(audit.attempts.iter().any(|a| a.actor == uid
            && a.kind == "append"
            && !a.success
            && a.source_free_mass_before_pass2 == 0));
        let births = parent_births(&audit, uid);
        assert!(!births.is_empty(), "{name}");
        assert_eq!(
            births[0].code.as_ref() == Some(&genome(name)),
            matches!(name, "guardstep" | "retrystep"),
            "{name}"
        );
    }
}

/// An extra foreign byte during construction makes Guardstep abandon that body.
#[test]
fn guarded_copy_rejects_foreign_growth_during_construction() {
    let mut witnessed = 0;
    for (alpha, delay) in [0.5, 0.55]
        .into_iter()
        .flat_map(|a| (18..24).map(move |d| (a, d)))
    {
        let mut cells = fixture("guardstep", 1_000);
        cells[INTERFERER] = actor(delay, op::APPEND_ADJ, Some(255), Direction::Left);
        let sim = simulation(cells, alpha, 1);
        let uid = parent_uid(&sim);
        let audit = run(sim, 1_000);
        if let Some(foreign) = audit
            .events
            .iter()
            .find(|e| e.kind == "append" && e.actor != Some(uid))
        {
            witnessed += 1;
            assert_eq!(foreign.cell, TARGET);
            assert!(foreign.before_len > 0);
            assert!(!audit.births.iter().any(|b| b.uid == foreign.uid));
            // One append may already be in flight after stale local sensing.
            assert!(
                audit
                    .events
                    .iter()
                    .filter(|e| e.kind == "append"
                        && e.actor == Some(uid)
                        && e.uid == foreign.uid
                        && e.tick > foreign.tick)
                    .count()
                    <= 1
            );
        } else {
            assert!(audit
                .attempts
                .iter()
                .any(|a| a.actor != uid && a.kind == "append" && !a.success));
        }
        let births = parent_births(&audit, uid);
        assert!(
            !births.is_empty(),
            "a rejected target must not prevent later reproduction"
        );
        assert!(births.iter().all(|b| b.code == Some(genome("guardstep"))));
    }
    assert!(
        witnessed >= 8,
        "the phase sweep must actually modify most targets"
    );
}

/// Same-length writes to an unchecked early or late byte are found by the scan.
#[test]
fn guarded_copy_detects_corruption_that_length_checks_cannot_find() {
    let (first_read, _) = verification_timing(0.55);
    for (index, delay) in [(0, 100), (60, first_read as usize + 30)] {
        let mut cells = fixture("guardstep", 1_000);
        cells[INTERFERER] = actor(delay, op::WRITE_ADJ, Some(255), Direction::Left);
        cells[INTERFERER].program.as_mut().unwrap().registers.dst = index;
        let sim = simulation(cells, 0.55, 1);
        let uid = parent_uid(&sim);
        let audit = run(sim, 1_500);
        let write = audit.events.iter().find(|e| e.kind == "write").unwrap();
        assert_eq!(write.index, Some(usize::from(index)));
        assert_eq!(write.tick, delay as u64);
        assert_eq!(write.tick > first_read, index == 60);
        assert_eq!(write.before_len, write.after_len);
        assert!(!audit.births.iter().any(|b| b.uid == write.uid));
        let births = parent_births(&audit, uid);
        assert!(!births.is_empty());
        assert!(births.iter().all(|b| b.code == Some(genome("guardstep"))));
    }
}

/// A foreign boot can animate an unfinished child without its creator's approval.
#[test]
fn foreign_boot_is_an_explicit_limit_of_the_guarded_protocol() {
    let mut cells = fixture("guardstep", 1_000);
    cells[INTERFERER] = actor(20, op::BOOT, None, Direction::Left);
    let sim = simulation(cells, 0.55, 1);
    let uid = parent_uid(&sim);
    let audit = run(sim, 200);
    assert_eq!(audit.births.len(), 1);
    let birth = &audit.births[0];
    assert_ne!(birth.actor, Some(uid));
    assert_eq!(birth.parent_uid, uid);
    assert_ne!(birth.code, Some(genome("guardstep")));
    assert!(birth.after_len < genome("guardstep").len());
}

/// A mover occupies a previously sensed empty target and protects it before append resolves.
#[test]
fn guarded_copy_recovers_from_a_stale_empty_target_reading() {
    for alpha in [0.5, 0.55] {
        let mut cells = fixture("guardstep", 1_000);
        cells[INTERFERER] = actor(0, op::MOVE, None, Direction::Left);
        let sim = simulation(cells, alpha, 1);
        let uid = parent_uid(&sim);
        let audit = run(sim, 1_000);
        assert!(audit.attempts.iter().any(|a| a.actor == uid
            && a.kind == "append"
            && !a.success
            && !a.target_open_before_pass2));
        let births = parent_births(&audit, uid);
        assert!(!births.is_empty());
        assert!(births.iter().all(|b| b.code == Some(genome("guardstep"))));
    }
}

/// An append after the byte scan is rejected by the final, freshly sampled length guard.
#[test]
fn final_length_guard_catches_growth_after_verification() {
    for alpha in [0.5, 0.55, 1.0] {
        let (_, boot_tick) = verification_timing(alpha);
        let mut cells = fixture("guardstep", 1_000);
        cells[INTERFERER] = actor(
            (boot_tick - 1) as usize,
            op::APPEND_ADJ,
            Some(255),
            Direction::Left,
        );
        let sim = simulation(cells, alpha, 1);
        let uid = parent_uid(&sim);
        let audit = run(sim, 1_500);
        let append = audit
            .events
            .iter()
            .find(|e| e.kind == "append" && e.actor != Some(uid))
            .unwrap();
        assert_eq!(append.tick, boot_tick - 1);
        assert_eq!(append.before_len, genome("guardstep").len());
        assert!(!audit.births.iter().any(|b| b.uid == append.uid));
        assert!(!parent_births(&audit, uid).is_empty());
    }
}

/// A write to an already-checked byte can evade the scan; verification is not atomic.
#[test]
fn late_same_length_write_is_an_explicit_limit_of_the_guarded_protocol() {
    let (first_read, _) = verification_timing(0.55);
    let mut cells = fixture("guardstep", 1_000);
    cells[INTERFERER] = actor(
        (first_read + 1) as usize,
        op::WRITE_ADJ,
        Some(255),
        Direction::Left,
    );
    let sim = simulation(cells, 0.55, 1);
    let uid = parent_uid(&sim);
    let audit = run(sim, 1_000);
    let write = audit.events.iter().find(|e| e.kind == "write").unwrap();
    assert_eq!(write.tick, first_read + 1);
    let birth = audit.births.iter().find(|b| b.uid == write.uid).unwrap();
    assert_eq!(birth.actor, Some(uid));
    assert_eq!(birth.after_len, genome("guardstep").len());
    assert_eq!(birth.code.as_ref().unwrap()[0], 255);
    assert_ne!(birth.code, Some(genome("guardstep")));
}

/// Funding precedes the read-only scan and remains with the newborn in a quiet world.
#[test]
fn guarded_parent_funds_the_body_before_verification() {
    for (name, expected_energy) in [
        ("guardstep", 7),
        ("guardstep-zero-gift", 0),
        ("guardstep-unfunded", 0),
    ] {
        let code = genome(name);
        let after_read = code.iter().position(|&byte| byte == op::READ_ADJ).unwrap() + 1;
        let mut sim = simulation(fixture(name, 1_000), 0.55, 1);
        let uid = parent_uid(&sim);
        let mut audit = GenomeAudit::new(0, 1_000);
        let mut saw_scan = false;
        let mut saw_birth = false;
        for _ in 0..1_000 {
            sim.run_tick_report_with_genomes(&mut audit);
            if sim.grid().cells()[PARENT]
                .program
                .as_ref()
                .is_some_and(|p| p.lineage.uid == uid && usize::from(p.registers.ip) == after_read)
            {
                saw_scan = true;
                assert_eq!(sim.grid().cells()[TARGET].free_energy, expected_energy);
            }
            if let Some(birth) = parent_births(&audit, uid).first() {
                assert!(saw_scan);
                assert_eq!(birth.code, Some(code.clone()));
                assert_eq!(sim.grid().cells()[TARGET].free_energy, expected_energy);
                saw_birth = true;
                break;
            }
        }
        assert!(saw_birth, "{name} must finish its funded control");
    }
}

/// Pre-existing inert debris is never extended or booted by a fresh construction attempt.
#[test]
fn guarded_copy_leaves_preexisting_debris_alone() {
    let mut cells = fixture("guardstep", 1_000);
    cells[TARGET].program = Some(Program::new_inert(vec![255; 5], Direction::Right, 0).unwrap());
    let sim = simulation(cells, 0.55, 1);
    let debris = sim.grid().cells()[TARGET]
        .program
        .as_ref()
        .unwrap()
        .lineage
        .uid;
    let uid = parent_uid(&sim);
    let audit = run(sim, 1_000);
    assert!(!audit.events.iter().any(|e| e.uid == debris));
    assert!(!parent_births(&audit, uid).is_empty());
}

/// Maintenance can defeat the seven-energy allowance; eroded bodies are rejected.
#[test]
fn guarded_copy_rejects_bodies_eroded_during_verification() {
    let mut sim = simulation(fixture("guardstep", 1_000), 0.55, 1);
    let mut config = sim.config().clone();
    config.maintenance_rate_log2 = Some(4);
    sim = Simulation::from_grid(config, sim.grid().clone()).unwrap();
    let audit = run(sim, 1_000);
    assert!(audit.totals.maintenance_inert_deleted_bytes > 0);
    assert_eq!(audit.totals.maintenance_live_deleted_bytes, 0);
    assert!(audit.totals.constructions_started >= 2);
    assert!(audit.totals.moves > 0);
    assert!(audit
        .births
        .iter()
        .all(|b| b.code == Some(genome("guardstep"))));
}

/// An empty read returning the expected zero is rejected by its failure flag, not equality.
#[test]
fn verification_checks_read_failure_even_when_the_returned_byte_matches() {
    for suppress_flag in [false, true] {
        let mut cells = fixture("guardstep", 1_000);
        let parent = cells[PARENT].program.as_mut().unwrap();
        let read = parent.code.iter().rposition(|&b| b == op::READ).unwrap();
        let after_adjacent_read = parent.code.iter().position(|&b| b == op::READ_ADJ).unwrap() + 1;
        if suppress_flag {
            // This single-instruction ablation must defeat the early-rejection assertion.
            parent.code[after_adjacent_read] = op::push(0);
        }
        parent.registers.ip = read as u16;
        parent.registers.src = 2; // Own byte 2 is zero, just like an empty read's return value.
        parent.registers.lc = 1;
        let mut sim = simulation(cells, 0.0, 1);
        for _ in 0..6 {
            sim.run_tick_report();
        }
        let parent = sim.grid().cells()[PARENT].program.as_ref().unwrap();
        assert_eq!(parent.stack, [0, 0]);
        assert!(parent.registers.flag);
        for _ in 0..12 {
            sim.run_tick_report();
        }
        let parent = sim.grid().cells()[PARENT].program.as_ref().unwrap();
        assert_eq!(parent.registers.ip == 0, !suppress_flag);
        assert!(parent.stack.is_empty());
    }
}
