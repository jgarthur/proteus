//! Read-only pass-boundary observation for genome and lineage analysis.

use crate::{Grid, QueuedAction};

/// Boundaries separating genome-changing mechanisms in SPEC §Tick Structure.
///
/// `Local` contains net local writes/deletions; `Nonlocal` contains resolved
/// neighbor actions and movement; `Maintenance` follows the lifecycle/maintenance/
/// spawn tail; `Mutation` follows the end-of-tick bit flip. Ambient physics does
/// not edit genomes. Movement changes positions while preserving identities.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GenomePhase {
    Start,
    Local,
    Nonlocal,
    Maintenance,
    Mutation,
}

/// Receives immutable state at genome-relevant boundaries of the real driver.
///
/// `tick` is the zero-based tick being executed (the resulting world has tick
/// `tick + 1`). `actions` is the captured Pass-1 queue at Local and Nonlocal,
/// and empty at other boundaries. Queue entries are attempts, not successes.
/// Callbacks are serial, including in Rayon builds. Implementations must keep
/// their own output/error state; neither mutable engine state nor RNG is exposed.
pub trait GenomeObserver {
    /// Allows the compiler to erase callbacks for ordinary execution.
    const ENABLED: bool = true;

    fn observe(&mut self, tick: u64, phase: GenomePhase, grid: &Grid, actions: &[QueuedAction]);
}

impl GenomeObserver for () {
    const ENABLED: bool = false;

    fn observe(&mut self, _: u64, _: GenomePhase, _: &Grid, _: &[QueuedAction]) {}
}

use std::collections::{BTreeMap, BTreeSet};

use crate::{Program, ProgramUid};

/// Mechanisms observed changing a creator while a body was under construction.
/// Categories can overlap. `unobserved` detects caller edits between ticks.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize)]
pub struct CreatorChanges {
    pub local_edit: bool,
    pub neighbor_edit: bool,
    pub maintenance: bool,
    pub base_mutation: bool,
    pub background_mutation: bool,
    pub death: bool,
    pub unobserved: bool,
}

impl CreatorChanges {
    fn any(&self) -> bool {
        self.local_edit
            || self.neighbor_edit
            || self.maintenance
            || self.base_mutation
            || self.background_mutation
            || self.death
            || self.unobserved
    }

    fn merge(&mut self, other: &Self) {
        self.local_edit |= other.local_edit;
        self.neighbor_edit |= other.neighbor_edit;
        self.maintenance |= other.maintenance;
        self.base_mutation |= other.base_mutation;
        self.background_mutation |= other.background_mutation;
        self.death |= other.death;
        self.unobserved |= other.unobserved;
    }
}

/// Deterministic event emitted in tick/phase/cell or captured-action order.
///
/// Source and target identities are independent of cell position. For a boot,
/// `code` is the exact newborn genome before maintenance and mutation; `matches_creator_at_creation`
/// compares it with the recorded creator's genome at first append. That comparison
/// is absent when observation started after construction. Contributor counts count
/// successful append/write/delete actors before boot, including identical writes.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct GenomeEvent {
    pub tick: u64,
    pub kind: &'static str,
    pub uid: ProgramUid,
    pub parent_uid: ProgramUid,
    pub cell: usize,
    pub actor: Option<ProgramUid>,
    pub was_live: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub boot_actors: Option<Vec<ProgramUid>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creator_changed_during_construction: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creator_changes: Option<CreatorChanges>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub matches_creator_at_boot: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub construction_maintenance_deleted_bytes: Option<u64>,
    pub before_len: usize,
    pub after_len: usize,
    pub index: Option<usize>,
    pub value: Option<u8>,
    pub old_value: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<Vec<u8>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub matches_creator_at_creation: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contributors: Option<usize>,
}

/// One queued edit, boot or move attempt, including failure outcomes.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct ActionAttempt {
    pub tick: u64,
    pub kind: &'static str,
    pub actor: ProgramUid,
    pub source_cell: usize,
    pub target_cell: usize,
    pub success: bool,
    pub source_free_mass_before_pass2: u32,
    pub source_free_energy_before_pass2: u32,
    pub target_uid_before_pass2: Option<ProgramUid>,
    pub target_open_before_pass2: bool,
    pub value: Option<u8>,
}

