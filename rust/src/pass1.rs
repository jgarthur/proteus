//! Executes Pass 1, where live programs spend their local action budget.

use crate::config::{SimConfig, PROGRAM_SIZE_CAP};
use crate::grid::Grid;
use crate::model::{Cell, CellSnapshot, Direction, Packet, QueuedAction};
use crate::opcode::{Locality, Opcode};
use crate::random::{cell_rng, WyRand};

/// Collects the nonlocal actions and emitted packets produced by Pass 1.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Pass1Output {
    pub actions: Vec<QueuedAction>,
    pub emitted_packets: Vec<Packet>,
}

/// Computes the local action budget for one program size and exponent.
pub fn local_action_budget(size_at_tick_start: u16, alpha: f64) -> u32 {
    let size = f64::from(size_at_tick_start);
    let budget = size.powf(alpha).floor() as u32;
    budget.max(1)
}

/// Runs the local VM for every program that was live at tick start.
pub fn pass1_local(
    grid: &mut Grid,
    snapshot: &[CellSnapshot],
    live_set: &[bool],
    config: &SimConfig,
    tick: u64,
    seed: u64,
) -> Pass1Output {
    assert_eq!(
        snapshot.len(),
        grid.len(),
        "snapshot length must match grid size"
    );
    assert_eq!(
        live_set.len(),
        grid.len(),
        "live-set length must match grid size"
    );

    for cell in grid.cells_mut() {
        if let Some(program) = cell.program.as_mut() {
            program.tick.reset_for_pass1(program.is_inert());
        }
    }

    let width = grid.width();
    let height = grid.height();
    let mut output = Pass1Output::default();

    for index in 0..grid.len() {
        if !live_set[index] {
            continue;
        }

        let mut remaining_actions =
            local_action_budget(snapshot[index].program_size, config.local_action_exponent);
        let mut rng = cell_rng(seed, tick, index as u64);

        while remaining_actions > 0 {
            let opcode = {
                let cell = grid.get(index).expect("cell should exist during pass 1");
                let program = cell
                    .program
                    .as_ref()
                    .expect("live-set cells should contain a program");
                let ip_index = current_ip_index(cell);
                Opcode::decode(program.code[ip_index])
            };

            match opcode.locality() {
                Locality::Local => {
                    let result = execute_local_instruction(
                        grid.get_mut(index)
                            .expect("cell should exist during pass 1"),
                        index,
                        opcode,
                        snapshot,
                        width,
                        height,
                        config,
                        &mut rng,
                    );

                    match result {
                        LocalExecResult::Continue { packet } => {
                            if let Some(packet) = packet {
                                output.emitted_packets.push(packet);
                            }
                            remaining_actions -= 1;
                        }
                        LocalExecResult::HaltForTick => break,
                    }
                }
                Locality::Nonlocal => {
                    let result = attempt_nonlocal_queue(
                        grid.get_mut(index)
                            .expect("cell should exist during pass 1"),
                        index,
                        opcode,
                        width,
                        height,
                    );

                    match result {
                        NonlocalExecResult::HaltForTickNoAdvance => break,
                        NonlocalExecResult::StopForTickAdvance { action } => {
                            if let Some(action) = action {
                                output.actions.push(action);
                            }
                            break;
                        }
                    }
                }
            }
        }
    }

    output
}

/// Describes how a local opcode changes Pass 1 control flow.
enum LocalExecResult {
    Continue { packet: Option<Packet> },
    HaltForTick,
}

/// Describes how a nonlocal opcode queue attempt ends Pass 1 execution.
enum NonlocalExecResult {
    HaltForTickNoAdvance,
    StopForTickAdvance { action: Option<QueuedAction> },
}

/// Describes how an opcode should update the program flag.
#[derive(Clone, Copy)]
enum FlagEffect {
    Clear,
    Set,
    Neutral,
}

