use crate::cell::Cell;
use crate::grid::Grid;
use crate::instruction::Instruction;
use crate::types::Coord;

#[derive(Clone, Debug)]
pub struct PendingInteraction {
    pub target: Coord,
    pub source: Coord,
    pub instruction: Instruction,
}

#[derive(Clone, Debug)]
enum ExecutionResult {
    Complete,
    NonLocal(PendingInteraction),
    NoProgram,
    OutOfEnergy,
}

/// Execute for 1 tick, stopping when we hit a nonlocal instruction.
/// Returns Some(PendingInteraction) if we hit a nonlocal instruction,
/// None if we completed local execution or encountered an error
pub fn run_tick_local(
    cell: &mut Cell,
) -> Option<PendingInteraction> {
    todo!();

    // // First execute immediate instructions until we hit a 1-tick instruction
    // let mut immediate_count = 0;
    // let program_size = cell.program.as_ref().map(|p| p.size).unwrap_or(0);

    // loop {
    //     match execute_instruction_local(cell) {
    //         Ok(ExecutionResult::Complete) => {
    //             // If this was an immediate instruction, continue executing
    //             if immediate_count < program_size {
    //                 immediate_count += 1;
    //                 continue;
    //             } else {
    //                 // We've executed too many immediate instructions, halt
    //                 cell.cpu.flag = true;
    //                 return None;
    //             }
    //         }
    //         Ok(ExecutionResult::NonLocal(interaction)) => {
    //             return Some(interaction);
    //         }
    //         Ok(ExecutionResult::NoProgram) => return None,
    //         Ok(ExecutionResult::OutOfEnergy) => return None,
    //         Err(_) => {
    //             cell.cpu.flag = true;
    //             return None;
    //         }
    //     }
    // }
}

/// Execute the next instruction. Assumes there is a program in Cell.
/// Returns Ok(ExecutionResult) on successful execution or error handling,
/// Err on unrecoverable errors
fn execute_instruction_local(
    cell: &mut Cell,
    coord: Coord,
) -> Result<ExecutionResult, &'static str> {
    todo!();
    // // Check if we have a program
    // let program = match &cell.program {
    //     Some(p) => p,
    //     None => return Ok(ExecutionResult::NoProgram),
    // };

    // // Get current plasmid and instruction
    // let plasmid = program
    //     .plasmids
    //     .get(cell.cpu.pp as usize)
    //     .ok_or("Invalid plasmid pointer")?;

    // let instruction = plasmid
    //     .instructions
    //     .get(cell.cpu.ip as usize)
    //     .ok_or("Invalid instruction pointer")?;

    // // Check if instruction is local
    // if !instruction.is_local() {
    //     // Calculate target based on CPU state
    //     let target = if cell.cpu.adj {
    //         // TODO: Calculate adjacent coordinate based on cell.cpu.dir
    //         todo!()
    //     } else {
    //         // Target self
    //         todo!() // Need current coordinates
    //     };

    //     return Ok(ExecutionResult::NonLocal(PendingInteraction {
    //         target,
    //         source: todo!(), // Need current coordinates
    //         instruction: *instruction,
    //     }));
    // }

    // // Check if we have enough energy for non-immediate instructions
    // if instruction.execution_time() > 0 {
    //     let cost = instruction.base_energy_cost();
    //     if cell.free_energy < cost as u32 && cell.background_radiation == 0 {
    //         return Ok(ExecutionResult::OutOfEnergy);
    //     }

    //     // Deduct energy cost
    //     if cell.free_energy >= cost as u32 {
    //         cell.free_energy -= cost as u32;
    //     } else {
    //         cell.background_radiation -= 1;
    //         // TODO: Increase mutation probability
    //     }
    // }

    // // Execute the instruction
    // match instruction {
    //     Instruction::Nop => {
    //         // Do nothing
    //         cell.cpu.ip += 1;
    //     }
    //     Instruction::Push0 => {
    //         cell.cpu.stack.push(0);
    //         cell.cpu.ip += 1;
    //     }
    //     Instruction::Push1 => {
    //         cell.cpu.stack.push(1);
    //         cell.cpu.ip += 1;
    //     }
    //     Instruction::Add => {
    //         if cell.cpu.stack.len() < 2 {
    //             cell.cpu.flag = true;
    //         } else {
    //             let b = cell.cpu.stack.pop().unwrap();
    //             let a = cell.cpu.stack.pop().unwrap();
    //             cell.cpu.stack.push(a.wrapping_add(b));
    //         }
    //         cell.cpu.ip += 1;
    //     }
    //     Instruction::CW => {
    //         cell.cpu.dir = cell.cpu.dir.rotate_cw();
    //         cell.cpu.ip += 1;
    //     }
    //     // TODO: Implement other instructions
    //     _ => return Err("Instruction not implemented"),
    // }

    // Ok(ExecutionResult::Complete)
}
