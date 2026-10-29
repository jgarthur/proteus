//! Checks that genome observation preserves execution and separates edit causes.

use proteus::genome_observe::{GenomeObserver, GenomePhase};
use proteus::{op, Cell, Direction, Grid, Program, QueuedAction, SimConfig, Simulation};

#[derive(Default)]
struct RecordingObserver {
    states: Vec<(u64, GenomePhase, Grid, Vec<QueuedAction>)>,
}

impl GenomeObserver for RecordingObserver {
    fn observe(&mut self, tick: u64, phase: GenomePhase, grid: &Grid, actions: &[QueuedAction]) {
        self.states
            .push((tick, phase, grid.clone(), actions.to_vec()));
    }
}

/// Exercises nontrivial random draws, movement, construction, packets and decay.
#[test]
fn observing_every_boundary_preserves_the_complete_simulation() {
    let config = SimConfig {
        width: 8,
        height: 8,
        seed: 718,
        mutation_base_log2: 3,
        mutation_background_log2: 2,
        p_spawn_log2: Some(4),
        ..SimConfig::default()
    };
    let mut cells = vec![Cell::empty(); 64];
    for (index, code) in [
        vec![83, 66, 48, 85, 95, 49, 100, 64, 99],
        vec![op::EMIT, op::LISTEN, op::MOVE],
        vec![op::ABSORB, op::COLLECT, op::DEL],
    ]
    .into_iter()
    .enumerate()
    {
        cells[index * 8].program = Some(Program::new_live(code, Direction::Right, 0).unwrap());
        cells[index * 8].free_energy = 30;
        cells[index * 8].free_mass = 30;
    }
    let mut plain = Simulation::from_grid(config, Grid::from_cells(8, 8, cells).unwrap()).unwrap();
    let mut observed = plain.clone();
    let mut observer = RecordingObserver::default();
    let mut mutations = 0;
    for tick in 0..100 {
        observer.states.clear();
        let expected = plain.run_tick_report();
        let actual = observed.run_tick_report_with_genomes(&mut observer);
        mutations += actual.mutations;
        assert_eq!(expected, actual);
        assert_eq!(plain, observed, "complete state diverged at tick {tick}");
        assert_eq!(
            observer
                .states
                .iter()
                .map(|entry| entry.1)
                .collect::<Vec<_>>(),
            [
                GenomePhase::Start,
                GenomePhase::Local,
                GenomePhase::Nonlocal,
                GenomePhase::Maintenance,
                GenomePhase::Mutation
            ]
        );
        assert!(observer.states.iter().all(|entry| entry.0 == tick));
    }
    assert!(
        mutations > 0,
        "the parity fixture must exercise drawing paths"
    );
}

/// A deterministic maintenance truncation precedes a deterministic mutation.
#[test]
fn boundaries_distinguish_maintenance_from_mutation_in_one_tick() {
    let config = SimConfig {
        width: 1,
        height: 1,
        seed: 1,
        r_energy: 0.0,
        r_mass: 0.0,
        d_energy_log2: None,
        d_mass_log2: None,
        maintenance_rate_log2: Some(0),
        maintenance_exponent: 0.0,
        mutation_base_log2: 0,
        mutation_background_log2: 0,
        ..SimConfig::default()
    };
    let mut cell = Cell::empty();
    cell.program = Some(Program::new_live(vec![op::NOP; 3], Direction::Right, 0).unwrap());
    let mut simulation =
        Simulation::from_grid(config, Grid::from_cells(1, 1, vec![cell]).unwrap()).unwrap();
    let mut observer = RecordingObserver::default();
    let report = simulation.run_tick_report_with_genomes(&mut observer);
    let code_at = |i: usize| {
        observer.states[i].2.cells()[0]
            .program
            .as_ref()
            .unwrap()
            .code
            .clone()
    };
    assert_eq!(code_at(2), vec![op::NOP; 3]);
    assert_eq!(code_at(3), vec![op::NOP; 2]);
    let before = code_at(3);
    let after = code_at(4);
    assert_eq!(
        before
            .iter()
            .zip(after)
            .map(|(a, b)| (a ^ b).count_ones())
            .sum::<u32>(),
        1
    );
    assert_eq!(report.base_mutations, 1);
}

