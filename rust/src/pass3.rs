//! Executes Pass 3 packet physics, ambient resource flow, and tick-end effects.

use crate::config::SimConfig;
use crate::grid::Grid;
use crate::model::{Cell, Direction, Packet, Program};
use crate::opcode::op;
use crate::random::{binomial, cell_rng, poisson};
#[cfg(feature = "rayon")]
use rayon::prelude::*;

const LISTEN_CAPTURE_SALT: u64 = 0x5d17_2ef3_94ab_c881;
const BG_RADIATION_SALT: u64 = 0x1f03_86da_b9c7_e251;
const BG_MASS_SALT: u64 = 0x2c69_4ab1_78de_3f44;
const MAINTENANCE_SALT: u64 = 0x78d2_0a45_4ecb_911f;
const DECAY_SALT: u64 = 0x42f5_c1a9_203d_b665;
const SPAWN_SALT: u64 = 0xbfd1_6a70_531c_2e84;
const MUTATION_SALT: u64 = 0xe3b9_1d8c_7a4f_5012;

/// Collects the ambient-phase outputs needed by the Pass 3 tail.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pass3AmbientOutput {
    pub spawn_candidates: Vec<bool>,
}

impl Pass3AmbientOutput {
    /// Allocates the ambient-phase output buffers for one grid size.
    pub fn new(cell_count: usize) -> Self {
        Self {
            spawn_candidates: vec![false; cell_count],
        }
    }
}

/// Reports the births and deaths produced by the Pass 3 tail.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Pass3TailOutput {
    pub deaths: u32,
    pub spontaneous_births: u32,
}

/// Bundles the immutable inputs needed by the Pass 3 tail.
#[derive(Clone, Copy, Debug)]
pub struct Pass3TailContext<'a> {
    pub incoming_writes: &'a [bool],
    pub spawn_candidates: &'a [bool],
    pub config: &'a SimConfig,
    pub tick: u64,
    pub seed: u64,
}

/// Advances packets through propagation, listening, collision, and survival.
pub fn pass3_packets(grid: &mut Grid, packets: &mut Vec<Packet>, tick: u64, seed: u64) {
    if packets.is_empty() {
        return;
    }

    for packet in packets.iter_mut() {
        packet.position = grid.neighbor(packet.position, packet.direction);
    }

    // Sorting avoids a grid-sized bucket allocation when packets are sparse,
    // while dense packet fields are faster with direct bucketing.
    if packets.len() > grid.len() / 4 {
        resolve_packets_bucketed(grid, packets, tick, seed);
        return;
    }

    packets.sort_by_key(|packet| packet.position);
    let mut run_start = 0;
    let mut survivor_count = 0;
    while run_start < packets.len() {
        let cell_index = packets[run_start].position;
        let run_end = run_start
            + packets[run_start..].partition_point(|packet| packet.position == cell_index);
        let bucket = &packets[run_start..run_end];
        let cell = grid.get_mut(cell_index).expect("cell should exist");
        if program(cell).is_some_and(|program| program.tick.did_listen) {
            apply_listen_capture(cell, cell_index, bucket, tick, seed);
        } else if bucket.len() >= 2 {
            cell.free_energy +=
                u32::try_from(bucket.len()).expect("packet count should fit in u32");
        } else {
            packets[survivor_count] = packets[run_start];
            survivor_count += 1;
        }

        run_start = run_end;
    }
    packets.truncate(survivor_count);
}

fn resolve_packets_bucketed(grid: &mut Grid, packets: &mut Vec<Packet>, tick: u64, seed: u64) {
    let mut buckets = vec![Vec::<Packet>::new(); grid.len()];
    for packet in packets.drain(..) {
        buckets[packet.position].push(packet);
    }

    let mut survivors = Vec::new();
    for (cell_index, bucket) in buckets.into_iter().enumerate() {
        if bucket.is_empty() {
            continue;
        }

        let cell = grid.get_mut(cell_index).expect("cell should exist");
        if program(cell).is_some_and(|program| program.tick.did_listen) {
            apply_listen_capture(cell, cell_index, &bucket, tick, seed);
        } else if bucket.len() >= 2 {
            cell.free_energy +=
                u32::try_from(bucket.len()).expect("packet count should fit in u32");
        } else {
            survivors.push(bucket[0]);
        }
    }

    *packets = survivors;
}

