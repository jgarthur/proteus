use crate::cell::Cell;
use crate::cpu::CPUError;
use crate::instruction::Instruction;
use crate::types::Coord;
use crate::world::WorldParams;

pub enum ExecutionResult {
    Complete,
    Immediate,
    NonLocal {
        instruction: Instruction,
        target: Coord,
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
#[inline(never)]
pub fn run_tick_local(cell: &mut Cell, coord: Coord, params: &WorldParams) -> ExecutionResult {
    let mut immediate_count = 0;

    loop {
        let result = execute_instruction_local(cell, coord, params);
        match result {
            ExecutionResult::Complete => {
                // cell.is_vulnerable = instruction.makes_vulnerable(); TODO
            }
            ExecutionResult::Immediate => {
                immediate_count += 1;
                if immediate_count == cell.program_size() {
                    cell.is_vulnerable = true;
                    return ExecutionResult::Error(ExecutionError::Halted);
                } else {
                    continue;
                }
            }
            ExecutionResult::Error(_) => {
                // cell.is_vulnerable = true; TODO
            }
            _ => {}
        };
        return result;
    }
}

/// Execute a single instruction and potentially mutate it
fn execute_instruction_local(
    cell: &mut Cell,
    coord: Coord,
    params: &WorldParams,
) -> ExecutionResult {
    let Some(instruction) = cell.next_instruction() else {
        return ExecutionResult::NoInstruction;
    };
    let energy_cost = instruction.base_energy_cost() as u32;

    // Try to pay with free energy or else background radiation
    // The type of energy used affects the chance of mutation
    let bg_rad_for_mutation: Option<u8>;
    if cell.free_energy >= energy_cost {
        cell.free_energy -= energy_cost;
        bg_rad_for_mutation = None;
    } else if cell.bg_rad.0 >= instruction.base_energy_cost() {
        bg_rad_for_mutation = Some(cell.bg_rad.0);
        cell.bg_rad.0 -= instruction.base_energy_cost();
    } else {
        return ExecutionResult::Error(ExecutionError::NoEnergy);
    }

    // Base energy is now paid and instruction will be executed

    // Potentially mutate the instruction in the current program
    // Note this does not affect execution of the current instruction!
    if cell.check_mutation(params, bg_rad_for_mutation) {
        let new_instruction = params
            .mutations
            .mutate_instruction(&mut cell.rng, instruction);
        *cell.next_instruction_mut().unwrap() = new_instruction;
    }

    // Increment instruction pointer after mutation
    cell.inc_inst_ptr();

    if !instruction.is_local() {
        return ExecutionResult::NonLocal {
            instruction: instruction,
            target: coord + cell.cpu.dir.to_offset(),
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

    match result {
        Ok(()) if instruction.execution_time() == 0 => ExecutionResult::Immediate,
        Ok(()) => {
            cell.is_vulnerable = instruction.makes_vulnerable();
            ExecutionResult::Complete
        }
        Err(e) => {
            cell.is_vulnerable = true;
            ExecutionResult::Error(e)
        }
    }
}