fn quiet_config(width: u32) -> SimConfig {
    SimConfig {
        width,
        height: 1,
        seed: 19,
        r_energy: 0.0,
        r_mass: 0.0,
        d_energy_log2: None,
        d_mass_log2: None,
        maintenance_rate_log2: None,
        mutation_base_log2: 63,
        mutation_background_log2: 63,
        local_action_exponent: 1.0,
        ..SimConfig::default()
    }
}

/// An append targets the pre-move cell but edits the body carried by the move.
#[test]
fn audit_follows_an_edited_target_through_a_same_tick_move() {
    use proteus::genome_observe::GenomeAudit;
    let mut cells = vec![Cell::empty(); 4];
    let mut writer = Program::new_live(vec![op::APPEND_ADJ], Direction::Right, 0).unwrap();
    writer.stack.push(42);
    cells[0].program = Some(writer);
    cells[0].free_energy = 10;
    cells[0].free_mass = 10;
    cells[1].program =
        Some(Program::new_live(vec![op::NOP, op::MOVE], Direction::Right, 0).unwrap());
    cells[1].free_energy = 10;
    let mut simulation =
        Simulation::from_grid(quiet_config(4), Grid::from_cells(4, 1, cells).unwrap()).unwrap();
    let uid = simulation.grid().cells()[1]
        .program
        .as_ref()
        .unwrap()
        .lineage
        .uid;
    let mut audit = GenomeAudit::new(0, 1);
    simulation.run_tick_report_with_genomes(&mut audit);
    assert!(simulation.grid().cells()[1].program.is_none());
    assert_eq!(
        simulation.grid().cells()[2].program.as_ref().unwrap().code,
        [op::NOP, op::MOVE, 42]
    );
    let append = audit.events.iter().find(|e| e.kind == "append").unwrap();
    assert_eq!((append.uid, append.cell, append.value), (uid, 2, Some(42)));
    assert_eq!(audit.attempts[0].source_free_mass_before_pass2, 10);
    assert_eq!(audit.attempts[0].source_free_energy_before_pass2, 9);
    assert_eq!(audit.attempts[0].target_uid_before_pass2, Some(uid));
    assert!(audit.attempts[0].target_open_before_pass2);
    assert_eq!(audit.totals.appends_to_live, 1);
    assert_eq!(audit.totals.moves, 1);
}

/// Two successful boots of one inert body produce one event and one birth count.
#[test]
fn audit_deduplicates_multi_boot_inside_and_outside_trace_window() {
    use proteus::genome_observe::GenomeAudit;
    for end in [0, 1] {
        let mut cells = vec![Cell::empty(); 3];
        cells[0].program = Some(Program::new_live(vec![op::BOOT], Direction::Right, 0).unwrap());
        cells[1].program = Some(Program::new_inert(vec![op::NOP], Direction::Right, 0).unwrap());
        cells[2].program = Some(Program::new_live(vec![op::BOOT], Direction::Left, 0).unwrap());
        let mut simulation =
            Simulation::from_grid(quiet_config(3), Grid::from_cells(3, 1, cells).unwrap()).unwrap();
        let mut audit = GenomeAudit::new(0, end);
        let report = simulation.run_tick_report_with_genomes(&mut audit);
        assert_eq!(report.boot_births, 1);
        assert_eq!(audit.totals.boots, 1);
        assert_eq!(audit.births.len(), 1);
        assert_eq!(audit.totals.boots_with_known_construction, 0);
        assert_eq!(
            audit.events.iter().filter(|e| e.kind == "boot").count(),
            end as usize
        );
    }
}

