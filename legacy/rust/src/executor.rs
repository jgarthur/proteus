use crate::cell::{Cell, CostPayment};
use crate::cpu::CPUError;
use crate::instruction::Instruction;
use crate::types::Coord;
use crate::world::WorldParams;

pub enum ExecutionResult {
    Complete,
    Immediate,
    NonLocal {
        target: Coord,
        instruction: Instruction,
    },
    Error(ExecutionError),
    NoInstruction,
}

#[derive(Debug)]
pub enum ExecutionError {
    CPU(CPUError),
    NoEnergy,
    Halted,
}

impl From<CPUError> for ExecutionError {
    fn from(err: CPUError) -> Self {
        ExecutionError::CPU(err)
    }
}

/// Execute for 1 tick
pub fn run_tick_local(cell: &mut Cell, coord: Coord, params: &WorldParams) -> ExecutionResult {
    let mut immediate_count = 0;

    loop {
        let result = try_execute_local(cell, coord, params);
        match result {
            ExecutionResult::Immediate => {
                immediate_count += 1;
                if immediate_count == cell.program_size() as usize {
                    cell.is_vulnerable = true;
                    return ExecutionResult::Error(ExecutionError::Halted);
                }
                continue;
            }
            other => return other,
        }
    }
}

/// Execute a single instruction and potentially mutate it.
/// If the instruction is nonlocal, it is returned as ExecutionResult::NonLocal, but the base
/// energy cost is still paid and a mutation may occur.
fn try_execute_local(cell: &mut Cell, coord: Coord, params: &WorldParams) -> ExecutionResult {
    let Some(instruction) = cell.next_instruction() else {
        return ExecutionResult::NoInstruction;
    };
    let energy_cost = instruction.base_energy_cost() as u32;

    // Try to pay with free energy or else background radiation
    let initial_bg_rad = cell.bg_rad.0;
    let payment = cell.pay_cost(energy_cost, 0);
    let rad_for_mutation = match payment {
        CostPayment::FreeEnergy => None,
        CostPayment::UsedRadiation => Some(initial_bg_rad),
        CostPayment::Insufficient => {
            return ExecutionResult::Error(ExecutionError::NoEnergy);
        }
    };

    // Potentially mutate the instruction in the current program
    // Note this does not affect execution of the current instruction!
    if cell.check_mutation(params, rad_for_mutation) {
        let new_instruction = params
            .mutations
            .mutate_instruction(&mut cell.rng, instruction);
        *cell.next_instruction_mut().unwrap() = new_instruction;
    }

    // Increment instruction pointer after any mutation
    cell.inc_inst_ptr();

    if !instruction.is_local() {
        return ExecutionResult::NonLocal {
            target: coord + cell.cpu.dir.to_offset(),
            instruction: instruction,
        };
    }

    let result: Result<(), ExecutionError> = match instruction {
        Instruction::Nop => Ok(()),
        Instruction::Absorb => {
            cell.free_energy += cell.bg_rad.0 as u32;
            cell.bg_rad.0 = 0;
            if let Some(radiation) = cell.directed_rad.take() {
                cell.free_energy += 1;
                cell.cpu.msg = radiation.message;
                cell.cpu.msg_dir = radiation.direction;
                cell.cpu.flag = true;
            } else {
                cell.cpu.flag = false;
            }
            Ok(())
        }
        Instruction::Push0 => cell.cpu.push(0).map_err(|e| e.into()),
        Instruction::Push1 => cell.cpu.push(1).map_err(|e| e.into()),
        Instruction::Add => cell.cpu.add().map_err(|e| e.into()),
        Instruction::CW => {
            cell.cpu.dir = cell.cpu.dir.rotate_cw();
            Ok(())
        }
        _ => unreachable!("Non-local instructions should be handled earlier"),
    };

    if instruction != Instruction::Absorb {
        cell.cpu.flag = result.is_err();
    }

    match result {
        Ok(()) if instruction.execution_time() == 0 => return ExecutionResult::Immediate,
        Ok(()) => {
            cell.is_vulnerable = instruction.makes_vulnerable();
            return ExecutionResult::Complete;
        }
        Err(e) => {
            cell.is_vulnerable = true;
            return ExecutionResult::Error(e);
        }
    }
}