/// Executes one local opcode against a single cell.
#[allow(clippy::too_many_arguments)]
fn execute_local_instruction(
    cell: &mut Cell,
    cell_index: usize,
    opcode: Opcode,
    snapshot: &[CellSnapshot],
    width: u32,
    height: u32,
    config: &SimConfig,
    rng: &mut WyRand,
) -> LocalExecResult {
    let base_cost = opcode.base_cost();
    let bg_used = match pay_base_cost(cell, base_cost) {
        Ok(bg_used) => bg_used,
        Err(()) => {
            payment_failure(cell);
            return LocalExecResult::HaltForTick;
        }
    };

    program_mut(cell).tick.bg_radiation_consumed += bg_used;

    let current_ip = current_ip_index(cell);

    match opcode {
        Opcode::PushLiteral(value) => {
            let pushed = push_stack(cell, value);
            if pushed {
                apply_flag(cell, FlagEffect::Clear);
            }
            advance_ip(cell, next_ip(current_ip, program(cell).code.len()));
        }
        Opcode::Dup => {
            let value = {
                let program = program(cell);
                program.stack.last().copied()
            };
            if let Some(value) = value {
                if push_stack(cell, value) {
                    apply_flag(cell, FlagEffect::Clear);
                }
            } else {
                apply_flag(cell, FlagEffect::Set);
            }
            advance_ip(cell, next_ip(current_ip, program(cell).code.len()));
        }
        Opcode::Drop => {
            if pop_stack(cell).is_some() {
                apply_flag(cell, FlagEffect::Clear);
            }
            advance_ip(cell, next_ip(current_ip, program(cell).code.len()));
        }
        Opcode::Swap => {
            let swapped = {
                let program = program_mut(cell);
                if program.stack.len() < 2 {
                    false
                } else {
                    let len = program.stack.len();
                    program.stack.swap(len - 1, len - 2);
                    true
                }
            };
            apply_flag(
                cell,
                if swapped {
                    FlagEffect::Clear
                } else {
                    FlagEffect::Set
                },
            );
            advance_ip(cell, next_ip(current_ip, program(cell).code.len()));
        }
        Opcode::Over => {
            let value = {
                let program = program(cell);
                if program.stack.len() < 2 {
                    None
                } else {
                    Some(program.stack[program.stack.len() - 2])
                }
            };
            if let Some(value) = value {
                if push_stack(cell, value) {
                    apply_flag(cell, FlagEffect::Clear);
                }
            } else {
                apply_flag(cell, FlagEffect::Set);
            }
            advance_ip(cell, next_ip(current_ip, program(cell).code.len()));
        }
        Opcode::Rand => {
            if push_stack(cell, (rng.next_u32() & 0xff) as i16) {
                apply_flag(cell, FlagEffect::Clear);
            }
            advance_ip(cell, next_ip(current_ip, program(cell).code.len()));
        }
        Opcode::Add => {
            execute_binary_op(cell, current_ip, |second, top| second.wrapping_add(top));
        }
        Opcode::Sub => {
            execute_binary_op(cell, current_ip, |second, top| second.wrapping_sub(top));
        }
        Opcode::Neg => {
            execute_unary_op(cell, current_ip, |top| top.wrapping_neg());
        }
        Opcode::Eq => {
            execute_binary_op(cell, current_ip, |second, top| i16::from(second == top));
        }
        Opcode::Lt => {
            execute_binary_op(cell, current_ip, |second, top| i16::from(second < top));
        }
        Opcode::Gt => {
            execute_binary_op(cell, current_ip, |second, top| i16::from(second > top));
        }
        Opcode::Not => {
            execute_unary_op(cell, current_ip, |top| i16::from(top == 0));
        }
        Opcode::And => {
            execute_binary_op(cell, current_ip, |second, top| {
                i16::from(second != 0 && top != 0)
            });
        }
        Opcode::Or => {
            execute_binary_op(cell, current_ip, |second, top| {
                i16::from(second != 0 || top != 0)
            });
        }
        Opcode::For => {
            let popped = pop_stack(cell);
            if let Some(count) = popped {
                program_mut(cell).registers.lc = count;
                if count <= 0 {
                    if let Some(next_index) = find_forward_opcode(cell, current_ip, Opcode::Next) {
                        apply_flag(cell, FlagEffect::Neutral);
                        advance_ip(cell, next_ip(next_index, program(cell).code.len()));
                    } else {
                        apply_flag(cell, FlagEffect::Set);
                        advance_ip(cell, next_ip(current_ip, program(cell).code.len()));
                    }
                } else {
                    apply_flag(cell, FlagEffect::Clear);
                    advance_ip(cell, next_ip(current_ip, program(cell).code.len()));
                }
            } else {
                advance_ip(cell, next_ip(current_ip, program(cell).code.len()));
            }
        }
        Opcode::Next => {
            if let Some(for_index) = find_backward_opcode(cell, current_ip, Opcode::For) {
                let lc = {
                    let program = program_mut(cell);
                    program.registers.lc = program.registers.lc.wrapping_sub(1);
                    program.registers.lc
                };
                if lc > 0 {
                    apply_flag(cell, FlagEffect::Clear);
                    advance_ip(cell, next_ip(for_index, program(cell).code.len()));
                } else {
                    apply_flag(cell, FlagEffect::Clear);
                    advance_ip(cell, next_ip(current_ip, program(cell).code.len()));
                }
            } else {
                apply_flag(cell, FlagEffect::Neutral);
                advance_ip(cell, next_ip(current_ip, program(cell).code.len()));
            }
        }
        Opcode::Jmp => {
            if let Some(offset) = pop_stack(cell) {
                advance_ip(cell, jump_ip(current_ip, offset, program(cell).code.len()));
                apply_flag(cell, FlagEffect::Clear);
            } else {
                advance_ip(cell, next_ip(current_ip, program(cell).code.len()));
            }
        }
        Opcode::JmpNz => {
            let value = pop_stack(cell);
            let offset = pop_stack(cell);
            match (value, offset) {
                (Some(value), Some(offset)) => {
                    if value != 0 {
                        advance_ip(cell, jump_ip(current_ip, offset, program(cell).code.len()));
                    } else {
                        advance_ip(cell, next_ip(current_ip, program(cell).code.len()));
                    }
                    apply_flag(cell, FlagEffect::Clear);
                }
                _ => advance_ip(cell, next_ip(current_ip, program(cell).code.len())),
            }
        }
        Opcode::JmpZ => {
            let value = pop_stack(cell);
            let offset = pop_stack(cell);
            match (value, offset) {
                (Some(value), Some(offset)) => {
                    if value == 0 {
                        advance_ip(cell, jump_ip(current_ip, offset, program(cell).code.len()));
                    } else {
                        advance_ip(cell, next_ip(current_ip, program(cell).code.len()));
                    }
                    apply_flag(cell, FlagEffect::Clear);
                }
                _ => advance_ip(cell, next_ip(current_ip, program(cell).code.len())),
            }
        }
        Opcode::Cw => {
            program_mut(cell).registers.dir = program(cell).registers.dir.clockwise();
            apply_flag(cell, FlagEffect::Clear);
            advance_ip(cell, next_ip(current_ip, program(cell).code.len()));
        }
        Opcode::Ccw => {
            program_mut(cell).registers.dir = program(cell).registers.dir.counterclockwise();
            apply_flag(cell, FlagEffect::Clear);
            advance_ip(cell, next_ip(current_ip, program(cell).code.len()));
        }
        Opcode::GetSize => {
            if push_stack(cell, program(cell).size() as i16) {
                apply_flag(cell, FlagEffect::Clear);
            }
            advance_ip(cell, next_ip(current_ip, program(cell).code.len()));
        }
        Opcode::GetIp => {
            if push_stack(cell, program(cell).registers.ip as i16) {
                apply_flag(cell, FlagEffect::Clear);
            }
            advance_ip(cell, next_ip(current_ip, program(cell).code.len()));
        }
        Opcode::GetFlag => {
            if push_stack(cell, i16::from(program(cell).registers.flag)) {
                apply_flag(cell, FlagEffect::Clear);
            }
            advance_ip(cell, next_ip(current_ip, program(cell).code.len()));
        }
        Opcode::GetMsg => {
            if push_stack(cell, program(cell).registers.msg) {
                apply_flag(cell, FlagEffect::Clear);
            }
            advance_ip(cell, next_ip(current_ip, program(cell).code.len()));
        }
        Opcode::GetId => {
            if push_stack(cell, i16::from(program(cell).registers.id)) {
                apply_flag(cell, FlagEffect::Clear);
            }
            advance_ip(cell, next_ip(current_ip, program(cell).code.len()));
        }
        Opcode::GetSrc => {
            if push_stack(cell, program(cell).registers.src as i16) {
                apply_flag(cell, FlagEffect::Clear);
            }
            advance_ip(cell, next_ip(current_ip, program(cell).code.len()));
        }
        Opcode::GetDst => {
            if push_stack(cell, program(cell).registers.dst as i16) {
                apply_flag(cell, FlagEffect::Clear);
            }
            advance_ip(cell, next_ip(current_ip, program(cell).code.len()));
        }
        Opcode::SetDir => {
            if let Some(value) = pop_stack(cell) {
                program_mut(cell).registers.dir = Direction::from_i16(value);
                apply_flag(cell, FlagEffect::Clear);
            }
            advance_ip(cell, next_ip(current_ip, program(cell).code.len()));
        }
        Opcode::SetSrc => {
            if let Some(value) = pop_stack(cell) {
                program_mut(cell).registers.src = value as u16;
                apply_flag(cell, FlagEffect::Clear);
            }
            advance_ip(cell, next_ip(current_ip, program(cell).code.len()));
        }
        Opcode::SetDst => {
            if let Some(value) = pop_stack(cell) {
                program_mut(cell).registers.dst = value as u16;
                apply_flag(cell, FlagEffect::Clear);
            }
            advance_ip(cell, next_ip(current_ip, program(cell).code.len()));
        }
        Opcode::SetId => {
            if let Some(value) = pop_stack(cell) {
                program_mut(cell).registers.id = value as u8;
                apply_flag(cell, FlagEffect::Clear);
            }
            advance_ip(cell, next_ip(current_ip, program(cell).code.len()));
        }
        Opcode::GetE => {
            if push_stack(cell, cell.free_energy as i16) {
                apply_flag(cell, FlagEffect::Clear);
            }
            advance_ip(cell, next_ip(current_ip, program(cell).code.len()));
        }
        Opcode::GetM => {
            if push_stack(cell, cell.free_mass as i16) {
                apply_flag(cell, FlagEffect::Clear);
            }
            advance_ip(cell, next_ip(current_ip, program(cell).code.len()));
        }
        Opcode::Nop => {
            {
                let program = program_mut(cell);
                program.tick.did_nop = true;
                program.tick.is_open = true;
            }
            apply_flag(cell, FlagEffect::Clear);
            advance_ip(cell, next_ip(current_ip, program(cell).code.len()));
        }
        Opcode::Absorb => {
            let dir = program(cell).registers.dir;
            {
                let program = program_mut(cell);
                if program.tick.absorb_count == 0 {
                    program.tick.absorb_count = 1;
                    program.tick.absorb_dir = Some(dir);
                } else if program.tick.absorb_count < 4 {
                    program.tick.absorb_count += 1;
                }
            }
            apply_flag(cell, FlagEffect::Clear);
            advance_ip(cell, next_ip(current_ip, program(cell).code.len()));
        }
        Opcode::Listen => {
            {
                let program = program_mut(cell);
                program.tick.did_listen = true;
                program.tick.is_open = true;
            }
            apply_flag(cell, FlagEffect::Neutral);
            advance_ip(cell, next_ip(current_ip, program(cell).code.len()));
        }
        Opcode::Collect => {
            program_mut(cell).tick.did_collect = true;
            apply_flag(cell, FlagEffect::Clear);
            advance_ip(cell, next_ip(current_ip, program(cell).code.len()));
        }
        Opcode::Emit => {
            let packet = if let Some(message) = pop_stack(cell) {
                let direction = program(cell).registers.dir;
                apply_flag(cell, FlagEffect::Clear);
                Some(Packet {
                    position: cell_index,
                    direction,
                    message,
                })
            } else {
                None
            };
            advance_ip(cell, next_ip(current_ip, program(cell).code.len()));
            return LocalExecResult::Continue { packet };
        }
        Opcode::Read => {
            let src_index = usize::from(program(cell).registers.src % program(cell).size());
            let value = i16::from(program(cell).code[src_index]);
            let pushed = push_stack(cell, value);
            if pushed {
                let program = program_mut(cell);
                program.registers.src = program.registers.src.wrapping_add(1);
                apply_flag(cell, FlagEffect::Clear);
            }
            advance_ip(cell, next_ip(current_ip, program(cell).code.len()));
        }
        Opcode::Write => {
            if let Some(value) = pop_stack(cell) {
                let write_index = usize::from(program(cell).registers.dst % program(cell).size());
                let program = program_mut(cell);
                program.code[write_index] = value as u8;
                program.registers.dst = program.registers.dst.wrapping_add(1);
                apply_flag(cell, FlagEffect::Clear);
            }
            advance_ip(cell, next_ip(current_ip, program(cell).code.len()));
        }
        Opcode::Del => {
            if program(cell).code.len() == 1 {
                apply_flag(cell, FlagEffect::Set);
                advance_ip(cell, next_ip(current_ip, program(cell).code.len()));
            } else {
                let delete_index = usize::from(program(cell).registers.dst % program(cell).size());
                let new_len = {
                    let program = program_mut(cell);
                    program.code.remove(delete_index);
                    program.code.len()
                };
                cell.free_mass += 1;
                apply_flag(cell, FlagEffect::Clear);
                let adjusted_current = if delete_index <= current_ip {
                    current_ip
                } else {
                    current_ip + 1
                };
                advance_ip(cell, wrap_ip(adjusted_current % new_len, new_len));
            }
        }
        Opcode::Synthesize => {
            if cell.free_energy < config.n_synth {
                apply_flag(cell, FlagEffect::Set);
                program_mut(cell).tick.is_open = true;
                return LocalExecResult::HaltForTick;
            }
            cell.free_energy -= config.n_synth;
            cell.free_mass += 1;
            apply_flag(cell, FlagEffect::Clear);
            advance_ip(cell, next_ip(current_ip, program(cell).code.len()));
        }
        Opcode::SenseSize => {
            let sensed = snapshot
                [neighbor_index(width, height, cell_index, program(cell).registers.dir)]
            .program_size as i16;
            if push_stack(cell, sensed) {
                apply_flag(cell, FlagEffect::Clear);
            }
            advance_ip(cell, next_ip(current_ip, program(cell).code.len()));
        }
        Opcode::SenseE => {
            let sensed = snapshot
                [neighbor_index(width, height, cell_index, program(cell).registers.dir)]
            .free_energy as i16;
            if push_stack(cell, sensed) {
                apply_flag(cell, FlagEffect::Clear);
            }
            advance_ip(cell, next_ip(current_ip, program(cell).code.len()));
        }
        Opcode::SenseM => {
            let sensed = snapshot
                [neighbor_index(width, height, cell_index, program(cell).registers.dir)]
            .free_mass as i16;
            if push_stack(cell, sensed) {
                apply_flag(cell, FlagEffect::Clear);
            }
            advance_ip(cell, next_ip(current_ip, program(cell).code.len()));
        }
        Opcode::SenseId => {
            let sensed =
                snapshot[neighbor_index(width, height, cell_index, program(cell).registers.dir)];
            let pushed = push_stack(cell, i16::from(sensed.program_id));
            if pushed {
                apply_flag(
                    cell,
                    if sensed.has_program {
                        FlagEffect::Clear
                    } else {
                        FlagEffect::Set
                    },
                );
            }
            advance_ip(cell, next_ip(current_ip, program(cell).code.len()));
        }
        Opcode::NoOp(_) => {
            apply_flag(cell, FlagEffect::Neutral);
            advance_ip(cell, next_ip(current_ip, program(cell).code.len()));
        }
        opcode => panic!("unexpected opcode in local execution: {opcode:?}"),
    }

    LocalExecResult::Continue { packet: None }
}