/// Repeated same-byte writes still identify a contributor to an inert body.
#[test]
fn audit_counts_multiple_contributors_before_boot() {
    use proteus::genome_observe::GenomeAudit;
    let mut cells = vec![Cell::empty(); 3];
    let mut creator = Program::new_live(vec![op::APPEND_ADJ], Direction::Right, 0).unwrap();
    creator.stack.push(op::NOP as i16);
    cells[0].program = Some(creator);
    cells[0].free_energy = 10;
    cells[0].free_mass = 10;
    cells[2].program = Some(Program::new_live(vec![op::NOP], Direction::Left, 0).unwrap());
    cells[2].free_energy = 10;
    let mut simulation =
        Simulation::from_grid(quiet_config(3), Grid::from_cells(3, 1, cells).unwrap()).unwrap();
    let mut audit = GenomeAudit::new(0, 3);
    simulation.run_tick_report_with_genomes(&mut audit);
    // Drive a controlled sequence with two distinct persistent actor identities.
    simulation.grid_mut().cells_mut()[0]
        .program
        .as_mut()
        .unwrap()
        .code = vec![op::NOP];
    let writer = simulation.grid_mut().cells_mut()[2]
        .program
        .as_mut()
        .unwrap();
    writer.code = vec![op::WRITE_ADJ];
    writer.stack.push(op::NOP as i16);
    simulation.run_tick_report_with_genomes(&mut audit);
    simulation.grid_mut().cells_mut()[2]
        .program
        .as_mut()
        .unwrap()
        .code = vec![op::BOOT];
    simulation.run_tick_report_with_genomes(&mut audit);
    let birth = audit.events.iter().find(|e| e.kind == "boot").unwrap();
    assert_eq!(birth.contributors, Some(2));
    assert_eq!(birth.creator_changed_during_construction, Some(true));
    assert!(birth.creator_changes.as_ref().unwrap().unobserved);
    assert_eq!(birth.matches_creator_at_boot, Some(true));
    assert_eq!(audit.totals.clean_single_contributor_boots, 0);
    assert_eq!(birth.matches_creator_at_creation, Some(false));
    assert_eq!(birth.code, Some(vec![op::NOP]));
    assert_eq!(audit.totals.mixed_contributor_boots, 1);
    assert_eq!(audit.totals.writes, 1);
}

/// A source whose additional mass payment fails contributes no append event.
#[test]
fn audit_ignores_unaffordable_and_protected_appends() {
    use proteus::genome_observe::GenomeAudit;
    for protected in [false, true] {
        let mut cells = vec![Cell::empty(); 3];
        let mut writer = Program::new_live(vec![op::APPEND_ADJ], Direction::Right, 0).unwrap();
        writer.stack.push(42);
        cells[0].program = Some(writer);
        cells[0].free_energy = 10;
        cells[0].free_mass = if protected { 10 } else { 0 };
        if protected {
            cells[1].program =
                Some(Program::new_live(vec![op::ABSORB], Direction::Right, 0).unwrap());
        }
        let mut simulation =
            Simulation::from_grid(quiet_config(3), Grid::from_cells(3, 1, cells).unwrap()).unwrap();
        let mut audit = GenomeAudit::new(0, 1);
        simulation.run_tick_report_with_genomes(&mut audit);
        assert!(
            simulation.grid().cells()[0]
                .program
                .as_ref()
                .unwrap()
                .registers
                .flag
        );
        assert_eq!(audit.totals.appends, 0);
        assert!(audit.events.is_empty());
    }
}

/// A move loser must not inherit the winning mover's clear flag at its target.
#[test]
fn audit_counts_only_the_winner_of_a_move_conflict() {
    use proteus::genome_observe::GenomeAudit;
    let mut cells = vec![Cell::empty(); 3];
    for (cell, direction) in [(0, Direction::Right), (2, Direction::Left)] {
        cells[cell].program = Some(Program::new_live(vec![op::MOVE], direction, 0).unwrap());
        cells[cell].free_energy = 10;
    }
    let mut simulation =
        Simulation::from_grid(quiet_config(3), Grid::from_cells(3, 1, cells).unwrap()).unwrap();
    let mut audit = GenomeAudit::new(0, 1);
    simulation.run_tick_report_with_genomes(&mut audit);
    assert_eq!(audit.totals.moves, 1);
    let winner = simulation.grid().cells()[1]
        .program
        .as_ref()
        .unwrap()
        .lineage
        .uid;
    assert_eq!(audit.events[0].uid, winner);
}