/// Counts across the entire observed interval, independent of event retention.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize)]
pub struct GenomeTotals {
    pub constructions_started: u64,
    pub constructions_died_unbooted: u64,
    pub appends: u64,
    pub appends_to_live: u64,
    pub appends_by_nonparent: u64,
    pub writes: u64,
    pub adjacent_deletions: u64,
    pub local_changed_program_ticks: u64,
    pub local_deleted_bytes: u64,
    pub maintenance_deleted_bytes: u64,
    pub maintenance_deaths: u64,
    pub maintenance_live_deleted_bytes: u64,
    pub maintenance_inert_deleted_bytes: u64,
    pub maintenance_live_deaths: u64,
    pub maintenance_inert_deaths: u64,
    pub base_mutations: u64,
    pub background_mutations: u64,
    pub boots: u64,
    pub boots_by_noncreator: u64,
    pub boots_without_creator: u64,
    pub boots_with_known_construction: u64,
    pub matching_creator_at_creation_boots: u64,
    pub boots_creator_changed: u64,
    pub boots_creator_missing: u64,
    pub boots_with_creator_at_boot: u64,
    pub matching_creator_at_boot_boots: u64,
    pub boots_body_maintained: u64,
    pub clean_single_contributor_boots: u64,
    pub clean_matching_boots: u64,
    pub mixed_contributor_boots: u64,
    pub moves: u64,
    pub spawns: u64,
}

#[derive(Clone)]
struct ObservedProgram {
    uid: ProgramUid,
    parent: ProgramUid,
    code: Vec<u8>,
    live: bool,
    flag: bool,
    background_used: u32,
    free_mass: u32,
    free_energy: u32,
    open: bool,
}

impl From<&Program> for ObservedProgram {
    fn from(program: &Program) -> Self {
        Self {
            uid: program.lineage.uid,
            parent: program.lineage.parent,
            code: program.code.clone(),
            live: program.live,
            flag: program.registers.flag,
            background_used: program.tick.bg_radiation_consumed,
            free_mass: 0,
            free_energy: 0,
            open: program.tick.is_open,
        }
    }
}

struct Construction {
    creator: ProgramUid,
    creator_code: Vec<u8>,
    creator_changes: CreatorChanges,
    maintenance_deleted_bytes: u64,
    contributors: BTreeSet<ProgramUid>,
}

/// Records construction causality and phase-separated genome changes.
///
/// Programs must have unique nonzero lineage identities (as assigned by engine
/// bootstrap). Attach from tick zero for complete construction histories. This diagnostic
/// clones code at each relevant boundary; ordinary execution pays none of that
/// cost. Retained events are limited to `[trace_start, trace_end)`; totals and
/// construction tracking cover every observed tick. Drain `events` each tick to
/// stream a bounded trace. Local events describe net changes within Pass 1, so
/// cancelling self-writes are intentionally not counted as individual operations.
#[derive(Default)]
pub struct GenomeAudit {
    previous: Vec<Option<ObservedProgram>>,
    construction: BTreeMap<ProgramUid, Construction>,
    pub totals: GenomeTotals,
    pub events: Vec<GenomeEvent>,
    /// Every boot, regardless of trace window; drain after each tick.
    pub births: Vec<GenomeEvent>,
    /// Edit/boot/move attempts within the trace interval; drain after each tick.
    pub attempts: Vec<ActionAttempt>,
    pub trace_start: u64,
    pub trace_end: u64,
}

impl GenomeAudit {
    pub fn new(trace_start: u64, trace_end: u64) -> Self {
        Self {
            trace_start,
            trace_end,
            ..Self::default()
        }
    }

    /// Bodies created during observation that have neither booted nor died.
    pub fn pending_constructions(&self) -> usize {
        self.construction.len()
    }

    fn retain(&mut self, event: GenomeEvent) {
        if event.kind == "boot" {
            self.births.push(event.clone());
        }
        if (self.trace_start..self.trace_end).contains(&event.tick) {
            self.events.push(event);
        }
    }

    fn event(
        tick: u64,
        kind: &'static str,
        cell: usize,
        before: Option<&ObservedProgram>,
        after: Option<&ObservedProgram>,
    ) -> GenomeEvent {
        GenomeEvent {
            tick,
            kind,
            cell,
            uid: after.or(before).expect("event has a program").uid,
            parent_uid: after.or(before).expect("event has a program").parent,
            actor: None,
            was_live: before.is_some_and(|p| p.live),
            boot_actors: None,
            creator_changed_during_construction: None,
            creator_changes: None,
            matches_creator_at_boot: None,
            construction_maintenance_deleted_bytes: None,
            before_len: before.map_or(0, |p| p.code.len()),
            after_len: after.map_or(0, |p| p.code.len()),
            index: None,
            value: None,
            old_value: None,
            code: None,
            matches_creator_at_creation: None,
            contributors: None,
        }
    }