/// Runs the ambient-resource sub-steps of Pass 3 in spec order.
pub fn pass3_ambient(
    grid: &mut Grid,
    config: &SimConfig,
    tick: u64,
    seed: u64,
) -> Pass3AmbientOutput {
    let mut output = Pass3AmbientOutput::new(grid.len());

    resolve_absorb(grid);
    #[cfg(feature = "rayon")]
    resolve_ambient_cells_rayon(grid, config, tick, seed, &mut output);
    #[cfg(not(feature = "rayon"))]
    {
        resolve_background_radiation(grid, config, tick, seed);
        resolve_collect(grid);
        resolve_background_mass(grid, config, tick, seed, &mut output);
    }

    output
}

/// Runs the lifecycle, maintenance, decay, aging, and spawn tail of Pass 3.
pub fn pass3_tail(grid: &mut Grid, context: Pass3TailContext<'_>) -> Pass3TailOutput {
    assert_eq!(
        grid.len(),
        context.incoming_writes.len(),
        "incoming-write length must match grid size"
    );
    assert_eq!(
        grid.len(),
        context.spawn_candidates.len(),
        "spawn-candidate length must match grid size"
    );

    #[cfg(feature = "rayon")]
    {
        resolve_tail_cells_rayon(grid, context)
    }

    #[cfg(not(feature = "rayon"))]
    {
        resolve_inert_lifecycle(grid, context.incoming_writes);
        let deaths = resolve_maintenance(grid, context.config, context.tick, context.seed);
        resolve_free_resource_decay(grid, context.config, context.tick, context.seed);
        resolve_age_update(grid);
        let spontaneous_births = resolve_spontaneous_creation(
            grid,
            context.spawn_candidates,
            context.config,
            context.tick,
            context.seed,
        );

        Pass3TailOutput {
            deaths,
            spontaneous_births,
        }
    }
}

/// Applies end-of-tick mutation to programs that were live at tick start.
pub fn mutate_end_of_tick(grid: &mut Grid, config: &SimConfig, tick: u64, seed: u64) -> u32 {
    #[cfg(feature = "rayon")]
    {
        grid.cells_mut()
            .par_iter_mut()
            .enumerate()
            .map(|(cell_index, cell)| mutate_end_of_tick_cell(cell, config, tick, seed, cell_index))
            .sum()
    }

    #[cfg(not(feature = "rayon"))]
    {
        grid.cells_mut()
            .iter_mut()
            .enumerate()
            .map(|(cell_index, cell)| mutate_end_of_tick_cell(cell, config, tick, seed, cell_index))
            .sum()
    }
}

/// Resolves packet capture for one listening cell.
fn apply_listen_capture(
    cell: &mut Cell,
    cell_index: usize,
    packets: &[Packet],
    tick: u64,
    seed: u64,
) {
    let count = u32::try_from(packets.len()).expect("packet count should fit in u32");
    cell.free_energy += count;

    if packets.is_empty() {
        return;
    }

    let mut rng = cell_rng(seed ^ LISTEN_CAPTURE_SALT, tick, cell_index as u64);
    let choice = (rng.next_u64() % packets.len() as u64) as usize;
    let chosen = packets[choice];

    let program = program_mut(cell).expect("listening cell should contain a program");
    program.registers.msg = chosen.message;
    program.registers.dir = chosen.direction.opposite();
    program.registers.flag = true;
}