/// Neighbor deletion records the original byte and raw-cursor modulo index.
#[test]
fn audit_records_deletion_and_filters_unpaid_deletion() {
    use proteus::genome_observe::GenomeAudit;
    for affordable in [false, true] {
        let mut cells = vec![Cell::empty(); 3];
        let mut attacker = Program::new_live(vec![op::DEL_ADJ], Direction::Right, 0).unwrap();
        attacker.registers.dst = 4;
        cells[0].program = Some(attacker);
        cells[0].free_energy = if affordable { 10 } else { 1 };
        cells[1].program =
            Some(Program::new_live(vec![op::NOP, 254, 255], Direction::Right, 0).unwrap());
        cells[1].free_energy = 3;
        let mut simulation =
            Simulation::from_grid(quiet_config(3), Grid::from_cells(3, 1, cells).unwrap()).unwrap();
        let mut audit = GenomeAudit::new(0, 1);
        simulation.run_tick_report_with_genomes(&mut audit);
        assert_eq!(audit.totals.adjacent_deletions, u64::from(affordable));
        if affordable {
            let deletion = audit
                .events
                .iter()
                .find(|e| e.kind == "delete_adjacent")
                .unwrap();
            assert_eq!((deletion.index, deletion.old_value), (Some(1), Some(254)));
        } else {
            assert!(audit.events.is_empty());
        }
    }
}

/// Cause totals equal engine totals under actual draws, deaths and spontaneous births.
#[test]
fn audit_counters_match_engine_reports_across_stochastic_ticks() {
    use proteus::genome_observe::GenomeAudit;
    for seed in [1, 2, 9] {
        let config = SimConfig {
            width: 8,
            height: 8,
            seed,
            r_energy: 1.0,
            r_mass: 1.0,
            maintenance_rate_log2: Some(2),
            p_spawn_log2: Some(1),
            mutation_base_log2: 2,
            mutation_background_log2: 1,
            ..SimConfig::default()
        };
        let mut simulation = Simulation::new(config).unwrap();
        let mut program = Program::new_live(vec![op::APPEND_ADJ], Direction::Right, 0).unwrap();
        program.stack = vec![op::NOP as i16; 20];
        // from_grid assigns distinct seed identities to this populated fixture.
        simulation.grid_mut().cells_mut()[0].program = Some(program);
        simulation.grid_mut().cells_mut()[0].free_mass = 20;
        let mut simulation =
            Simulation::from_grid(simulation.config().clone(), simulation.grid().clone()).unwrap();
        let mut audit = GenomeAudit::new(0, 0);
        let mut totals = proteus::EventTotals::default();
        for _ in 0..200 {
            totals.record(simulation.run_tick_report_with_genomes(&mut audit));
            assert_eq!(audit.totals.boots, totals.boot_births);
            assert_eq!(audit.totals.spawns, totals.spawn_births);
            assert_eq!(audit.totals.maintenance_deaths, totals.deaths);
            assert_eq!(audit.totals.base_mutations, totals.base_mutations);
            assert_eq!(
                audit.totals.background_mutations,
                totals.background_mutations
            );
        }
        assert!(totals.spawn_births > 0 && totals.deaths > 0 && totals.base_mutations > 0);
        assert!(audit.totals.maintenance_deleted_bytes > 0);
    }
}