/// Attempts to queue one nonlocal action from the current program.
fn attempt_nonlocal_queue(
    cell: &mut Cell,
    cell_index: usize,
    opcode: Opcode,
    width: u32,
    height: u32,
) -> NonlocalExecResult {
    let bg_used = match pay_base_cost(cell, opcode.base_cost()) {
        Ok(bg_used) => bg_used,
        Err(()) => {
            payment_failure(cell);
            return NonlocalExecResult::HaltForTickNoAdvance;
        }
    };

    program_mut(cell).tick.bg_radiation_consumed += bg_used;

    let current_ip = current_ip_index(cell);
    let next_ip = next_ip(current_ip, program(cell).code.len());
    let target = neighbor_index(width, height, cell_index, program(cell).registers.dir);

    let action = match opcode {
        Opcode::ReadAdj => Some(QueuedAction::ReadAdj {
            source: cell_index,
            target,
            src_cursor: program(cell).registers.src,
        }),
        Opcode::WriteAdj => capture_single_operand(cell).map(|value| QueuedAction::WriteAdj {
            source: cell_index,
            target,
            value: value as u8,
            dst_cursor: program(cell).registers.dst,
        }),
        Opcode::AppendAdj => capture_single_operand(cell).map(|value| QueuedAction::AppendAdj {
            source: cell_index,
            target,
            value: value as u8,
        }),
        Opcode::DelAdj => Some(QueuedAction::DelAdj {
            source: cell_index,
            target,
            dst_cursor: program(cell).registers.dst,
        }),
        Opcode::GiveE => capture_single_operand(cell).map(|amount| QueuedAction::GiveE {
            source: cell_index,
            target,
            amount,
        }),
        Opcode::GiveM => capture_single_operand(cell).map(|amount| QueuedAction::GiveM {
            source: cell_index,
            target,
            amount,
        }),
        Opcode::Move => Some(QueuedAction::Move {
            source: cell_index,
            target,
        }),
        Opcode::Boot => Some(QueuedAction::Boot {
            source: cell_index,
            target,
        }),
        other => panic!("unexpected opcode in nonlocal queue attempt: {other:?}"),
    };

    if action.is_none() {
        apply_flag(cell, FlagEffect::Set);
    }

    advance_ip(cell, next_ip);
    NonlocalExecResult::StopForTickAdvance { action }
}