/// Splits background radiation across all absorb footprints.
fn resolve_absorb(grid: &mut Grid) {
    if !grid.cells().iter().any(|cell| {
        cell.program
            .as_ref()
            .is_some_and(|program| program.tick.absorb_count > 0)
    }) {
        return;
    }

    let mut buckets = vec![Vec::<usize>::new(); grid.len()];
    for source in 0..grid.len() {
        let Some(program) = grid
            .get(source)
            .expect("cell should exist")
            .program
            .as_ref()
        else {
            continue;
        };
        if program.tick.absorb_count == 0 {
            continue;
        }

        let dir = program.tick.absorb_dir.unwrap_or(program.registers.dir);
        for footprint_cell in absorb_footprint(grid, source, program.tick.absorb_count, dir) {
            buckets[footprint_cell].push(source);
        }
    }

    let mut gains = vec![0_u32; grid.len()];
    for (cell_index, absorbers) in buckets.into_iter().enumerate() {
        if absorbers.is_empty() {
            continue;
        }

        let bg = grid
            .get(cell_index)
            .expect("cell should exist")
            .bg_radiation;
        if bg == 0 {
            continue;
        }

        let absorber_count =
            u32::try_from(absorbers.len()).expect("absorber count should fit in u32");
        let share = bg / absorber_count;
        let remainder = bg % absorber_count;
        if share > 0 {
            for absorber in absorbers {
                gains[absorber] += share;
            }
        }

        grid.get_mut(cell_index)
            .expect("cell should exist")
            .bg_radiation = remainder;
    }

    for (cell_index, gain) in gains.into_iter().enumerate() {
        if gain == 0 {
            continue;
        }
        grid.get_mut(cell_index)
            .expect("cell should exist")
            .free_energy += gain;
    }
}

/// Runs radiation, collect, then mass for each cell in one Rayon traversal.
#[cfg(feature = "rayon")]
fn resolve_ambient_cells_rayon(
    grid: &mut Grid,
    config: &SimConfig,
    tick: u64,
    seed: u64,
    output: &mut Pass3AmbientOutput,
) {
    grid.cells_mut()
        .par_iter_mut()
        .zip(output.spawn_candidates.par_iter_mut())
        .enumerate()
        .for_each(|(cell_index, (cell, spawn_candidate))| {
            resolve_background_radiation_cell(cell, config, tick, seed, cell_index);
            resolve_collect_cell(cell);
            *spawn_candidate = resolve_background_mass_cell(cell, config, tick, seed, cell_index);
        });
}

/// Applies decay and Poisson arrival for background radiation.
#[cfg(not(feature = "rayon"))]
fn resolve_background_radiation(grid: &mut Grid, config: &SimConfig, tick: u64, seed: u64) {
    for (cell_index, cell) in grid.cells_mut().iter_mut().enumerate() {
        resolve_background_radiation_cell(cell, config, tick, seed, cell_index);
    }
}

/// Moves background mass into free mass for cells that collected this tick.
#[cfg(not(feature = "rayon"))]
fn resolve_collect(grid: &mut Grid) {
    for cell in grid.cells_mut() {
        resolve_collect_cell(cell);
    }
}

/// Applies decay and Poisson arrival for background mass and marks spawn candidates.
#[cfg(not(feature = "rayon"))]
fn resolve_background_mass(
    grid: &mut Grid,
    config: &SimConfig,
    tick: u64,
    seed: u64,
    output: &mut Pass3AmbientOutput,
) {
    for (cell_index, (cell, spawn_candidate)) in grid
        .cells_mut()
        .iter_mut()
        .zip(output.spawn_candidates.iter_mut())
        .enumerate()
    {
        *spawn_candidate = resolve_background_mass_cell(cell, config, tick, seed, cell_index);
    }
}

/// Runs lifecycle, maintenance, decay, age, then spawn per cell in one Rayon traversal.
#[cfg(feature = "rayon")]
fn resolve_tail_cells_rayon(grid: &mut Grid, context: Pass3TailContext<'_>) -> Pass3TailOutput {
    let (deaths, spontaneous_births) = grid
        .cells_mut()
        .par_iter_mut()
        .enumerate()
        .map(|(cell_index, cell)| {
            resolve_inert_lifecycle_cell(cell, context.incoming_writes[cell_index]);
            let deaths = resolve_maintenance_cell(
                cell,
                context.config,
                context.tick,
                context.seed,
                cell_index,
            );
            resolve_free_resource_decay_cell(
                cell,
                context.config,
                context.tick,
                context.seed,
                cell_index,
            );
            resolve_age_update_cell(cell);
            let births = resolve_spontaneous_creation_cell(
                cell,
                context.spawn_candidates[cell_index],
                context.config,
                context.tick,
                context.seed,
                cell_index,
            );
            (deaths, births)
        })
        .reduce(
            || (0, 0),
            |(left_deaths, left_births), (right_deaths, right_births)| {
                (left_deaths + right_deaths, left_births + right_births)
            },
        );

    Pass3TailOutput {
        deaths,
        spontaneous_births,
    }
}