/// Maintenance of an inert body is retained as a pre-boot construction cause.
#[test]
fn audit_attributes_maintenance_to_the_body_before_boot() {
    use proteus::genome_observe::GenomeAudit;
    let mut cells = vec![Cell::empty(); 3];
    let mut parent = Program::new_live(vec![op::APPEND_ADJ], Direction::Right, 0).unwrap();
    parent.stack = vec![42, 43];
    cells[0].program = Some(parent);
    cells[0].free_energy = 100;
    cells[0].free_mass = 10;
    let mut config = quiet_config(3);
    config.maintenance_rate_log2 = Some(0);
    config.maintenance_exponent = 0.0;
    config.inert_grace_ticks = 0;
    let mut simulation =
        Simulation::from_grid(config, Grid::from_cells(3, 1, cells).unwrap()).unwrap();
    let mut audit = GenomeAudit::new(0, 3);
    simulation.run_tick_report_with_genomes(&mut audit);
    simulation.run_tick_report_with_genomes(&mut audit);
    simulation.grid_mut().cells_mut()[0]
        .program
        .as_mut()
        .unwrap()
        .code = vec![op::BOOT];
    simulation.run_tick_report_with_genomes(&mut audit);
    let birth = audit.events.iter().find(|e| e.kind == "boot").unwrap();
    assert_eq!(birth.construction_maintenance_deleted_bytes, Some(1));
    assert_eq!(audit.totals.boots_body_maintained, 1);
    assert_eq!(audit.totals.maintenance_inert_deleted_bytes, 1);
    assert_eq!(audit.totals.maintenance_live_deleted_bytes, 0);
    let maintenance = audit
        .events
        .iter()
        .find(|e| e.kind == "maintenance")
        .unwrap();
    assert!(!maintenance.was_live);
}

/// Background-funded instruction costs select the stressed mutation cause.
#[test]
fn audit_records_the_background_mutation_byte() {
    use proteus::genome_observe::GenomeAudit;
    let mut cells = vec![Cell::empty(); 3];
    let mut parent = Program::new_live(vec![op::APPEND_ADJ], Direction::Right, 0).unwrap();
    parent.stack.push(42);
    cells[0].program = Some(parent);
    cells[0].bg_radiation = 1;
    cells[0].free_mass = 1;
    cells[2].program = Some(Program::new_live(vec![op::NOP], Direction::Left, 0).unwrap());
    cells[2].free_energy = 100;
    let mut config = quiet_config(3);
    config.mutation_background_log2 = 0;
    let mut simulation =
        Simulation::from_grid(config, Grid::from_cells(3, 1, cells).unwrap()).unwrap();
    let mut audit = GenomeAudit::new(0, 1);
    let report = simulation.run_tick_report_with_genomes(&mut audit);
    assert_eq!(report.background_mutations, 1);
    assert_eq!(audit.totals.background_mutations, 1);
    assert_eq!(audit.totals.base_mutations, 0);
    let mutation = audit
        .events
        .iter()
        .find(|e| e.kind == "mutation_background")
        .unwrap();
    assert_eq!(mutation.index, Some(0));
    assert_eq!(mutation.old_value, Some(op::APPEND_ADJ));
    assert_eq!(
        (mutation.old_value.unwrap() ^ mutation.value.unwrap()).count_ones(),
        1
    );
    simulation.grid_mut().cells_mut()[2]
        .program
        .as_mut()
        .unwrap()
        .code = vec![op::BOOT];
    simulation.run_tick_report_with_genomes(&mut audit);
    assert!(
        audit.births[0]
            .creator_changes
            .as_ref()
            .unwrap()
            .background_mutation
    );
}