/// Pops two operands, applies a binary operator, and pushes the result.
fn execute_binary_op<F>(cell: &mut Cell, current_ip: usize, op: F)
where
    F: FnOnce(i16, i16) -> i16,
{
    let top = pop_stack(cell);
    let second = pop_stack(cell);

    if let (Some(top), Some(second)) = (top, second) {
        let pushed = push_stack(cell, op(second, top));
        if pushed {
            apply_flag(cell, FlagEffect::Clear);
        }
    }

    advance_ip(cell, next_ip(current_ip, program(cell).code.len()));
}

/// Pops one operand, applies a unary operator, and pushes the result.
fn execute_unary_op<F>(cell: &mut Cell, current_ip: usize, op: F)
where
    F: FnOnce(i16) -> i16,
{
    if let Some(top) = pop_stack(cell) {
        let pushed = push_stack(cell, op(top));
        if pushed {
            apply_flag(cell, FlagEffect::Clear);
        }
    }

    advance_ip(cell, next_ip(current_ip, program(cell).code.len()));
}

/// Pays an opcode's base energy cost, falling back to background radiation.
fn pay_base_cost(cell: &mut Cell, cost: u32) -> Result<u32, ()> {
    if cost == 0 {
        return Ok(0);
    }

    let free_used = cell.free_energy.min(cost);
    let remainder = cost - free_used;
    if cell.bg_radiation < remainder {
        return Err(());
    }

    cell.free_energy -= free_used;
    cell.bg_radiation -= remainder;
    Ok(remainder)
}