    fn nonlocal(
        &mut self,
        tick: u64,
        current: &[Option<ObservedProgram>],
        actions: &[QueuedAction],
        changes: &BTreeMap<ProgramUid, CreatorChanges>,
    ) {
        let locations: BTreeMap<_, _> = current
            .iter()
            .enumerate()
            .filter_map(|(cell, p)| p.as_ref().map(|p| (p.uid, cell)))
            .collect();
        let mut booted = BTreeSet::new();
        for action in actions {
            let (source, target, kind) = match *action {
                QueuedAction::AppendAdj { source, target, .. } => (source, target, "append"),
                QueuedAction::WriteAdj { source, target, .. } => (source, target, "write"),
                QueuedAction::DelAdj { source, target, .. } => (source, target, "delete_adjacent"),
                QueuedAction::Boot { source, target } => (source, target, "boot"),
                QueuedAction::Move { source, target } => (source, target, "move"),
                _ => continue,
            };
            let actor = self.previous[source]
                .as_ref()
                .expect("queued source exists");
            // Only move can change an actor's cell during Pass 2. Failed moves
            // leave the actor at source; successful moves carry its cleared Flag.
            let actor_after = if kind == "move" {
                current[target]
                    .as_ref()
                    .filter(|p| p.uid == actor.uid)
                    .or(current[source].as_ref())
            } else {
                current[source].as_ref()
            };
            let success = actor_after.is_some_and(|p| p.uid == actor.uid && !p.flag);
            if (self.trace_start..self.trace_end).contains(&tick) {
                self.attempts.push(ActionAttempt {
                    tick,
                    kind,
                    actor: actor.uid,
                    source_cell: source,
                    target_cell: target,
                    success,
                    source_free_mass_before_pass2: actor.free_mass,
                    source_free_energy_before_pass2: actor.free_energy,
                    target_uid_before_pass2: self.previous[target].as_ref().map(|p| p.uid),
                    target_open_before_pass2: self.previous[target].as_ref().is_none_or(|p| p.open),
                    value: match *action {
                        QueuedAction::AppendAdj { value, .. }
                        | QueuedAction::WriteAdj { value, .. } => Some(value),
                        _ => None,
                    },
                });
            }
            if !success {
                continue;
            }
            if kind == "move" {
                self.totals.moves += 1;
                let mut event = Self::event(tick, kind, target, Some(actor), actor_after);
                event.actor = Some(actor.uid);
                self.retain(event);
                continue;
            }
            let before = self.previous[target].as_ref();
            // Edits commit before moves. An occupied target can carry the edited
            // body to another cell during the same pass.
            let final_cell = before.map_or(target, |p| locations[&p.uid]);
            let after = current[final_cell]
                .as_ref()
                .expect("successful target action retains a program");
            let mut event = Self::event(tick, kind, final_cell, before, Some(after));
            event.actor = Some(actor.uid);
            match *action {
                QueuedAction::AppendAdj { value, .. } => {
                    self.totals.appends += 1;
                    self.totals.appends_to_live += u64::from(before.is_some_and(|p| p.live));
                    self.totals.appends_by_nonparent += u64::from(after.parent != actor.uid);
                    event.index = Some(event.before_len);
                    event.value = Some(value);
                    if before.is_none() {
                        self.totals.constructions_started += 1;
                        self.construction.insert(
                            after.uid,
                            Construction {
                                creator: actor.uid,
                                creator_code: actor.code.clone(),
                                creator_changes: changes
                                    .get(&actor.uid)
                                    .cloned()
                                    .unwrap_or_default(),
                                maintenance_deleted_bytes: 0,
                                contributors: BTreeSet::new(),
                            },
                        );
                    }
                }
                QueuedAction::WriteAdj {
                    value, dst_cursor, ..
                } => {
                    self.totals.writes += 1;
                    let before = before.expect("write target exists");
                    let index = usize::from(dst_cursor) % before.code.len();
                    event.index = Some(index);
                    event.old_value = Some(before.code[index]);
                    event.value = Some(value);
                }
                QueuedAction::DelAdj { dst_cursor, .. } => {
                    self.totals.adjacent_deletions += 1;
                    let before = before.expect("delete target exists");
                    let index = usize::from(dst_cursor) % before.code.len();
                    event.index = Some(index);
                    event.old_value = Some(before.code[index]);
                }
                QueuedAction::Boot { .. } => {
                    // Multiple boot actors can succeed on one inert body. Emit
                    // a birth once, while preserving all edit contributors.
                    if !booted.insert(after.uid) {
                        continue;
                    }
                    event.boot_actors = Some(
                        actions
                            .iter()
                            .filter_map(|action| {
                                let QueuedAction::Boot {
                                    source,
                                    target: boot_target,
                                } = *action
                                else {
                                    return None;
                                };
                                if boot_target != target {
                                    return None;
                                }
                                current[source].as_ref().filter(|p| !p.flag).map(|p| p.uid)
                            })
                            .collect(),
                    );
                    event.code = Some(after.code.clone());
                    if let Some(construction) = self.construction.remove(&after.uid) {
                        let faithful = construction.creator_code == after.code;
                        event.matches_creator_at_creation = Some(faithful);
                        event.creator_changed_during_construction =
                            Some(construction.creator_changes.any());
                        event.creator_changes = Some(construction.creator_changes.clone());
                        event.construction_maintenance_deleted_bytes =
                            Some(construction.maintenance_deleted_bytes);
                        // Compare with the creator immediately before this boot
                        // pass; concurrent neighbor writes are separate events.
                        let creator = self
                            .previous
                            .iter()
                            .flatten()
                            .find(|p| p.uid == construction.creator);
                        event.matches_creator_at_boot = creator.map(|p| p.code == after.code);
                        self.totals.boots_creator_changed +=
                            u64::from(construction.creator_changes.any());
                        self.totals.boots_creator_missing += u64::from(creator.is_none());
                        self.totals.boots_with_creator_at_boot += u64::from(creator.is_some());
                        self.totals.matching_creator_at_boot_boots +=
                            u64::from(event.matches_creator_at_boot == Some(true));
                        self.totals.boots_body_maintained +=
                            u64::from(construction.maintenance_deleted_bytes > 0);
                        let boot_actors = event.boot_actors.as_ref().expect("boot actors recorded");
                        let foreign_booter =
                            boot_actors.iter().any(|uid| *uid != construction.creator);
                        let creator_booted = boot_actors.contains(&construction.creator);
                        self.totals.boots_by_noncreator += u64::from(foreign_booter);
                        self.totals.boots_without_creator += u64::from(!creator_booted);
                        let clean = !foreign_booter
                            && creator_booted
                            && !construction.creator_changes.any()
                            && creator.is_some()
                            && construction.maintenance_deleted_bytes == 0
                            && construction.contributors.len() == 1;
                        self.totals.clean_single_contributor_boots += u64::from(clean);
                        self.totals.clean_matching_boots += u64::from(clean && faithful);
                        event.contributors = Some(construction.contributors.len());
                        self.totals.boots_with_known_construction += 1;
                        self.totals.matching_creator_at_creation_boots += u64::from(faithful);
                        self.totals.mixed_contributor_boots +=
                            u64::from(construction.contributors.len() > 1);
                    }
                }
                _ => unreachable!(),
            }
            if kind != "boot" {
                if let Some(construction) = self.construction.get_mut(&after.uid) {
                    construction.contributors.insert(actor.uid);
                }
            }
            self.retain(event);
        }
        // Count live transitions rather than successful boot actions (multi-boot).
        self.totals.boots += self.previous.iter().zip(current).filter(|(old, new)| {
            matches!((old, new), (Some(a), Some(b)) if a.uid == b.uid && !a.live && b.live)
        }).count() as u64;
    }
}

