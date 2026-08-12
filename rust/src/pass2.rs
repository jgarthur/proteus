//! Executes Pass 2, where queued nonlocal actions resolve against targets.

use crate::config::PROGRAM_SIZE_CAP;
use crate::grid::Grid;
use crate::model::{Cell, Direction, Program, QueuedAction};
use crate::random::cell_rng;

const EXCLUSIVE_TIE_SALT: u64 = 0x8d51_4c2f_d5b3_7a11;
const APPEND_CREATE_SALT: u64 = 0xa4e2_9c61_7f33_b58d;

/// Collects the Pass 2 outputs needed by later phases.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pass2Output {
    pub incoming_writes: Vec<bool>,
    pub booted_programs: u32,
}

impl Pass2Output {
    /// Allocates a fresh Pass 2 output buffer for one grid size.
    pub fn new(cell_count: usize) -> Self {
        Self {
            incoming_writes: vec![false; cell_count],
            booted_programs: 0,
        }
    }
}

/// Resolves all queued nonlocal actions in the spec's class order.
pub fn pass2_nonlocal(
    grid: &mut Grid,
    actions: &[QueuedAction],
    tick: u64,
    seed: u64,
) -> Pass2Output {
    let mut output = Pass2Output::new(grid.len());
    if actions.is_empty() {
        return output;
    }

    let (candidates, invalid_sources) = stage_exclusive(grid, actions);
    resolve_reads(grid, actions);
    resolve_additive_transfers(grid, actions);
    resolve_exclusive(grid, candidates, invalid_sources, tick, seed, &mut output);

    output
}

/// Stores the prevalidated data needed to compete in one exclusive target group.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ExclusiveCandidate {
    action: QueuedAction,
    source: usize,
    target: usize,
    strength: u32,
    size: u16,
    target_strength: u32,
    target_has_program: bool,
}

/// Records a successful move to apply after winner resolution finishes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct MoveCommit {
    source: usize,
    target: usize,
}

/// Records an append-into-empty-cell creation to apply after conflicts resolve.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct AppendCreateCommit {
    target: usize,
    value: u8,
}

/// Stages exclusive candidates against the immutable pre-Pass-2 state.
fn stage_exclusive(grid: &Grid, actions: &[QueuedAction]) -> (Vec<ExclusiveCandidate>, Vec<usize>) {
    let mut candidates = Vec::new();
    let mut invalid_sources = Vec::new();

    for action in actions {
        let Some((source, _)) = exclusive_endpoints(*action) else {
            continue;
        };

        if let Some(candidate) = validate_exclusive(*action, grid) {
            candidates.push(candidate);
        } else {
            invalid_sources.push(source);
        }
    }

    (candidates, invalid_sources)
}

/// Resolves all read-only queued actions in place.
fn resolve_reads(grid: &mut Grid, actions: &[QueuedAction]) {
    for action in actions {
        let QueuedAction::ReadAdj {
            source,
            target,
            src_cursor,
        } = *action
        else {
            continue;
        };

        let target_value = grid
            .get(target)
            .expect("target cell should exist")
            .program
            .as_ref()
            .map(|target_program| {
                let read_index = usize::from(src_cursor % target_program.size());
                target_program.code[read_index]
            });
        let source_cell = grid.get_mut(source).expect("source cell should exist");

        if let Some(target_value) = target_value {
            let pushed = push_stack(source_cell, i16::from(target_value));
            if pushed {
                program_mut(source_cell).registers.src =
                    program(source_cell).registers.src.wrapping_add(1);
                set_flag(source_cell, false);
            }
        } else {
            let _ = push_stack(source_cell, 0);
            set_flag(source_cell, true);
        }
    }
}