/// Updates inert abandonment timers and openness after Pass 2 writes.
#[cfg(not(feature = "rayon"))]
fn resolve_inert_lifecycle(grid: &mut Grid, incoming_writes: &[bool]) {
    for (cell_index, cell) in grid.cells_mut().iter_mut().enumerate() {
        resolve_inert_lifecycle_cell(cell, incoming_writes[cell_index]);
    }
}

/// Charges maintenance to programs that still exist after the grace checks.
#[cfg(not(feature = "rayon"))]
fn resolve_maintenance(grid: &mut Grid, config: &SimConfig, tick: u64, seed: u64) -> u32 {
    grid.cells_mut()
        .iter_mut()
        .enumerate()
        .map(|(cell_index, cell)| resolve_maintenance_cell(cell, config, tick, seed, cell_index))
        .sum()
}

/// Burns maintenance quanta out of free energy, then program code, then live state.
fn apply_maintenance(cell: &mut Cell, mut quanta: u32) -> bool {
    let energy_paid = cell.free_energy.min(quanta);
    cell.free_energy -= energy_paid;
    quanta -= energy_paid;

    let mass_paid = cell.free_mass.min(quanta);
    cell.free_mass -= mass_paid;
    quanta -= mass_paid;

    if quanta == 0 {
        return false;
    }

    let Some(program) = cell.program.as_mut() else {
        return false;
    };
    let destroy = usize::try_from(quanta).expect("maintenance quanta should fit in usize");
    let new_len = program.code.len().saturating_sub(destroy);
    program.code.truncate(new_len);
    if program.code.is_empty() {
        cell.program = None;
        return true;
    }

    false
}

/// Decays free resources above the per-cell threshold.
#[cfg(not(feature = "rayon"))]
fn resolve_free_resource_decay(grid: &mut Grid, config: &SimConfig, tick: u64, seed: u64) {
    for (cell_index, cell) in grid.cells_mut().iter_mut().enumerate() {
        resolve_free_resource_decay_cell(cell, config, tick, seed, cell_index);
    }
}

/// Increments age for programs that were live at tick start.
#[cfg(not(feature = "rayon"))]
fn resolve_age_update(grid: &mut Grid) {
    for cell in grid.cells_mut().iter_mut() {
        resolve_age_update_cell(cell);
    }
}

/// Spawns new one-cell programs in empty candidate cells.
#[cfg(not(feature = "rayon"))]
fn resolve_spontaneous_creation(
    grid: &mut Grid,
    spawn_candidates: &[bool],
    config: &SimConfig,
    tick: u64,
    seed: u64,
) -> u32 {
    grid.cells_mut()
        .iter_mut()
        .enumerate()
        .map(|(cell_index, cell)| {
            resolve_spontaneous_creation_cell(
                cell,
                spawn_candidates[cell_index],
                config,
                tick,
                seed,
                cell_index,
            )
        })
        .sum()
}

fn mutate_end_of_tick_cell(
    cell: &mut Cell,
    config: &SimConfig,
    tick: u64,
    seed: u64,
    cell_index: usize,
) -> u32 {
    let Some(program) = cell.program.as_ref() else {
        return 0;
    };
    if !program.tick.was_live_at_tick_start {
        return 0;
    }

    let probability = mutation_probability(program, config);
    let mut rng = cell_rng(seed ^ MUTATION_SALT, tick, cell_index as u64);
    if !rng.bernoulli(probability) {
        return 0;
    }

    let program = cell
        .program
        .as_mut()
        .expect("program should still exist when mutating");
    let instruction_index = (rng.next_u64() % program.code.len() as u64) as usize;
    let bit_index = (rng.next_u64() % 8) as u8;
    program.code[instruction_index] ^= 1_u8 << bit_index;
    1
}