impl GenomeObserver for GenomeAudit {
    fn observe(&mut self, tick: u64, phase: GenomePhase, grid: &Grid, actions: &[QueuedAction]) {
        let current: Vec<_> = grid
            .cells()
            .iter()
            .map(|cell| {
                cell.program.as_ref().map(|program| {
                    let mut observed = ObservedProgram::from(program);
                    observed.free_mass = cell.free_mass;
                    observed.free_energy = cell.free_energy;
                    observed
                })
            })
            .collect();
        let current_by_uid: BTreeMap<_, _> = current.iter().flatten().map(|p| (p.uid, p)).collect();
        let mut changes = BTreeMap::new();
        for old in self.previous.iter().flatten() {
            let new = current_by_uid.get(&old.uid).copied();
            if new.is_some_and(|new| old.code == new.code) {
                continue;
            }
            let mut cause = CreatorChanges {
                death: new.is_none(),
                ..CreatorChanges::default()
            };
            match phase {
                GenomePhase::Start => cause.unobserved = true,
                GenomePhase::Local => cause.local_edit = true,
                GenomePhase::Nonlocal => cause.neighbor_edit = true,
                GenomePhase::Maintenance => cause.maintenance = true,
                GenomePhase::Mutation => {
                    let background = new.is_some_and(|p| p.background_used > 0);
                    cause.background_mutation = background;
                    cause.base_mutation = !background;
                }
            }
            changes.insert(old.uid, cause);
        }
        for (body_uid, construction) in &mut self.construction {
            // A creator edit concurrent with boot cannot affect this finished
            // body. Creation-pass edits do matter for subsequent copy ticks.
            if phase == GenomePhase::Nonlocal
                && current_by_uid.get(body_uid).is_some_and(|p| p.live)
            {
                continue;
            }
            if let Some(change) = changes.get(&construction.creator) {
                construction.creator_changes.merge(change);
            }
        }
        if phase == GenomePhase::Nonlocal {
            self.nonlocal(tick, &current, actions, &changes);
        } else if phase != GenomePhase::Start {
            let previous = std::mem::take(&mut self.previous);
            for (cell, new) in current.iter().enumerate() {
                let old = previous[cell].as_ref();
                let new = new.as_ref();
                match phase {
                    GenomePhase::Local => {
                        if let (Some(a), Some(b)) = (old, new) {
                            assert_eq!(
                                a.uid, b.uid,
                                "identity changed outside nonlocal/tail phase"
                            );
                            if a.code != b.code {
                                self.totals.local_changed_program_ticks += 1;
                                self.totals.local_deleted_bytes +=
                                    a.code.len().saturating_sub(b.code.len()) as u64;
                                let mut event = Self::event(tick, "local_net_edit", cell, old, new);
                                event.actor = Some(a.uid);
                                event.code = Some(b.code.clone());
                                self.retain(event);
                            }
                        }
                    }
                    GenomePhase::Maintenance => {
                        if let Some(a) = old {
                            let survivor = new.filter(|b| b.uid == a.uid);
                            let lost = a.code.len() - survivor.map_or(0, |b| b.code.len());
                            if lost > 0 {
                                self.totals.maintenance_deleted_bytes += lost as u64;
                                if let Some(construction) = self.construction.get_mut(&a.uid) {
                                    construction.maintenance_deleted_bytes += lost as u64;
                                }
                                self.totals.maintenance_deaths += u64::from(survivor.is_none());
                                if a.live {
                                    self.totals.maintenance_live_deleted_bytes += lost as u64;
                                    self.totals.maintenance_live_deaths +=
                                        u64::from(survivor.is_none());
                                } else {
                                    self.totals.maintenance_inert_deleted_bytes += lost as u64;
                                    self.totals.maintenance_inert_deaths +=
                                        u64::from(survivor.is_none());
                                }
                                self.retain(Self::event(tick, "maintenance", cell, old, survivor));
                            }
                            if survivor.is_none() && self.construction.remove(&a.uid).is_some() {
                                self.totals.constructions_died_unbooted += 1;
                            }
                        }
                        if let Some(b) = new.filter(|b| old.is_none_or(|a| a.uid != b.uid)) {
                            self.totals.spawns += 1;
                            let mut event = Self::event(tick, "spawn", cell, None, Some(b));
                            event.code = Some(b.code.clone());
                            self.retain(event);
                        }
                    }
                    GenomePhase::Mutation => {
                        if let (Some(a), Some(b)) = (old, new) {
                            assert_eq!(
                                a.uid, b.uid,
                                "identity changed outside nonlocal/tail phase"
                            );
                            if a.code != b.code {
                                let background = b.background_used > 0;
                                self.totals.background_mutations += u64::from(background);
                                self.totals.base_mutations += u64::from(!background);
                                let mut event = Self::event(
                                    tick,
                                    if background {
                                        "mutation_background"
                                    } else {
                                        "mutation_base"
                                    },
                                    cell,
                                    old,
                                    new,
                                );
                                let index = a
                                    .code
                                    .iter()
                                    .zip(&b.code)
                                    .position(|(a, b)| a != b)
                                    .expect("changed byte");
                                event.index = Some(index);
                                event.old_value = Some(a.code[index]);
                                event.value = Some(b.code[index]);
                                self.retain(event);
                            }
                        }
                    }
                    _ => unreachable!(),
                }
            }
        }
        if phase == GenomePhase::Start {
            debug_assert!(current.iter().flatten().all(|p| !p.uid.is_none()));
            debug_assert_eq!(
                current
                    .iter()
                    .flatten()
                    .map(|p| p.uid)
                    .collect::<BTreeSet<_>>()
                    .len(),
                current.iter().flatten().count()
            );
        }
        self.previous = current;
    }
}