/// Driftkin boots before its first append but first animates a two-byte body.
#[test]
fn driftkin_trace_resolves_the_literal_byte_and_boot_order_prediction() {
    use proteus::genome_observe::GenomeAudit;
    let config = SimConfig {
        width: 64,
        height: 64,
        seed: 1,
        r_mass: 1.0,
        ..SimConfig::default()
    };
    let mut simulation = Simulation::new(config).unwrap();
    let bootstrap = serde_json::from_value(serde_json::json!({"programs": [{
        "x": 32, "y": 32, "free_energy": 20, "free_mass": 12,
        "code": [81,64,0,66,83,48,0,48,81,85,0,93,81,49,64,49,81,100,81,99,81,99,81,81,81,81,81,81,49,64,85,66,95,48,66,85,48,95,81]
    }], "environment": []})).unwrap();
    proteus::apply_bootstrap(&mut simulation, &bootstrap).unwrap();
    let mut audit = GenomeAudit::new(0, 24);
    for _ in 0..24 {
        simulation.run_tick_report_with_genomes(&mut audit);
    }
    assert_eq!(
        (
            audit.attempts[0].tick,
            audit.attempts[0].kind,
            audit.attempts[0].success
        ),
        (0, "boot", false)
    );
    let first_append = audit.attempts.iter().find(|a| a.kind == "append").unwrap();
    assert_eq!(
        (first_append.tick, first_append.value, first_append.success),
        (3, Some(39), true)
    );
    let first_birth = &audit.births[0];
    assert_eq!(first_birth.tick, 23);
    assert_eq!(first_birth.code, Some(vec![39, 39]));
    assert_eq!(first_birth.construction_maintenance_deleted_bytes, Some(0));
    assert_eq!(
        audit.totals.base_mutations + audit.totals.background_mutations,
        0
    );
}

/// A funded solitary Squarestep copies its whole genome and boots it itself.
#[test]
fn audit_has_a_positive_clean_match_without_retained_trace_events() {
    use proteus::genome_observe::GenomeAudit;
    let code = vec![83, 66, 48, 85, 95, 49, 100, 64, 99];
    let mut cells = vec![Cell::empty(); 8];
    cells[0].program = Some(Program::new_live(code.clone(), Direction::Right, 0).unwrap());
    cells[0].free_energy = 100;
    cells[0].free_mass = 100;
    let mut config = quiet_config(8);
    config.local_action_exponent = 0.55;
    let mut simulation =
        Simulation::from_grid(config, Grid::from_cells(8, 1, cells).unwrap()).unwrap();
    let mut audit = GenomeAudit::default();
    for _ in 0..12 {
        simulation.run_tick_report_with_genomes(&mut audit);
    }
    assert_eq!(audit.totals.boots, 1);
    assert_eq!(audit.totals.clean_single_contributor_boots, 1);
    assert_eq!(audit.totals.clean_matching_boots, 1);
    assert_eq!(audit.totals.matching_creator_at_boot_boots, 1);
    assert_eq!(audit.totals.constructions_started, 1);
    assert_eq!(audit.totals.constructions_died_unbooted, 0);
    assert_eq!(audit.pending_constructions(), 0);
    assert_eq!(audit.births.len(), 1);
    assert_eq!(audit.births[0].code, Some(code));
    assert_eq!(audit.births[0].matches_creator_at_creation, Some(true));
    assert!(audit.events.is_empty());
}

/// Creator mutation and creator death are separate recorded construction causes.
#[test]
fn audit_separates_creator_mutation_from_maintenance_death() {
    use proteus::genome_observe::GenomeAudit;
    for dies in [false, true] {
        let mut cells = vec![Cell::empty(); 3];
        let mut parent = Program::new_live(vec![op::APPEND_ADJ], Direction::Right, 0).unwrap();
        parent.stack.push(42);
        cells[0].program = Some(parent);
        cells[0].free_energy = 1;
        cells[0].free_mass = 1;
        cells[2].program = Some(Program::new_live(vec![op::NOP], Direction::Left, 0).unwrap());
        cells[2].free_energy = 100;
        let mut config = quiet_config(3);
        config.mutation_base_log2 = if dies { 63 } else { 0 };
        config.maintenance_rate_log2 = if dies { Some(0) } else { None };
        let mut simulation =
            Simulation::from_grid(config, Grid::from_cells(3, 1, cells).unwrap()).unwrap();
        let mut audit = GenomeAudit::new(0, 2);
        simulation.run_tick_report_with_genomes(&mut audit);
        simulation.grid_mut().cells_mut()[2]
            .program
            .as_mut()
            .unwrap()
            .code = vec![op::BOOT];
        simulation.run_tick_report_with_genomes(&mut audit);
        let birth = &audit.births[0];
        let causes = birth.creator_changes.as_ref().unwrap();
        assert_eq!(causes.base_mutation, !dies);
        assert_eq!(causes.maintenance, dies);
        assert_eq!(causes.death, dies);
        assert!(!causes.unobserved && !causes.local_edit && !causes.neighbor_edit);
        assert_eq!(birth.matches_creator_at_boot.is_none(), dies);
        assert_eq!(audit.totals.boots_creator_missing, u64::from(dies));
        assert_eq!(audit.totals.boots_without_creator, 1);
        assert_eq!(audit.totals.boots_by_noncreator, 1);
        assert_eq!(audit.totals.clean_single_contributor_boots, 0);
    }
}

