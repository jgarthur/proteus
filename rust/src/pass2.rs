use crate::config::PROGRAM_SIZE_CAP;
use crate::grid::Grid;
use crate::model::{Cell, Direction, Program, QueuedAction};
use crate::random::cell_rng;

const EXCLUSIVE_TIE_SALT: u64 = 0x8d51_4c2f_d5b3_7a11;
const APPEND_CREATE_SALT: u64 = 0xa4e2_9c61_7f33_b58d;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pass2Output {
    pub incoming_writes: Vec<bool>,
    pub booted_programs: u32,
}

impl Pass2Output {
    pub fn new(cell_count: usize) -> Self {
        Self {
            incoming_writes: vec![false; cell_count],
            booted_programs: 0,
        }
    }
}

pub fn pass2_nonlocal(
    grid: &mut Grid,
    actions: &[QueuedAction],
    tick: u64,
    seed: u64,
) -> Pass2Output {
    let pre_pass2 = grid.clone();
    let mut output = Pass2Output::new(grid.len());

    resolve_reads(grid, &pre_pass2, actions);
    resolve_additive_transfers(grid, &pre_pass2, actions);
    resolve_exclusive(grid, &pre_pass2, actions, tick, seed, &mut output);

    output
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ExclusiveCandidate {
    action: QueuedAction,
    source: usize,
    target: usize,
    strength: u32,
    size: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct MoveCommit {
    source: usize,
    target: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct AppendCreateCommit {
    target: usize,
    value: u8,
}

fn resolve_reads(grid: &mut Grid, pre_pass2: &Grid, actions: &[QueuedAction]) {
    for action in actions {
        let QueuedAction::ReadAdj {
            source,
            target,
            src_cursor,
        } = *action
        else {
            continue;
        };

        let target_cell = pre_pass2.get(target).expect("target cell should exist");
        let source_cell = grid.get_mut(source).expect("source cell should exist");

        if let Some(target_program) = target_cell.program.as_ref() {
            let read_index = usize::from(src_cursor % target_program.size());
            let pushed = push_stack(source_cell, i16::from(target_program.code[read_index]));
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

fn resolve_additive_transfers(grid: &mut Grid, pre_pass2: &Grid, actions: &[QueuedAction]) {
    let mut energy_in = vec![0_u32; grid.len()];
    let mut energy_out = vec![0_u32; grid.len()];
    let mut mass_in = vec![0_u32; grid.len()];
    let mut mass_out = vec![0_u32; grid.len()];

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
                    pre_pass2
                        .get(source)
                        .expect("source cell should exist")
                        .free_energy,
                );
                energy_out[source] += transferable;
                energy_in[target] += transferable;
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
                    pre_pass2
                        .get(source)
                        .expect("source cell should exist")
                        .free_mass,
                );
                mass_out[source] += transferable;
                mass_in[target] += transferable;
                set_flag(
                    grid.get_mut(source).expect("source cell should exist"),
                    false,
                );
            }
            _ => {}
        }
    }

    for index in 0..grid.len() {
        let cell = grid.get_mut(index).expect("cell should exist");
        cell.free_energy = cell.free_energy - energy_out[index] + energy_in[index];
        cell.free_mass = cell.free_mass - mass_out[index] + mass_in[index];
    }
}

fn resolve_exclusive(
    grid: &mut Grid,
    pre_pass2: &Grid,
    actions: &[QueuedAction],
    tick: u64,
    seed: u64,
    output: &mut Pass2Output,
) {
    let exclusive_base = grid.clone();
    let mut working = exclusive_base.clone();
    let mut candidates = Vec::new();
    let mut by_target = vec![Vec::<usize>::new(); grid.len()];
    let mut moves = Vec::new();
    let mut creates = Vec::new();

    for action in actions {
        let Some((source, target)) = exclusive_endpoints(*action) else {
            continue;
        };

        if let Some(candidate) = validate_exclusive(*action, pre_pass2) {
            by_target[target].push(candidates.len());
            candidates.push(candidate);
        } else {
            set_flag(
                working.get_mut(source).expect("source cell should exist"),
                true,
            );
        }
    }

    for (target, group) in by_target.iter().enumerate() {
        if group.is_empty() {
            continue;
        }

        if group
            .iter()
            .all(|candidate| matches!(candidates[*candidate].action, QueuedAction::Boot { .. }))
        {
            for candidate_index in group {
                let candidate = candidates[*candidate_index];
                set_flag(
                    working
                        .get_mut(candidate.source)
                        .expect("source cell should exist"),
                    false,
                );
            }
            apply_boot_success(working.get_mut(target).expect("target cell should exist"));
            output.booted_programs += 1;
            continue;
        }

        let winner_index = choose_winner(&candidates, group, target, tick, seed);
        for candidate_index in group {
            if *candidate_index == winner_index {
                continue;
            }

            let candidate = candidates[*candidate_index];
            set_flag(
                working
                    .get_mut(candidate.source)
                    .expect("source cell should exist"),
                true,
            );
        }

        apply_winner(
            &mut working,
            pre_pass2,
            candidates[winner_index],
            &mut moves,
            &mut creates,
            output,
        );
    }

    let mut final_grid = working;
    for commit in moves {
        apply_move_commit(&mut final_grid, &exclusive_base, commit);
    }
    for commit in creates {
        apply_append_create_commit(&mut final_grid, commit, tick, seed);
    }

    *grid = final_grid;
}

fn validate_exclusive(action: QueuedAction, pre_pass2: &Grid) -> Option<ExclusiveCandidate> {
    let (source, target) = exclusive_endpoints(action)?;
    let source_cell = pre_pass2.get(source).expect("source cell should exist");
    let source_program = source_cell.program.as_ref()?;
    let target_cell = pre_pass2.get(target).expect("target cell should exist");

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
    })
}