/// Applies the spec-defined state updates for a failed base-cost payment.
fn payment_failure(cell: &mut Cell) {
    apply_flag(cell, FlagEffect::Set);
    program_mut(cell).tick.is_open = true;
}

/// Pops the single operand required by a nonlocal instruction.
fn capture_single_operand(cell: &mut Cell) -> Option<i16> {
    if program(cell).stack.is_empty() {
        None
    } else {
        Some(
            program_mut(cell)
                .stack
                .pop()
                .expect("checked non-empty stack"),
        )
    }
}

/// Pops one value from the program stack if present.
fn pop_stack(cell: &mut Cell) -> Option<i16> {
    let popped = program_mut(cell).stack.pop();
    if popped.is_none() {
        apply_flag(cell, FlagEffect::Set);
    }
    popped
}

/// Pushes one value onto the program stack when capacity allows.
fn push_stack(cell: &mut Cell, value: i16) -> bool {
    if program(cell).stack.len() >= usize::from(PROGRAM_SIZE_CAP) {
        apply_flag(cell, FlagEffect::Set);
        false
    } else {
        program_mut(cell).stack.push(value);
        true
    }
}

/// Applies a flag update after an opcode executes.
fn apply_flag(cell: &mut Cell, effect: FlagEffect) {
    match effect {
        FlagEffect::Clear => program_mut(cell).registers.flag = false,
        FlagEffect::Set => program_mut(cell).registers.flag = true,
        FlagEffect::Neutral => {}
    }
}

