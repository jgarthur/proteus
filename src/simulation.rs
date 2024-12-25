use rayon::prelude::*;

use crate::executor::{run_tick_local, ExecutionResult};
use crate::instruction::Instruction;
use crate::types::Coord;
use crate::world::{World, WorldParams};

pub struct Simulation {
    pub world: World,
}

impl Simulation {
    pub fn new(params: WorldParams) -> Self {
        Self {
            world: World::new(params),
        }
    }

    pub fn tick(&mut self) {
        // First pass: Execute local instructions and collect nonlocal ones
        let nonlocal_instructions: Vec<_> = self
            .world
            .grid
            .par_iter_mut()
            .flat_map(
                |(cell, coord)| match run_tick_local(cell, coord, &self.world.params) {
                    ExecutionResult::NonLocal {
                        instruction,
                        target,
                    } => Some((instruction, coord, target)),
                    _ => None,
                },
            )
            .collect();

        // Second pass: Execute nonlocal instructions
        for (instruction, source, target) in nonlocal_instructions {
            self.execute_nonlocal(instruction, source, target);
        }

        // Update physics
        self.world.update_physics();
    }

    fn execute_nonlocal(&mut self, _instruction: Instruction, _source: Coord, _target: Coord) {
        // Implementation of nonlocal instruction execution
        // This would handle things like Move, Emit, etc.
        // You'll need to implement this based on your requirements
        return;
    }
}
