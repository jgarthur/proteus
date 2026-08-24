//! Executes Pass 3 packet physics, ambient resource flow, and tick-end effects.

use crate::config::SimConfig;
use crate::grid::Grid;
use crate::model::{Cell, Direction, Lineage, Packet, Program, ProgramOrigin, ProgramUid};
use crate::opcode::op;
use crate::random::{
    bernoulli_pow2, binomial_pow2, cell_rng, poisson, PoissonInverter, POISSON_INVERSION_MAX_RATE,
};
#[cfg(feature = "rayon")]
use rayon::prelude::*;
use std::iter::Sum;
use std::ops::Add;

const LISTEN_CAPTURE_SALT: u64 = 0x5d17_2ef3_94ab_c881;
const BG_RADIATION_SALT: u64 = 0x1f03_86da_b9c7_e251;
const BG_MASS_SALT: u64 = 0x2c69_4ab1_78de_3f44;
const MAINTENANCE_SALT: u64 = 0x78d2_0a45_4ecb_911f;
const DECAY_SALT: u64 = 0x42f5_c1a9_203d_b665;
const SPAWN_SALT: u64 = 0xbfd1_6a70_531c_2e84;
const MUTATION_SALT: u64 = 0xe3b9_1d8c_7a4f_5012;

#[derive(Clone, Copy, Debug)]
struct AmbientSamplers {
    d_energy_log2: Option<u32>,
    d_mass_log2: Option<u32>,
    energy_arrivals: Option<PoissonInverter>,
    mass_arrivals: Option<PoissonInverter>,
}

impl AmbientSamplers {
    fn new(config: &SimConfig) -> Self {
        Self {
            d_energy_log2: config.d_energy_log2,
            d_mass_log2: config.d_mass_log2,
            energy_arrivals: poisson_inverter(config.r_energy),
            mass_arrivals: poisson_inverter(config.r_mass),
        }
    }
}

fn poisson_inverter(rate: f64) -> Option<PoissonInverter> {
    (rate <= POISSON_INVERSION_MAX_RATE).then(|| PoissonInverter::new(rate))
}

fn sample_poisson(
    rng: &mut crate::random::WyRand,
    rate: f64,
    inverter: Option<PoissonInverter>,
) -> u32 {
    match inverter {
        Some(inverter) => inverter.sample(rng),
        None => poisson(rng, rate),
    }
}

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

/// Attributes end-of-tick mutations to their configured cause.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MutationCounts {
    pub base: u32,
    pub background: u32,
}

impl Add for MutationCounts {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self {
            base: self.base + rhs.base,
            background: self.background + rhs.background,
        }
    }
}

impl Sum<MutationCounts> for MutationCounts {
    fn sum<I: Iterator<Item = MutationCounts>>(iter: I) -> Self {
        iter.fold(Self::default(), Add::add)
    }
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
    let samplers = AmbientSamplers::new(config);