fn resolve_background_radiation_cell(
    cell: &mut Cell,
    config: &SimConfig,
    tick: u64,
    seed: u64,
    cell_index: usize,
) {
    let mut rng = cell_rng(seed ^ BG_RADIATION_SALT, tick, cell_index as u64);
    let decayed = binomial(&mut rng, cell.bg_radiation, config.d_energy);
    let remaining = cell.bg_radiation - decayed;
    let arrivals = poisson(&mut rng, config.r_energy);
    cell.bg_radiation = remaining.saturating_add(arrivals);
}

fn resolve_collect_cell(cell: &mut Cell) {
    if cell
        .program
        .as_ref()
        .is_some_and(|program| program.tick.did_collect)
    {
        cell.free_mass += cell.bg_mass;
        cell.bg_mass = 0;
    }
}

fn resolve_background_mass_cell(
    cell: &mut Cell,
    config: &SimConfig,
    tick: u64,
    seed: u64,
    cell_index: usize,
) -> bool {
    let mut rng = cell_rng(seed ^ BG_MASS_SALT, tick, cell_index as u64);
    let decayed = binomial(&mut rng, cell.bg_mass, config.d_mass);
    let remaining = cell.bg_mass - decayed;
    let arrivals = poisson(&mut rng, config.r_mass);

    cell.bg_mass = remaining.saturating_add(arrivals);
    arrivals > 0 && !cell.has_program()
}

fn resolve_inert_lifecycle_cell(cell: &mut Cell, incoming_write: bool) {
    let Some(program) = cell.program.as_mut() else {
        return;
    };
    if !program.is_inert() {
        return;
    }

    if incoming_write {
        program.abandonment_timer = 0;
    } else {
        program.abandonment_timer = program.abandonment_timer.wrapping_add(1);
    }
    program.tick.is_open = true;
}

fn resolve_maintenance_cell(
    cell: &mut Cell,
    config: &SimConfig,
    tick: u64,
    seed: u64,
    cell_index: usize,
) -> u32 {
    let Some(program) = cell.program.as_ref() else {
        return 0;
    };
    if !program.tick.existed_at_tick_start || program.tick.is_newborn {
        return 0;
    }

    let rate = if program.live {
        config.maintenance_rate
    } else if program.abandonment_timer < config.inert_grace_ticks {
        0.0
    } else {
        config.maintenance_rate
    };
    if rate <= 0.0 {
        return 0;
    }

    let q = f64::from(program.size()).powf(config.maintenance_exponent);
    let whole = q.floor() as u32;
    let fractional = (q - f64::from(whole)) * rate;
    let mut rng = cell_rng(seed ^ MAINTENANCE_SALT, tick, cell_index as u64);
    let mut quanta = binomial(&mut rng, whole, rate);
    quanta += u32::from(rng.bernoulli(fractional));
    if quanta == 0 {
        return 0;
    }

    u32::from(apply_maintenance(cell, quanta))
}

fn resolve_free_resource_decay_cell(
    cell: &mut Cell,
    config: &SimConfig,
    tick: u64,
    seed: u64,
    cell_index: usize,
) {
    let mut rng = cell_rng(seed ^ DECAY_SALT, tick, cell_index as u64);
    let threshold = resource_threshold(cell, config);

    let energy_excess = (f64::from(cell.free_energy) - threshold).max(0.0).floor() as u32;
    let energy_decay = binomial(&mut rng, energy_excess, config.d_energy);
    cell.free_energy -= energy_decay;

    let mass_excess = (f64::from(cell.free_mass) - threshold).max(0.0).floor() as u32;
    let mass_decay = binomial(&mut rng, mass_excess, config.d_mass);
    cell.free_mass -= mass_decay;
}

fn resolve_age_update_cell(cell: &mut Cell) {
    let Some(program) = cell.program.as_mut() else {
        return;
    };
    if !program.tick.was_live_at_tick_start {
        return;
    }
    program.age = program.age.wrapping_add(1);
}

