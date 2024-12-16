// NOTE: need global table of program sizes and free energy? may need to compute additional costs in local pass
// Use rayon for instruction execution. Local instructions execute
use crate::mutation::MutationParams;
use crate::world::{Grid, WorldParams};

#[derive(Clone, Debug)]
pub struct SimulationParams {
    mutation: MutationParams,
    world: WorldParams,
    move_rate: usize,
    maintenance_scale: usize,
}

#[derive(Clone, Debug)]
pub struct Simulation {
    world: Grid,
    parameters: SimulationParams,
}

impl Default for SimulationParams {
    fn default() -> Self {
        Self {
            mutation: MutationParams::default(),
            world: WorldParams::default(),
            move_rate: 8,
            maintenance_scale: 64,
        }
    }
}

/*
// EXAMPLE
use crossbeam::queue::SegQueue;
use std::sync::Arc;
use std::thread;

fn main() {
    // Create a thread-safe, lock-free queue
    let queue = Arc::new(SegQueue::new());

    // Number of worker threads
    let num_workers = 4;

    // Create worker threads to generate numbers and push them onto the queue
    let mut handles = vec![];

    for i in 0..num_workers {
        let queue_clone = Arc::clone(&queue);

        let handle = thread::spawn(move || {
            // Each worker pushes some numbers to the queue
            for j in 0..10 {
                let value = i * 10 + j;
                queue_clone.push(value);
                println!("Worker {} pushed {}", i, value);
            }
        });

        handles.push(handle);
    }

    // Wait for all threads to complete
    for handle in handles {
        handle.join().expect("Thread panicked");
    }

    // Once all production is complete, consume the queue
    let mut total_sum = 0;

    while let Some(value) = queue.pop() {
        total_sum += value;
    }

    println!("Total sum: {}", total_sum);
}
 */