/// A body lost to maintenance is a failed construction, not a missing denominator.
#[test]
fn audit_balances_started_dead_and_pending_constructions() {
    use proteus::genome_observe::GenomeAudit;
    let mut cells = vec![Cell::empty(); 3];
    let mut parent = Program::new_live(vec![op::APPEND_ADJ], Direction::Right, 0).unwrap();
    parent.stack.push(42);
    cells[0].program = Some(parent);
    cells[0].free_energy = 100;
    cells[0].free_mass = 1;
    let mut config = quiet_config(3);
    config.maintenance_rate_log2 = Some(0);
    config.inert_grace_ticks = 0;
    let mut simulation =
        Simulation::from_grid(config, Grid::from_cells(3, 1, cells).unwrap()).unwrap();
    let mut audit = GenomeAudit::default();
    simulation.run_tick_report_with_genomes(&mut audit);
    assert_eq!(audit.totals.constructions_started, 1);
    assert_eq!(audit.pending_constructions(), 1);
    simulation.run_tick_report_with_genomes(&mut audit);
    assert_eq!(audit.pending_constructions(), 0);
    assert_eq!(audit.totals.constructions_died_unbooted, 1);
    assert_eq!(audit.totals.maintenance_inert_deaths, 1);
    assert_eq!(audit.totals.maintenance_live_deaths, 0);
}

/// A neighbor editing the creator in the boot pass cannot change the completed copy.
#[test]
fn creator_neighbor_changes_stop_before_the_boot_pass() {
    use proteus::genome_observe::GenomeAudit;
    let mut cells = vec![Cell::empty(); 4];
    let mut parent =
        Program::new_live(vec![op::APPEND_ADJ, op::NOP, op::BOOT], Direction::Right, 0).unwrap();
    parent.stack.push(42);
    cells[0].program = Some(parent);
    cells[0].free_energy = 100;
    cells[0].free_mass = 1;
    cells[3].program = Some(Program::new_live(vec![op::NOP], Direction::Right, 0).unwrap());
    cells[3].free_energy = 100;
    let mut simulation =
        Simulation::from_grid(quiet_config(4), Grid::from_cells(4, 1, cells).unwrap()).unwrap();
    let mut audit = GenomeAudit::new(0, 2);
    simulation.run_tick_report_with_genomes(&mut audit);
    let attacker = simulation.grid_mut().cells_mut()[3]
        .program
        .as_mut()
        .unwrap();
    attacker.code = vec![op::WRITE_ADJ];
    attacker.stack.push(42);
    simulation.run_tick_report_with_genomes(&mut audit);
    assert_eq!(audit.totals.writes, 1);
    assert_eq!(audit.births.len(), 1);
    assert!(
        !audit.births[0]
            .creator_changes
            .as_ref()
            .unwrap()
            .neighbor_edit
    );
    assert_eq!(audit.totals.clean_single_contributor_boots, 1);
}