fn resolve_spontaneous_creation_cell(
    cell: &mut Cell,
    is_spawn_candidate: bool,
    config: &SimConfig,
    tick: u64,
    seed: u64,
    cell_index: usize,
) -> u32 {
    if !is_spawn_candidate || cell.has_program() {
        return 0;
    }

    let mut rng = cell_rng(seed ^ SPAWN_SALT, tick, cell_index as u64);
    if !rng.bernoulli(config.p_spawn) {
        return 0;
    }

    let dir = Direction::ALL[(rng.next_u32() % Direction::ALL.len() as u32) as usize];
    let id = rng.next_u32() as u8;
    let mut program =
        Program::new_live(vec![op::NOP], dir, id).expect("spawned program should be valid");
    program.tick.is_newborn = true;

    cell.program = Some(program);
    cell.free_energy += cell.bg_radiation;
    cell.free_mass += cell.bg_mass;
    cell.bg_radiation = 0;
    cell.bg_mass = 0;
    1
}

/// Computes the free-resource decay threshold for one cell.
fn resource_threshold(cell: &Cell, config: &SimConfig) -> f64 {
    let size = cell
        .program
        .as_ref()
        .map_or(0.0, |program| f64::from(program.size()));
    config.t_cap * size
}

/// Computes the mutation probability for one program at tick end.
fn mutation_probability(program: &Program, config: &SimConfig) -> f64 {
    if program.tick.bg_radiation_consumed > 0 {
        let denominator = 2_f64.powi(config.mutation_background_log2 as i32);
        (f64::from(program.tick.bg_radiation_consumed) / denominator).min(1.0)
    } else {
        2_f64.powi(-(config.mutation_base_log2 as i32))
    }
}

/// Returns the set of cells covered by one absorb footprint.
fn absorb_footprint(
    grid: &Grid,
    source: usize,
    count: u8,
    dir: crate::model::Direction,
) -> Vec<usize> {
    let mut cells = Vec::with_capacity(5);
    push_unique(&mut cells, source);
    if count >= 2 {
        push_unique(&mut cells, grid.neighbor(source, dir));
    }
    if count >= 3 {
        push_unique(&mut cells, grid.neighbor(source, dir.clockwise()));
        push_unique(&mut cells, grid.neighbor(source, dir.counterclockwise()));
    }
    if count >= 4 {
        push_unique(&mut cells, grid.neighbor(source, dir.opposite()));
    }
    cells
}

/// Pushes a cell index into a list only if it is not already present.
fn push_unique(cells: &mut Vec<usize>, value: usize) {
    if !cells.contains(&value) {
        cells.push(value);
    }
}

/// Returns the program stored in a cell, if any.
fn program(cell: &Cell) -> Option<&Program> {
    cell.program.as_ref()
}

/// Returns the mutable program stored in a cell, if any.
fn program_mut(cell: &mut Cell) -> Option<&mut Program> {
    cell.program.as_mut()
}

#[cfg(test)]
mod tests {
    use crate::config::SimConfig;
    use crate::grid::Grid;
    use crate::model::{Cell, Direction, Packet, Program};

    use super::{pass3_ambient, pass3_packets, resolve_packets_bucketed, Pass3AmbientOutput};
    use crate::opcode::op;

    #[test]
    fn packet_phase_propagates_and_persists_single_packets() {
        let mut grid = Grid::new(3, 1).expect("grid should build");
        let mut packets = vec![Packet {
            position: 0,
            direction: Direction::Right,
            message: 7,
        }];

        pass3_packets(&mut grid, &mut packets, 0, 11);

        assert_eq!(
            packets,
            vec![Packet {
                position: 1,
                direction: Direction::Right,
                message: 7,
            }]
        );
        assert_eq!(grid.get(1).expect("cell should exist").free_energy, 0);
    }

    #[test]
    fn collisions_convert_packets_to_free_energy() {
        let mut grid = Grid::new(3, 1).expect("grid should build");
        let mut packets = vec![
            Packet {
                position: 0,
                direction: Direction::Right,
                message: 1,
            },
            Packet {
                position: 2,
                direction: Direction::Left,
                message: 2,
            },
        ];

        pass3_packets(&mut grid, &mut packets, 0, 11);

        assert!(packets.is_empty());
        assert_eq!(grid.get(1).expect("cell should exist").free_energy, 2);
    }