/// Resolves additive transfers after staging caps against pre-transfer resources.
fn resolve_additive_transfers(grid: &mut Grid, actions: &[QueuedAction]) {
    let mut energy_transfers = Vec::new();
    let mut mass_transfers = Vec::new();

    for action in actions {
        match *action {
            QueuedAction::GiveE {
                source,
                target,
                amount,
            } => {
                if amount <= 0 {
                    continue;
                }

                let transferable = (amount as u32).min(
                    grid.get(source)
                        .expect("source cell should exist")
                        .free_energy,
                );
                energy_transfers.push((source, target, transferable));
                set_flag(
                    grid.get_mut(source).expect("source cell should exist"),
                    false,
                );
            }
            QueuedAction::GiveM {
                source,
                target,
                amount,
            } => {
                if amount <= 0 {
                    continue;
                }

                let transferable = (amount as u32).min(
                    grid.get(source)
                        .expect("source cell should exist")
                        .free_mass,
                );
                mass_transfers.push((source, target, transferable));
                set_flag(
                    grid.get_mut(source).expect("source cell should exist"),
                    false,
                );
            }
            _ => {}
        }
    }

    for (source, target, amount) in energy_transfers {
        grid.get_mut(source)
            .expect("source cell should exist")
            .free_energy -= amount;
        grid.get_mut(target)
            .expect("target cell should exist")
            .free_energy += amount;
    }
    for (source, target, amount) in mass_transfers {
        grid.get_mut(source)
            .expect("source cell should exist")
            .free_mass -= amount;
        grid.get_mut(target)
            .expect("target cell should exist")
            .free_mass += amount;
    }
}

/// Resolves all exclusive actions target by target.
fn resolve_exclusive(
    grid: &mut Grid,
    mut candidates: Vec<ExclusiveCandidate>,
    invalid_sources: Vec<usize>,
    tick: u64,
    seed: u64,
    output: &mut Pass2Output,
) {
    if candidates.is_empty() && invalid_sources.is_empty() {
        return;
    }

    let mut moves = Vec::new();
    let mut creates = Vec::new();

    for source in invalid_sources {
        set_flag(
            grid.get_mut(source).expect("source cell should exist"),
            true,
        );
    }

    candidates.sort_by_key(|candidate| (candidate.target, candidate.source));
    let mut group_start = 0;
    while group_start < candidates.len() {
        let target = candidates[group_start].target;
        let group_end = group_start
            + candidates[group_start..].partition_point(|candidate| candidate.target == target);
        let group = &candidates[group_start..group_end];

        if group
            .iter()
            .all(|candidate| matches!(candidate.action, QueuedAction::Boot { .. }))
        {
            for candidate in group {
                set_flag(
                    grid.get_mut(candidate.source)
                        .expect("source cell should exist"),
                    false,
                );
            }
            apply_boot_success(grid.get_mut(target).expect("target cell should exist"));
            output.booted_programs += 1;
        } else {
            let winner_index = choose_winner(group, target, tick, seed);
            for (candidate_index, candidate) in group.iter().enumerate() {
                if candidate_index == winner_index {
                    continue;
                }

                set_flag(
                    grid.get_mut(candidate.source)
                        .expect("source cell should exist"),
                    true,
                );
            }

            apply_winner(grid, group[winner_index], &mut moves, &mut creates, output);
        }

        group_start = group_end;
    }

    for commit in moves {
        apply_move_commit(grid, commit);
    }
    for commit in creates {
        apply_append_create_commit(grid, commit, tick, seed);
    }
}

/// Validates one exclusive action against the pre-Pass-2 target state.
fn validate_exclusive(action: QueuedAction, grid: &Grid) -> Option<ExclusiveCandidate> {
    let (source, target) = exclusive_endpoints(action)?;
    let source_cell = grid.get(source).expect("source cell should exist");
    let source_program = source_cell.program.as_ref()?;
    let target_cell = grid.get(target).expect("target cell should exist");

    let valid = match action {
        QueuedAction::WriteAdj { .. } => target_cell.has_program() && target_is_open(target_cell),
        QueuedAction::AppendAdj { .. } => target_cell
            .program
            .as_ref()
            .is_none_or(|program| target_is_open(target_cell) && program.size() < PROGRAM_SIZE_CAP),
        QueuedAction::DelAdj { .. } => target_cell
            .program
            .as_ref()
            .is_some_and(|program| target_is_open(target_cell) && program.size() > 1),
        QueuedAction::Move { .. } => !target_cell.has_program(),
        QueuedAction::Boot { .. } => target_cell.program.as_ref().is_some_and(Program::is_inert),
        _ => false,
    };

    valid.then_some(ExclusiveCandidate {
        action,
        source,
        target,
        strength: u32::from(source_program.size()).min(source_cell.free_energy),
        size: source_program.size(),
        target_strength: target_cell.program.as_ref().map_or(0, |program| {
            u32::from(program.size()).min(target_cell.free_energy)
        }),
        target_has_program: target_cell.has_program(),
    })
}