/// Local deletion and an earlier neighbor write remain distinct creator causes.
#[test]
fn audit_distinguishes_local_and_neighbor_creator_edits() {
    use proteus::genome_observe::GenomeAudit;
    for neighbor in [false, true] {
        let mut cells = vec![Cell::empty(); 4];
        let code = if neighbor {
            vec![op::APPEND_ADJ, op::NOP, op::NOP, op::BOOT]
        } else {
            vec![op::APPEND_ADJ, op::DEL, op::BOOT]
        };
        let mut parent = Program::new_live(code, Direction::Right, 0).unwrap();
        parent.stack.push(42);
        cells[0].program = Some(parent);
        cells[0].free_energy = 100;
        cells[0].free_mass = 1;
        cells[3].program = Some(Program::new_live(vec![op::NOP], Direction::Right, 0).unwrap());
        cells[3].free_energy = 100;
        let mut config = quiet_config(4);
        config.local_action_exponent = 0.0;
        let mut simulation =
            Simulation::from_grid(config, Grid::from_cells(4, 1, cells).unwrap()).unwrap();
        let mut audit = GenomeAudit::new(0, 4);
        simulation.run_tick_report_with_genomes(&mut audit);
        if neighbor {
            let attacker = simulation.grid_mut().cells_mut()[3]
                .program
                .as_mut()
                .unwrap();
            attacker.code = vec![op::WRITE_ADJ];
            attacker.stack.push(42);
        }
        for _ in 0..3 {
            simulation.run_tick_report_with_genomes(&mut audit);
        }
        let causes = audit.births[0].creator_changes.as_ref().unwrap();
        assert_eq!(causes.local_edit, !neighbor);
        assert_eq!(causes.neighbor_edit, neighbor);
        assert!(!causes.unobserved && !causes.maintenance && !causes.base_mutation);
    }
}

/// Co-booting with the creator counts as foreign involvement, not creator absence.
#[test]
fn audit_excludes_foreign_coboots_from_the_clean_group() {
    use proteus::genome_observe::GenomeAudit;
    let mut cells = vec![Cell::empty(); 3];
    let mut parent =
        Program::new_live(vec![op::APPEND_ADJ, op::BOOT], Direction::Right, 0).unwrap();
    parent.stack.push(42);
    cells[0].program = Some(parent);
    cells[0].free_energy = 100;
    cells[0].free_mass = 1;
    cells[2].program = Some(Program::new_live(vec![op::NOP], Direction::Left, 0).unwrap());
    let mut simulation =
        Simulation::from_grid(quiet_config(3), Grid::from_cells(3, 1, cells).unwrap()).unwrap();
    let mut audit = GenomeAudit::default();
    simulation.run_tick_report_with_genomes(&mut audit);
    simulation.grid_mut().cells_mut()[2]
        .program
        .as_mut()
        .unwrap()
        .code = vec![op::BOOT];
    simulation.run_tick_report_with_genomes(&mut audit);
    assert_eq!(audit.births.len(), 1);
    assert_eq!(audit.births[0].boot_actors.as_ref().unwrap().len(), 2);
    assert_eq!(audit.totals.boots_by_noncreator, 1);
    assert_eq!(audit.totals.boots_without_creator, 0);
    assert_eq!(audit.totals.clean_single_contributor_boots, 0);
}

/// A creator edit during the first append can affect later bytes of that construction.
#[test]
fn creator_neighbor_changes_include_the_creation_pass() {
    use proteus::genome_observe::GenomeAudit;
    let mut cells = vec![Cell::empty(); 4];
    let mut parent =
        Program::new_live(vec![op::NOP, op::APPEND_ADJ, op::BOOT], Direction::Right, 0).unwrap();
    parent.stack.push(42);
    cells[0].program = Some(parent);
    cells[0].free_energy = 100;
    cells[0].free_mass = 1;
    let mut attacker = Program::new_live(vec![op::WRITE_ADJ], Direction::Right, 0).unwrap();
    attacker.stack.push(42);
    cells[3].program = Some(attacker);
    cells[3].free_energy = 100;
    let mut simulation =
        Simulation::from_grid(quiet_config(4), Grid::from_cells(4, 1, cells).unwrap()).unwrap();
    let mut audit = GenomeAudit::default();
    simulation.run_tick_report_with_genomes(&mut audit);
    simulation.run_tick_report_with_genomes(&mut audit);
    assert_eq!(audit.totals.writes, 1);
    assert!(
        audit.births[0]
            .creator_changes
            .as_ref()
            .unwrap()
            .neighbor_edit
    );
    assert_eq!(audit.totals.clean_single_contributor_boots, 0);
}