    #[test]
    fn listening_captures_packets_and_sets_message_and_arrival_direction() {
        let mut cell = Cell::with_program(
            Program::new_live(vec![op::LISTEN], Direction::Up, 4).expect("program should build"),
        );
        cell.program
            .as_mut()
            .expect("program should exist")
            .tick
            .did_listen = true;

        let mut grid =
            Grid::from_cells(2, 1, vec![Cell::default(), cell]).expect("grid should build");
        let mut packets = vec![Packet {
            position: 0,
            direction: Direction::Right,
            message: 9,
        }];

        pass3_packets(&mut grid, &mut packets, 0, 11);

        let listener = grid.get(1).expect("cell should exist");
        let program = listener.program.as_ref().expect("program should exist");
        assert!(packets.is_empty());
        assert_eq!(listener.free_energy, 1);
        assert_eq!(program.registers.msg, 9);
        assert_eq!(program.registers.dir, Direction::Left);
        assert!(program.registers.flag);
    }

    #[test]
    fn sparse_sorted_packet_path_matches_bucketed_reference() {
        let mut listener = Cell::with_program(
            Program::new_live(vec![op::LISTEN], Direction::Up, 4).expect("program should build"),
        );
        listener
            .program
            .as_mut()
            .expect("program should exist")
            .tick
            .did_listen = true;
        let mut cells = vec![Cell::default(); 16];
        cells[7] = listener;
        let grid = Grid::from_cells(16, 1, cells).expect("grid should build");
        let packets = vec![
            Packet {
                position: 1,
                direction: Direction::Right,
                message: 1,
            },
            Packet {
                position: 3,
                direction: Direction::Left,
                message: 2,
            },
            Packet {
                position: 6,
                direction: Direction::Right,
                message: 3,
            },
            Packet {
                position: 11,
                direction: Direction::Right,
                message: 4,
            },
        ];

        let mut sorted_grid = grid.clone();
        let mut sorted_packets = packets.clone();
        pass3_packets(&mut sorted_grid, &mut sorted_packets, 5, 11);

        let mut bucketed_grid = grid;
        let mut bucketed_packets = packets;
        for packet in &mut bucketed_packets {
            packet.position = bucketed_grid.neighbor(packet.position, packet.direction);
        }
        resolve_packets_bucketed(&mut bucketed_grid, &mut bucketed_packets, 5, 11);

        assert_eq!(sorted_grid, bucketed_grid);
        assert_eq!(sorted_packets, bucketed_packets);
    }

    #[test]
    fn absorb_distribution_splits_background_radiation_and_leaves_remainder() {
        let mut left = Cell::with_program(
            Program::new_live(vec![op::ABSORB], Direction::Right, 1).expect("program should build"),
        );
        let mut right = Cell::with_program(
            Program::new_live(vec![op::ABSORB], Direction::Left, 2).expect("program should build"),
        );
        left.program
            .as_mut()
            .expect("program should exist")
            .tick
            .absorb_count = 2;
        left.program
            .as_mut()
            .expect("program should exist")
            .tick
            .absorb_dir = Some(Direction::Right);
        right
            .program
            .as_mut()
            .expect("program should exist")
            .tick
            .absorb_count = 2;
        right
            .program
            .as_mut()
            .expect("program should exist")
            .tick
            .absorb_dir = Some(Direction::Left);

        let center = Cell {
            bg_radiation: 5,
            ..Cell::default()
        };

        let mut grid =
            Grid::from_cells(3, 1, vec![left, center, right]).expect("grid should build");
        let config = SimConfig {
            d_energy: 0.0,
            r_energy: 0.0,
            d_mass: 0.0,
            r_mass: 0.0,
            ..SimConfig::default()
        };

        let output = pass3_ambient(&mut grid, &config, 0, 5);

        assert_eq!(
            output,
            Pass3AmbientOutput {
                spawn_candidates: vec![false, false, false]
            }
        );
        assert_eq!(grid.get(0).expect("cell should exist").free_energy, 2);
        assert_eq!(grid.get(1).expect("cell should exist").bg_radiation, 1);
        assert_eq!(grid.get(2).expect("cell should exist").free_energy, 2);
    }
}