/// Stores the next instruction pointer value back into the program registers.
fn advance_ip(cell: &mut Cell, next_ip: u16) {
    program_mut(cell).registers.ip = next_ip;
}

/// Returns the current instruction pointer as a valid code index.
fn current_ip_index(cell: &Cell) -> usize {
    let program = program(cell);
    usize::from(program.registers.ip % program.size())
}

/// Returns the next sequential instruction pointer with wraparound.
fn next_ip(current_ip: usize, size: usize) -> u16 {
    wrap_ip((current_ip + 1) % size, size)
}

/// Returns the wrapped instruction pointer reached by a relative jump.
fn jump_ip(current_ip: usize, offset: i16, size: usize) -> u16 {
    let base = current_ip as i32;
    let offset = i32::from(offset);
    let size = size as i32;
    let target = (base + offset).rem_euclid(size) as usize;
    wrap_ip(target, size as usize)
}

/// Wraps an arbitrary instruction index into the program code length.
fn wrap_ip(index: usize, size: usize) -> u16 {
    u16::try_from(index % size).expect("program size is capped to u16")
}

/// Scans forward for the next matching opcode, wrapping around the program.
fn find_forward_opcode(cell: &Cell, current_ip: usize, needle: Opcode) -> Option<usize> {
    let code = &program(cell).code;
    for step in 1..code.len() {
        let index = (current_ip + step) % code.len();
        if Opcode::decode(code[index]) == needle {
            return Some(index);
        }
    }
    None
}

