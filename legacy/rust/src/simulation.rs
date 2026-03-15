use rayon::prelude::*;

use crate::cell::CostPayment;
use crate::executor::{run_tick_local, ExecutionResult};
use crate::instruction::{AdditionalCost, Instruction};
use crate::types::Coord;
use crate::world::{World, WorldParams};

pub struct Simulation {
    pub world: World,
}

struct Interaction {
    source: Coord,
    target: Coord,
    instruction: Instruction,
}

impl Simulation {
    pub fn new(params: WorldParams) -> Self {
        Self {
            world: World::new(params),
        }
    }

    pub fn tick(&mut self) {
        // First pass: Execute local instructions and collect nonlocal ones, paying their costs
        let interactions: Vec<_> = self
            .world
            .grid
            .par_iter_mut()
            .filter_map(
                |(cell, coord)| match run_tick_local(cell, coord, &self.world.params) {
                    ExecutionResult::NonLocal {
                        target,
                        instruction,
                    } => Some(Interaction {
                        source: coord,
                        target,
                        instruction,
                    }),
                    _ => None,
                },
            )
            .collect();

        // Second pass: nonlocal instructions
        // 1. Calculate and pay additional costs for nonlocal instructions. If the cost cannot be
        //    paid, the instruction fails.
        // 2. Nonlocal instructions targeting a protected cell fail.
        // 3. If two programs target one another, then only a single instruction can execute and the
        //    others fail. The winner is decided by the program with the highest strength. Ties are
        //    broken based on program size, and further ties result in both instructions failing.
        // 4. If a program/cell is targeted by multiple adjacent programs, then only a single
        //    instruction can execute and the others fail. The winner is decided by the program with
        //    the highest strength. Ties are broken based on program size. Further ties are broken
        //    if the targeted program is pointing (via its `Dir` register) to one of the targeting
        //    programs, else all instructions targeting that program fail.
        // 5. Execute the successful nonlocal instructions simultaneously. Note that there may be
        //    cycles. Note: can call execute_nonlocal for the successful nonlocal instruction

        // Calculate additional costs and check if they can be paid.
        // Pay costs (might need to be done sequentially?)
        // Filter out instructions that cannot be paid or target protected cells
        // Handle collisions etc

        let (costs, interactions): (Vec<_>, Vec<_>) = interactions
            .into_par_iter()
            .filter_map(|interaction| {
                let origin_cell = &self.world.grid[interaction.source];
                let target_cell = &self.world.grid[interaction.target];
                let cost = interaction.instruction.additional_cost(
                    origin_cell,
                    target_cell,
                    &self.world.params,
                );
                let can_pay_cost = origin_cell.can_pay_cost(cost.energy, cost.mass);
                match (can_pay_cost, target_cell.is_vulnerable) {
                    (true, true) => Some(((interaction.source, cost), Some(interaction))),
                    (true, false) => Some(((interaction.source, cost), None)),
                    (false, _) => None,
                }
            })
            .unzip();

        // NOTE(performance): probably some way to do this in parallel
        costs.into_iter().for_each(|(coord, cost)| {
            let origin_cell = &mut self.world.grid[coord];
            origin_cell.pay_cost(cost.energy, cost.mass);
        });

        // Update physics
        self.world.update_physics();
    }
}