/// Picks the winning exclusive candidate for one target cell.
fn choose_winner(group: &[ExclusiveCandidate], target: usize, tick: u64, seed: u64) -> usize {
    let best_strength = group
        .iter()
        .map(|candidate| candidate.strength)
        .max()
        .expect("group should contain at least one candidate");

    let mut tied = group
        .iter()
        .enumerate()
        .filter_map(|(index, candidate)| (candidate.strength == best_strength).then_some(index))
        .collect::<Vec<_>>();
    tied.sort_by_key(|candidate| group[*candidate].source);

    if tied.len() == 1 {
        return tied[0];
    }

    let total_weight = tied
        .iter()
        .map(|candidate| u64::from(group[*candidate].size))
        .sum::<u64>();
    let mut rng = cell_rng(seed ^ EXCLUSIVE_TIE_SALT, tick, target as u64);
    let roll = rng.next_u64() % total_weight;

    let mut cumulative = 0_u64;
    for candidate in tied {
        cumulative += u64::from(group[candidate].size);
        if roll < cumulative {
            return candidate;
        }
    }

    unreachable!("weighted selection should return within the tied set")
}

/// Applies the effects of one winning exclusive action.
fn apply_winner(
    working: &mut Grid,
    candidate: ExclusiveCandidate,
    moves: &mut Vec<MoveCommit>,
    creates: &mut Vec<AppendCreateCommit>,
    output: &mut Pass2Output,
) {
    let source_cell = working
        .get_mut(candidate.source)
        .expect("source cell should exist");

    match candidate.action {
        QueuedAction::WriteAdj {
            value, dst_cursor, ..
        } => {
            set_flag(source_cell, false);
            program_mut(source_cell).registers.dst =
                program(source_cell).registers.dst.wrapping_add(1);

            let target_cell = working
                .get_mut(candidate.target)
                .expect("target cell should exist");
            let write_index = {
                let target_program = program(target_cell);
                usize::from(dst_cursor % target_program.size())
            };
            let target_program = program_mut(target_cell);
            target_program.code[write_index] = value;
            output.incoming_writes[candidate.target] = true;
        }
        QueuedAction::AppendAdj { value, .. } => {
            if source_cell.free_mass == 0 {
                set_flag(source_cell, true);
                return;
            }

            source_cell.free_mass -= 1;
            set_flag(source_cell, false);

            if candidate.target_has_program {
                let target_program = program_mut(
                    working
                        .get_mut(candidate.target)
                        .expect("target cell should exist"),
                );
                target_program.code.push(value);
            } else {
                creates.push(AppendCreateCommit {
                    target: candidate.target,
                    value,
                });
            }

            output.incoming_writes[candidate.target] = true;
        }
        QueuedAction::DelAdj { dst_cursor, .. } => {
            if source_cell.free_energy < candidate.target_strength {
                set_flag(source_cell, true);
                return;
            }

            source_cell.free_energy -= candidate.target_strength;
            source_cell.free_mass += 1;
            set_flag(source_cell, false);
            program_mut(source_cell).registers.dst =
                program(source_cell).registers.dst.wrapping_add(1);

            let target_cell = working
                .get_mut(candidate.target)
                .expect("target cell should exist");
            let delete_index = {
                let target_program = program(target_cell);
                usize::from(dst_cursor % target_program.size())
            };
            let target_program = program_mut(target_cell);
            target_program.code.remove(delete_index);
            if delete_index < usize::from(target_program.registers.ip) {
                target_program.registers.ip = target_program.registers.ip.wrapping_sub(1);
            }
        }
        QueuedAction::Move { .. } => {
            set_flag(source_cell, false);
            moves.push(MoveCommit {
                source: candidate.source,
                target: candidate.target,
            });
        }
        QueuedAction::Boot { .. } => {
            set_flag(source_cell, false);
            apply_boot_success(
                working
                    .get_mut(candidate.target)
                    .expect("target cell should exist"),
            );
            output.booted_programs += 1;
        }
        _ => unreachable!("winner should always be exclusive"),
    }
}