/// Scans backward for the previous matching opcode, wrapping around the program.
fn find_backward_opcode(cell: &Cell, current_ip: usize, needle: Opcode) -> Option<usize> {
    let code = &program(cell).code;
    for step in 1..code.len() {
        let index = (current_ip + code.len() - step) % code.len();
        if Opcode::decode(code[index]) == needle {
            return Some(index);
        }
    }
    None
}

/// Computes a neighbor index from raw grid dimensions and a direction.
fn neighbor_index(width: u32, height: u32, index: usize, dir: Direction) -> usize {
    let width_usize = usize::try_from(width).expect("width should fit in usize");
    let height_usize = usize::try_from(height).expect("height should fit in usize");
    let x = index % width_usize;
    let y = index / width_usize;

    match dir {
        Direction::Right => (y * width_usize) + ((x + 1) % width_usize),
        Direction::Up => (((y + height_usize - 1) % height_usize) * width_usize) + x,
        Direction::Left => (y * width_usize) + ((x + width_usize - 1) % width_usize),
        Direction::Down => (((y + 1) % height_usize) * width_usize) + x,
    }
}

/// Returns the program stored in a cell, asserting that one exists.
fn program(cell: &Cell) -> &crate::model::Program {
    cell.program
        .as_ref()
        .expect("cell should contain a program")
}

/// Returns the mutable program stored in a cell, asserting that one exists.
fn program_mut(cell: &mut Cell) -> &mut crate::model::Program {
    cell.program
        .as_mut()
        .expect("cell should contain a program")
}

#[cfg(test)]
mod tests {
    use crate::pass1::local_action_budget;

    #[test]
    fn local_action_budget_respects_minimum_of_one() {
        assert_eq!(local_action_budget(0, 1.0), 1);
        assert_eq!(local_action_budget(1, 1.0), 1);
        assert_eq!(local_action_budget(4, 0.5), 2);
    }
}