    resolve_absorb(grid);
    #[cfg(feature = "rayon")]
    resolve_ambient_cells_rayon(grid, config, samplers, tick, seed, &mut output);
    #[cfg(not(feature = "rayon"))]
    {
        resolve_background_radiation(grid, config, samplers, tick, seed);
        resolve_collect(grid);
        resolve_background_mass(grid, config, samplers, tick, seed, &mut output);
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
pub fn mutate_end_of_tick(
    grid: &mut Grid,
    config: &SimConfig,
    tick: u64,
    seed: u64,
) -> MutationCounts {
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
    samplers: AmbientSamplers,
    tick: u64,
    seed: u64,
    output: &mut Pass3AmbientOutput,
) {
    grid.cells_mut()
        .par_iter_mut()
        .zip(output.spawn_candidates.par_iter_mut())
        .enumerate()
        .for_each(|(cell_index, (cell, spawn_candidate))| {
            resolve_background_radiation_cell(cell, config, samplers, tick, seed, cell_index);
            resolve_collect_cell(cell);
            *spawn_candidate =
                resolve_background_mass_cell(cell, config, samplers, tick, seed, cell_index);
        });
}

/// Applies decay and Poisson arrival for background radiation.
#[cfg(not(feature = "rayon"))]
fn resolve_background_radiation(
    grid: &mut Grid,
    config: &SimConfig,
    samplers: AmbientSamplers,
    tick: u64,
    seed: u64,
) {
    for (cell_index, cell) in grid.cells_mut().iter_mut().enumerate() {
        resolve_background_radiation_cell(cell, config, samplers, tick, seed, cell_index);
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
    samplers: AmbientSamplers,
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
        *spawn_candidate =
            resolve_background_mass_cell(cell, config, samplers, tick, seed, cell_index);
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

#[cfg(test)]
std::thread_local! {
    static BACKGROUND_MUTATION_CALLS: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

fn background_mutation_fires(
    rng: &mut crate::random::WyRand,
    background_consumed: u32,
    exponent: u32,
) -> bool {
    #[cfg(test)]
    BACKGROUND_MUTATION_CALLS.with(|calls| calls.set(calls.get() + 1));

    binomial_pow2(rng, background_consumed, exponent) > 0
}

fn mutate_end_of_tick_cell(
    cell: &mut Cell,
    config: &SimConfig,
    tick: u64,
    seed: u64,
    cell_index: usize,
) -> MutationCounts {
    let Some(program) = cell.program.as_ref() else {
        return MutationCounts::default();
    };
    if !program.tick.was_live_at_tick_start {
        return MutationCounts::default();
    }

    let background_consumed = program.tick.bg_radiation_consumed;
    let mut rng = cell_rng(seed ^ MUTATION_SALT, tick, cell_index as u64);
    let (should_mutate, count) = if background_consumed > 0 {
        (
            background_mutation_fires(
                &mut rng,
                background_consumed,
                config.mutation_background_log2,
            ),
            MutationCounts {
                base: 0,
                background: 1,
            },
        )
    } else {
        (
            bernoulli_pow2(&mut rng, config.mutation_base_log2),
            MutationCounts {
                base: 1,
                background: 0,
            },
        )
    };
    if !should_mutate {
        return MutationCounts::default();
    }

    let program = cell
        .program
        .as_mut()
        .expect("program should still exist when mutating");
    let instruction_index = (rng.next_u64() % program.code.len() as u64) as usize;
    let bit_index = (rng.next_u64() % 8) as u8;
    program.code[instruction_index] ^= 1_u8 << bit_index;
    count
}

fn resolve_background_radiation_cell(
    cell: &mut Cell,
    config: &SimConfig,
    samplers: AmbientSamplers,
    tick: u64,
    seed: u64,
    cell_index: usize,
) {
    let mut rng = cell_rng(seed ^ BG_RADIATION_SALT, tick, cell_index as u64);
    let decayed = samplers.d_energy_log2.map_or(0, |exponent| {
        binomial_pow2(&mut rng, cell.bg_radiation, exponent)
    });
    let remaining = cell.bg_radiation - decayed;
    let arrivals = sample_poisson(&mut rng, config.r_energy, samplers.energy_arrivals);
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
    samplers: AmbientSamplers,
    tick: u64,
    seed: u64,
    cell_index: usize,
) -> bool {
    let mut rng = cell_rng(seed ^ BG_MASS_SALT, tick, cell_index as u64);
    let decayed = samplers.d_mass_log2.map_or(0, |exponent| {
        binomial_pow2(&mut rng, cell.bg_mass, exponent)
    });
    let remaining = cell.bg_mass - decayed;
    let arrivals = sample_poisson(&mut rng, config.r_mass, samplers.mass_arrivals);

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

    let rate_log2 = if program.live {
        config.maintenance_rate_log2
    } else if program.abandonment_timer < config.inert_grace_ticks {
        None
    } else {
        config.maintenance_rate_log2
    };
    let Some(rate_log2) = rate_log2 else {
        return 0;
    };

    let size = f64::from(program.size());
    let q = if config.maintenance_exponent == 1.0 {
        size
    } else {
        size.powf(config.maintenance_exponent)
    };
    let whole = q.floor() as u32;
    let rate = 2_f64.powi(-(rate_log2 as i32));
    let fractional = (q - f64::from(whole)) * rate;
    let mut rng = cell_rng(seed ^ MAINTENANCE_SALT, tick, cell_index as u64);
    let mut quanta = binomial_pow2(&mut rng, whole, rate_log2);
    // For non-integer maintenance exponents, the fractional product is not
    // generally dyadic and deliberately retains the f64 Bernoulli draw.
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
    let energy_decay = config.d_energy_log2.map_or(0, |exponent| {
        binomial_pow2(&mut rng, energy_excess, exponent)
    });
    cell.free_energy -= energy_decay;

    let mass_excess = (f64::from(cell.free_mass) - threshold).max(0.0).floor() as u32;
    let mass_decay = config
        .d_mass_log2
        .map_or(0, |exponent| binomial_pow2(&mut rng, mass_excess, exponent));
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
    let Some(spawn_exponent) = config.p_spawn_log2 else {
        return 0;
    };
    let mut rng = cell_rng(seed ^ SPAWN_SALT, tick, cell_index as u64);
    if !bernoulli_pow2(&mut rng, spawn_exponent) {
        return 0;
    }

    let dir = Direction::ALL[(rng.next_u32() % Direction::ALL.len() as u32) as usize];
    let id = rng.next_u32() as u8;
    let mut program =
        Program::new_live(vec![op::NOP], dir, id).expect("spawned program should be valid");
    program.tick.is_newborn = true;

    // Lineage only; no RNG draw, and every value is a pure function of the
    // creation site, so serial and Rayon traversals agree. SPEC.md calls spawn
    // the primordial bootstrap, so a spawned program is a parentless root.
    let cell_count = config.cell_count().unwrap_or(0);
    let uid = ProgramUid::create(tick, cell_index, cell_count, ProgramOrigin::Spawn);
    program.lineage = Lineage::root(uid, tick);

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

    use super::{
        background_mutation_fires, mutate_end_of_tick_cell, pass3_ambient, pass3_packets,
        resolve_packets_bucketed, resolve_spontaneous_creation_cell, MutationCounts,
        Pass3AmbientOutput, BACKGROUND_MUTATION_CALLS, MUTATION_SALT, SPAWN_SALT,
    };
    use crate::model::{Lineage, ProgramOrigin, ProgramSite, ProgramUid};
    use crate::opcode::op;
    use crate::random::{bernoulli_pow2, binomial_pow2, cell_rng};

    #[test]
    fn base_mutation_is_attributed_to_the_base_counter() {
        let mut cell = Cell::with_program(
            Program::new_live(vec![op::NOP], Direction::Up, 1).expect("program should build"),
        );
        cell.program
            .as_mut()
            .expect("program should exist")
            .tick
            .was_live_at_tick_start = true;
        let config = SimConfig {
            mutation_base_log2: 0,
            ..SimConfig::default()
        };

        assert_eq!(
            mutate_end_of_tick_cell(&mut cell, &config, 9, 17, 0),
            MutationCounts {
                base: 1,
                background: 0,
            }
        );
    }

    #[test]
    fn background_mutation_is_attributed_to_the_background_counter() {
        let mut cell = Cell::with_program(
            Program::new_live(vec![op::NOP], Direction::Up, 1).expect("program should build"),
        );
        let program = cell.program.as_mut().expect("program should exist");
        program.tick.was_live_at_tick_start = true;
        program.tick.bg_radiation_consumed = 1;
        let config = SimConfig {
            mutation_background_log2: 0,
            ..SimConfig::default()
        };

        assert_eq!(
            mutate_end_of_tick_cell(&mut cell, &config, 9, 17, 0),
            MutationCounts {
                base: 0,
                background: 1,
            }
        );
    }

    #[test]
    fn non_firing_mutation_has_zero_cause_counts() {
        let tick = 9;
        let cell_index = 0;
        let seed = (0_u64..)
            .find(|seed| {
                let mut rng = cell_rng(*seed ^ MUTATION_SALT, tick, cell_index as u64);
                !bernoulli_pow2(&mut rng, 1)
            })
            .expect("a non-firing base-mutation stream should exist");
        let mut cell = Cell::with_program(
            Program::new_live(vec![op::NOP], Direction::Up, 1).expect("program should build"),
        );
        cell.program
            .as_mut()
            .expect("program should exist")
            .tick
            .was_live_at_tick_start = true;
        let config = SimConfig {
            mutation_base_log2: 1,
            ..SimConfig::default()
        };

        assert_eq!(
            mutate_end_of_tick_cell(&mut cell, &config, tick, seed, cell_index),
            MutationCounts::default()
        );
    }

    /// Covers the base branch where the sampler actually draws *and* fires.
    ///
    /// `base_mutation_is_attributed_to_the_base_counter` pins attribution at
    /// `mutation_base_log2 = 0`, where `bernoulli_pow2` short-circuits to `true`
    /// without consuming a draw, so on its own it never exercises the drawing
    /// path through to a flip.
    #[test]
    fn drawn_base_mutation_is_attributed_to_the_base_counter_and_flips_a_bit() {
        let tick = 9;
        let cell_index = 0;
        let seed = (0_u64..)
            .find(|seed| {
                let mut rng = cell_rng(*seed ^ MUTATION_SALT, tick, cell_index as u64);
                bernoulli_pow2(&mut rng, 1)
            })
            .expect("a firing base-mutation stream should exist");
        let mut cell = Cell::with_program(
            Program::new_live(vec![op::NOP], Direction::Up, 1).expect("program should build"),
        );
        cell.program
            .as_mut()
            .expect("program should exist")
            .tick
            .was_live_at_tick_start = true;
        let config = SimConfig {
            mutation_base_log2: 1,
            ..SimConfig::default()
        };

        assert_eq!(
            mutate_end_of_tick_cell(&mut cell, &config, tick, seed, cell_index),
            MutationCounts {
                base: 1,
                background: 0,
            }
        );
        assert_ne!(
            cell.program.expect("program should exist").code,
            vec![op::NOP],
            "a firing mutation should have flipped one bit of the program"
        );
    }

    #[test]
    fn background_stressed_mutation_routes_through_any_quantum_sampler() {
        let mut always_cell = Cell::with_program(
            Program::new_live(vec![op::NOP], Direction::Up, 1).expect("program should build"),
        );
        let always_program = always_cell.program.as_mut().expect("program should exist");
        always_program.tick.was_live_at_tick_start = true;
        always_program.tick.bg_radiation_consumed = 4;
        let always_config = SimConfig {
            mutation_base_log2: 63,
            mutation_background_log2: 0,
            ..SimConfig::default()
        };

        BACKGROUND_MUTATION_CALLS.with(|calls| calls.set(0));
        assert_eq!(
            mutate_end_of_tick_cell(&mut always_cell, &always_config, 9, 17, 0),
            MutationCounts {
                base: 0,
                background: 1,
            }
        );
        BACKGROUND_MUTATION_CALLS.with(|calls| assert_eq!(calls.get(), 1));
        assert_ne!(
            always_cell
                .program
                .as_ref()
                .expect("program should exist")
                .code,
            vec![op::NOP]
        );

        let (background_consumed, exponent, tick, cell_index) = (4_u32, 1_u32, 11_u64, 0_usize);
        let seed = (0_u64..)
            .find(|seed| {
                let mut rng = cell_rng(*seed ^ MUTATION_SALT, tick, cell_index as u64);
                let triggers = binomial_pow2(&mut rng, background_consumed, exponent);
                triggers > 0 && triggers < background_consumed
            })
            .expect("a partial-success trigger stream should exist");
        let mut partial_cell = Cell::with_program(
            Program::new_live(vec![op::NOP], Direction::Up, 2).expect("program should build"),
        );
        let partial_program = partial_cell.program.as_mut().expect("program should exist");
        partial_program.tick.was_live_at_tick_start = true;
        partial_program.tick.bg_radiation_consumed = background_consumed;
        let partial_config = SimConfig {
            mutation_base_log2: 63,
            mutation_background_log2: exponent,
            ..SimConfig::default()
        };

        assert_eq!(
            mutate_end_of_tick_cell(&mut partial_cell, &partial_config, tick, seed, cell_index),
            MutationCounts {
                base: 0,
                background: 1,
            }
        );
        assert_ne!(
            partial_cell
                .program
                .as_ref()
                .expect("program should exist")
                .code,
            vec![op::NOP]
        );
    }

    #[test]
    fn background_mutation_matches_any_quantum_probability_law() {
        const DRAWS: u32 = 200_000;
        let exponent = 8_u32;
        for consumed in [1_u32, 20, 25, 32] {
            let mut rng = cell_rng(0x4d55_5441, consumed as u64, exponent as u64);
            let fired = (0..DRAWS)
                .filter(|_| background_mutation_fires(&mut rng, consumed, exponent))
                .count() as f64;
            let actual = fired / f64::from(DRAWS);
            let per_quantum = 2_f64.powi(-(exponent as i32));
            let expected = 1.0 - (1.0 - per_quantum).powi(consumed as i32);
            assert!(
                (actual - expected).abs() < 0.002,
                "consumed={consumed}: actual {actual} differs from expected {expected}"
            );
        }
    }

    #[test]
    fn background_mutation_k_zero_always_fires_without_consuming_a_draw() {
        let mut rng = cell_rng(0x4d55_5441, 7, 11);
        let mut twin = rng.clone();

        assert!(background_mutation_fires(&mut rng, 32, 0));
        assert_eq!(rng.next_u64(), twin.next_u64());
    }

    /// Builds a config whose spawn draw always succeeds.
    fn always_spawn_config(width: u32, height: u32) -> SimConfig {
        SimConfig {
            width,
            height,
            p_spawn_log2: Some(0),
            ..SimConfig::default()
        }
    }

    #[test]
    fn spontaneous_spawn_is_a_lineage_root_tagged_with_its_creation_site() {
        let config = always_spawn_config(8, 8);
        let cell_count = config.cell_count().expect("config should size the grid");
        let (tick, seed, cell_index) = (42_u64, 0x1234_5678_9abc_def0_u64, 19_usize);

        let mut cell = Cell::default();
        let births =
            resolve_spontaneous_creation_cell(&mut cell, true, &config, tick, seed, cell_index);

        assert_eq!(births, 1);
        let lineage = cell
            .program
            .as_ref()
            .expect("spawn should place a program")
            .lineage;
        assert_eq!(lineage.parent, ProgramUid::NONE);
        assert_eq!(lineage.generation, 0);
        assert_eq!(lineage.birth_tick(), Some(tick as u32));
        assert_eq!(
            lineage.uid.site(cell_count),
            Some(ProgramSite {
                tick,
                cell_index,
                origin: ProgramOrigin::Spawn,
            })
        );
    }

    #[test]
    fn spontaneous_spawn_leaves_the_direction_and_id_draws_untouched() {
        let config = always_spawn_config(8, 8);
        let (tick, seed, cell_index) = (7_u64, 0xfeed_face_dead_beef_u64, 5_usize);

        // This fixture uses p_spawn_log2 = 0, so the exponent is 0 and
        // `bernoulli_pow2` short-circuits to true *without consuming a draw*.
        // The spawn path therefore advances the stream exactly twice here:
        // direction, then id. The k > 0 path, where the Bernoulli really does
        // consume a draw, is covered by the test below.
        let spawn_exponent = config.p_spawn_log2.expect("spawn should be enabled");
        assert_eq!(spawn_exponent, 0, "p_spawn_log2 = 0 is the k == 0 case");

        let mut twin = cell_rng(seed ^ SPAWN_SALT, tick, cell_index as u64);
        assert!(bernoulli_pow2(&mut twin, spawn_exponent));
        let expected_dir = Direction::ALL[(twin.next_u32() % Direction::ALL.len() as u32) as usize];
        let expected_id = twin.next_u32() as u8;
        let next_after_spawn = twin.next_u32();

        let mut cell = Cell::default();
        resolve_spontaneous_creation_cell(&mut cell, true, &config, tick, seed, cell_index);

        let program = cell.program.as_ref().expect("spawn should place a program");
        assert_eq!(program.registers.dir, expected_dir);
        assert_eq!(program.registers.id, expected_id);

        // A fresh twin must land on the same third `u32`, proving the spawn
        // path consumed exactly the two draws above and that lineage consumed
        // nothing after them.
        let mut replay = cell_rng(seed ^ SPAWN_SALT, tick, cell_index as u64);
        let _ = bernoulli_pow2(&mut replay, spawn_exponent);
        let _ = replay.next_u32();
        let _ = replay.next_u32();
        assert_eq!(replay.next_u32(), next_after_spawn);
    }

    #[test]
    fn spontaneous_spawn_mirrors_the_dyadic_sampler_on_the_drawing_path() {
        // p_spawn_log2 = 2 (probability 2^-2), so `bernoulli_pow2` consumes a real draw before the
        // direction and id draws. A regression that skipped or added that
        // Bernoulli draw only for k > 0 would shift every successful
        // spawn's direction and id while still passing the k == 0 test above.
        let config = SimConfig {
            width: 8,
            height: 8,
            p_spawn_log2: Some(2),
            ..SimConfig::default()
        };
        let cell_count = config.cell_count().expect("config should size the grid");
        let spawn_exponent = config.p_spawn_log2.expect("spawn should be enabled");
        assert_eq!(spawn_exponent, 2);

        let (tick, seed) = (11_u64, 0x0bad_c0de_dead_10cc_u64);
        let (mut fired, mut skipped) = (0_u32, 0_u32);

        for cell_index in 0..cell_count {
            // Twin mirrors production draw for draw: Bernoulli first, and only
            // on success the direction and id draws.
            let mut twin = cell_rng(seed ^ SPAWN_SALT, tick, cell_index as u64);
            let expect_spawn = bernoulli_pow2(&mut twin, spawn_exponent);

            let mut cell = Cell::default();
            let births =
                resolve_spontaneous_creation_cell(&mut cell, true, &config, tick, seed, cell_index);

            if !expect_spawn {
                skipped += 1;
                assert_eq!(births, 0, "cell {cell_index} should not have spawned");
                assert!(
                    cell.program.is_none(),
                    "cell {cell_index} should still be empty"
                );
                continue;
            }

            fired += 1;
            assert_eq!(births, 1, "cell {cell_index} should have spawned");
            let expected_dir =
                Direction::ALL[(twin.next_u32() % Direction::ALL.len() as u32) as usize];
            let expected_id = twin.next_u32() as u8;
            let next_after_spawn = twin.next_u32();

            let program = cell
                .program
                .as_ref()
                .expect("a fired spawn should place a program");
            assert_eq!(
                program.registers.dir, expected_dir,
                "direction diverged at cell {cell_index}"
            );
            assert_eq!(
                program.registers.id, expected_id,
                "id diverged at cell {cell_index}"
            );

            // Lineage is attached after the draws and must consume none of them.
            let mut replay = cell_rng(seed ^ SPAWN_SALT, tick, cell_index as u64);
            assert!(bernoulli_pow2(&mut replay, spawn_exponent));
            let _ = replay.next_u32();
            let _ = replay.next_u32();
            assert_eq!(
                replay.next_u32(),
                next_after_spawn,
                "spawn consumed an unexpected number of draws at cell {cell_index}"
            );

            assert_eq!(program.lineage.parent, ProgramUid::NONE);
            assert_eq!(program.lineage.generation, 0);
            assert_eq!(program.lineage.birth_tick(), Some(tick as u32));
            assert_eq!(
                program.lineage.uid.site(cell_count),
                Some(ProgramSite {
                    tick,
                    cell_index,
                    origin: ProgramOrigin::Spawn,
                })
            );
        }

        // Both branches of the Bernoulli must actually be exercised, or the
        // loop proves nothing about the path it never took.
        assert!(fired > 0, "no cell spawned; the fixture exercises nothing");
        assert!(
            skipped > 0,
            "every cell spawned; the non-spawning branch is untested"
        );
    }

    #[test]
    fn spawn_skips_occupied_cells_and_leaves_their_lineage_alone() {
        let config = always_spawn_config(8, 8);
        let existing = Lineage::root(ProgramUid(97), 3);
        let mut cell = Cell::with_program(
            Program::new_live(vec![op::NOP], Direction::Up, 2)
                .expect("program should build")
                .with_lineage(existing),
        );

        let births = resolve_spontaneous_creation_cell(&mut cell, true, &config, 9, 11, 1);

        assert_eq!(births, 0);
        assert_eq!(
            cell.program.as_ref().expect("program should exist").lineage,
            existing
        );
    }

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
            d_energy_log2: None,
            r_energy: 0.0,
            d_mass_log2: None,
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