fn choose_winner(
    candidates: &[ExclusiveCandidate],
    group: &[usize],
    target: usize,
    tick: u64,
    seed: u64,
) -> usize {
    let best_strength = group
        .iter()
        .map(|candidate| candidates[*candidate].strength)
        .max()
        .expect("group should contain at least one candidate");

    let mut tied = group
        .iter()
        .copied()
        .filter(|candidate| candidates[*candidate].strength == best_strength)
        .collect::<Vec<_>>();
    tied.sort_by_key(|candidate| candidates[*candidate].source);

    if tied.len() == 1 {
        return tied[0];
    }

    let total_weight = tied
        .iter()
        .map(|candidate| u64::from(candidates[*candidate].size))
        .sum::<u64>();
    let mut rng = cell_rng(seed ^ EXCLUSIVE_TIE_SALT, tick, target as u64);
    let roll = rng.next_u64() % total_weight;

    let mut cumulative = 0_u64;
    for candidate in tied {
        cumulative += u64::from(candidates[candidate].size);
        if roll < cumulative {
            return candidate;
        }
    }

    unreachable!("weighted selection should return within the tied set")
}

#[allow(clippy::too_many_arguments)]
fn apply_winner(
    working: &mut Grid,
    pre_pass2: &Grid,
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

            if pre_pass2
                .get(candidate.target)
                .expect("target cell should exist")
                .has_program()
            {
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
            let target_cell = pre_pass2
                .get(candidate.target)
                .expect("target cell should exist");
            let target_program = target_cell
                .program
                .as_ref()
                .expect("validated delAdj target should exist");
            let strength = u32::from(target_program.size()).min(target_cell.free_energy);

            if source_cell.free_energy < strength {
                set_flag(source_cell, true);
                return;
            }

            source_cell.free_energy -= strength;
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

fn apply_boot_success(target_cell: &mut Cell) {
    let target_program = program_mut(target_cell);
    target_program.live = true;
    target_program.age = 0;
    target_program.registers.ip = 0;
    target_program.tick.is_newborn = true;
    target_program.tick.is_open = false;
}

fn apply_move_commit(grid: &mut Grid, exclusive_base: &Grid, commit: MoveCommit) {
    let source_base = exclusive_base
        .get(commit.source)
        .expect("source cell should exist");
    let source_working = grid
        .get(commit.source)
        .expect("source cell should exist")
        .clone();
    let target_cell = grid
        .get_mut(commit.target)
        .expect("target cell should exist");

    target_cell.program = source_working.program;
    target_cell.free_energy += source_working.free_energy;
    target_cell.free_mass += source_working.free_mass;

    let source_cell = grid
        .get_mut(commit.source)
        .expect("source cell should exist");
    source_cell.program = None;
    source_cell.free_energy = 0;
    source_cell.free_mass = 0;
    source_cell.bg_radiation = source_base.bg_radiation;
    source_cell.bg_mass = source_base.bg_mass;
}

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

fn target_is_open(cell: &Cell) -> bool {
    cell.program
        .as_ref()
        .is_none_or(|program| program.tick.is_open)
}

fn push_stack(cell: &mut Cell, value: i16) -> bool {
    if program(cell).stack.len() >= usize::from(PROGRAM_SIZE_CAP) {
        set_flag(cell, true);
        false
    } else {
        program_mut(cell).stack.push(value);
        true
    }
}

fn set_flag(cell: &mut Cell, value: bool) {
    program_mut(cell).registers.flag = value;
}

fn program(cell: &Cell) -> &Program {
    cell.program
        .as_ref()
        .expect("cell should contain a program for pass 2")
}

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
            },
        ];

        assert_eq!(choose_winner(&candidates, &[0, 1], 9, 0, 0), 1);
    }
}