/// Marks an inert target as successfully booted.
fn apply_boot_success(target_cell: &mut Cell) {
    let target_program = program_mut(target_cell);
    target_program.live = true;
    target_program.age = 0;
    target_program.registers.ip = 0;
    target_program.tick.is_newborn = true;
    target_program.tick.is_open = false;
}

/// Applies the deferred state transfer for a successful move.
fn apply_move_commit(grid: &mut Grid, commit: MoveCommit) {
    let (program, free_energy, free_mass) = {
        let source_cell = grid
            .get_mut(commit.source)
            .expect("source cell should exist");
        (
            source_cell.program.take(),
            std::mem::take(&mut source_cell.free_energy),
            std::mem::take(&mut source_cell.free_mass),
        )
    };
    let target_cell = grid
        .get_mut(commit.target)
        .expect("target cell should exist");

    target_cell.program = program;
    target_cell.free_energy += free_energy;
    target_cell.free_mass += free_mass;
}

/// Applies the deferred inert-program creation for `appendAdj` into empty space.
fn apply_append_create_commit(grid: &mut Grid, commit: AppendCreateCommit, tick: u64, seed: u64) {
    let mut rng = cell_rng(seed ^ APPEND_CREATE_SALT, tick, commit.target as u64);
    let dir = Direction::ALL[(rng.next_u32() % Direction::ALL.len() as u32) as usize];
    let id = rng.next_u32() as u8;

    let mut program =
        Program::new_inert(vec![commit.value], dir, id).expect("append create should be valid");
    program.tick.is_open = true;

    let cell = grid
        .get_mut(commit.target)
        .expect("target cell should exist");
    cell.program = Some(program);
}

/// Returns the source and target endpoints for exclusive actions.
fn exclusive_endpoints(action: QueuedAction) -> Option<(usize, usize)> {
    match action {
        QueuedAction::WriteAdj { source, target, .. }
        | QueuedAction::AppendAdj { source, target, .. }
        | QueuedAction::DelAdj { source, target, .. }
        | QueuedAction::Move { source, target }
        | QueuedAction::Boot { source, target } => Some((source, target)),
        _ => None,
    }
}

/// Reports whether a target cell is currently open to exclusive writes.
fn target_is_open(cell: &Cell) -> bool {
    cell.program
        .as_ref()
        .is_none_or(|program| program.tick.is_open)
}

/// Pushes a value onto a source program's stack if space remains.
fn push_stack(cell: &mut Cell, value: i16) -> bool {
    if program(cell).stack.len() >= usize::from(PROGRAM_SIZE_CAP) {
        set_flag(cell, true);
        false
    } else {
        program_mut(cell).stack.push(value);
        true
    }
}

/// Writes a boolean flag back into a source program.
fn set_flag(cell: &mut Cell, value: bool) {
    program_mut(cell).registers.flag = value;
}

/// Returns the program stored in a cell, asserting that one exists.
fn program(cell: &Cell) -> &Program {
    cell.program
        .as_ref()
        .expect("cell should contain a program for pass 2")
}

/// Returns the mutable program stored in a cell, asserting that one exists.
fn program_mut(cell: &mut Cell) -> &mut Program {
    cell.program
        .as_mut()
        .expect("cell should contain a program for pass 2")
}

#[cfg(test)]
mod tests {
    use super::{choose_winner, ExclusiveCandidate};
    use crate::model::QueuedAction;

    #[test]
    fn choose_winner_prefers_higher_strength_before_weighted_ties() {
        let candidates = vec![
            ExclusiveCandidate {
                action: QueuedAction::Move {
                    source: 1,
                    target: 9,
                },
                source: 1,
                target: 9,
                strength: 2,
                size: 4,
                target_strength: 0,
                target_has_program: false,
            },
            ExclusiveCandidate {
                action: QueuedAction::Move {
                    source: 3,
                    target: 9,
                },
                source: 3,
                target: 9,
                strength: 5,
                size: 1,
                target_strength: 0,
                target_has_program: false,
            },
        ];

        assert_eq!(choose_winner(&candidates, 9, 0, 0), 1);
    }
}
