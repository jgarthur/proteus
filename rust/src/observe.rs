//! Builds read-only projections and compact encodings for external observers.

use std::collections::{HashMap, HashSet};

use crate::grid::Grid;
use crate::model::{Cell, Program, ProgramOrigin, ProgramUid};
use crate::opcode::Opcode;
use crate::simulation::TickReport;

/// Buckets program sizes 1..=256 before overflowing.
pub const CENSUS_SIZE_BUCKETS: usize = 256;
/// Counts one bucket per possible instruction byte.
pub const CENSUS_OPCODE_BUCKETS: usize = 256;
/// Buckets lineage generations 0..=511 before overflowing.
///
/// Widened from the originally specified 32 after measurement: on a grown
/// `web-256x256` ecology at tick 8000 the live generation distribution was
/// p50 186 / p99 303 / max 319, which put 97.8% of the population in the
/// overflow bin. Generation grows roughly linearly with tick count, so a very
/// long run will overflow this too; `count`, `sum`, and `max` stay exact when
/// it does.
pub const CENSUS_GENERATION_BUCKETS: usize = 512;
/// Buckets offspring counts 0..=15 before overflowing.
///
/// Measured max on the same fixture was 4, so 16 buckets is ample.
pub const CENSUS_OFFSPRING_BUCKETS: usize = 16;
/// Buckets stack depths on a log2 scale: one zero bucket plus one per power of
/// two, which covers every `u32` in 33 buckets.
///
/// Stack depth is the one census distribution that no linear width describes.
/// Measured on `web-256x256` at tick 8000 it was p50 759 / p90 8315 / max 32767
/// (the stack cap), spanning the entire range, so a linear histogram would need
/// 32 768 buckets per metrics row to keep its overflow bin near-empty. On a
/// log2 scale the same distribution fits in 33 numbers with no overflow at all.
pub const CENSUS_STACK_BUCKETS: usize = 33;

/// Accumulates event counts across every completed tick in one observation epoch.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventTotals {
    pub births: u64,
    pub boot_births: u64,
    pub spawn_births: u64,
    pub deaths: u64,
    pub mutations: u64,
}

impl EventTotals {
    /// Records exactly one completed tick's event deltas.
    pub fn record(&mut self, report: TickReport) {
        self.births = self
            .births
            .checked_add(u64::from(report.births))
            .expect("cumulative birth count should fit in u64");
        self.boot_births = self
            .boot_births
            .checked_add(u64::from(report.boot_births))
            .expect("cumulative boot-birth count should fit in u64");
        self.spawn_births = self
            .spawn_births
            .checked_add(u64::from(report.spawn_births))
            .expect("cumulative spawn-birth count should fit in u64");
        self.deaths = self
            .deaths
            .checked_add(u64::from(report.deaths))
            .expect("cumulative death count should fit in u64");
        self.mutations = self
            .mutations
            .checked_add(u64::from(report.mutations))
            .expect("cumulative mutation count should fit in u64");
    }
}

/// Summarizes the simulation state into observer-facing aggregate metrics.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MetricsSnapshot {
    pub epoch: u64,
    pub tick: u64,
    pub population: u32,
    pub live_count: u32,
    pub inert_count: u32,
    pub total_energy: u64,
    pub packet_energy: u64,
    pub total_mass: u64,
    pub mean_program_size: f64,
    pub max_program_size: u32,
    pub unique_genomes: u32,
    pub births: u32,
    pub boot_births: u32,
    pub spawn_births: u32,
    pub deaths: u32,
    pub mutations: u32,
    pub event_totals: EventTotals,
    /// Point-in-time program census, present only where it was asked for.
    ///
    /// `Some` in every runner metrics row; `None` in the WebSocket stream and in
    /// `GET /v1/sim/metrics` without `?census=1`. `default` keeps metrics rows
    /// written before this field readable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub census: Option<ProgramCensus>,
}

/// Names how a [`BucketHistogram`] maps observed values onto bucket indices.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HistogramScale {
    /// One bucket per consecutive integer starting at `first_value`.
    #[default]
    Linear,
    /// Bucket 0 counts the value 0; bucket `i >= 1` counts values `v` with
    /// `floor(log2(v)) == i - 1`, i.e. `v` in `[2^(i-1), 2^i)`.
    Log2,
}

/// Counts observations into buckets with one overflow bin.
///
/// On the [`Linear`](HistogramScale::Linear) scale, `counts[i]` counts the value
/// `first_value + i` and `overflow` counts every value above
/// `first_value + counts.len() - 1`.
///
/// On the [`Log2`](HistogramScale::Log2) scale, `counts[0]` counts the value 0
/// and `counts[i]` counts `[2^(i-1), 2^i)`. The 33 buckets cover every `u32`, so
/// `overflow` is always 0 there; the field is kept so both scales share one wire
/// shape. `first_value` is unused on this scale and stays 0.
///
/// `count`, `sum`, and `max` are exact over all observations on either scale,
/// including overflowing ones.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BucketHistogram {
    /// Defaults to [`HistogramScale::Linear`] when absent, so metrics rows
    /// written before this field existed still describe themselves correctly.
    #[serde(default)]
    pub scale: HistogramScale,
    pub first_value: u32,
    pub counts: Vec<u32>,
    pub overflow: u32,
    pub count: u32,
    pub sum: u64,
    pub max: u32,
}

impl BucketHistogram {
    /// Builds an empty linear histogram over `buckets` values from `first_value`.
    pub fn new(first_value: u32, buckets: usize) -> Self {
        Self {
            scale: HistogramScale::Linear,
            first_value,
            counts: vec![0; buckets],
            overflow: 0,
            count: 0,
            sum: 0,
            max: 0,
        }
    }

    /// Builds an empty log2 histogram covering every `u32` in 33 buckets.
    pub fn log2() -> Self {
        Self {
            scale: HistogramScale::Log2,
            first_value: 0,
            counts: vec![0; CENSUS_STACK_BUCKETS],
            overflow: 0,
            count: 0,
            sum: 0,
            max: 0,
        }
    }

    /// Records one observation.
    ///
    /// On the linear scale, values below `first_value` cannot occur for any
    /// census histogram (sizes start at 1, every other one starts at 0), so they
    /// are folded into the first bucket rather than given a second overflow bin.
    pub fn record(&mut self, value: u32) {
        self.count = self.count.saturating_add(1);
        self.sum = self.sum.saturating_add(u64::from(value));
        self.max = self.max.max(value);

        let index = match self.scale {
            HistogramScale::Linear => {
                debug_assert!(
                    value >= self.first_value,
                    "linear census histograms never observe values below first_value"
                );
                value.saturating_sub(self.first_value) as usize
            }
            HistogramScale::Log2 => log2_bucket(value),
        };

        match self.counts.get_mut(index) {
            Some(bucket) => *bucket = bucket.saturating_add(1),
            None => self.overflow = self.overflow.saturating_add(1),
        }
    }

    /// Returns the exact mean over every observation, or `0.0` when empty.
    ///
    /// Exact on either scale: it reads `sum` and `count`, never the buckets.
    pub fn mean(&self) -> f64 {
        if self.count == 0 {
            0.0
        } else {
            self.sum as f64 / f64::from(self.count)
        }
    }
}

/// Maps one value onto its log2 bucket index.
///
/// Bucket 0 holds the value 0; bucket `i >= 1` holds `[2^(i-1), 2^i)`. The
/// largest `u32` lands in bucket 32, so 33 buckets cover the whole type and a
/// log2 histogram can never overflow.
fn log2_bucket(value: u32) -> usize {
    if value == 0 {
        0
    } else {
        (u32::BITS - value.leading_zeros()) as usize
    }
}

/// Describes the size-1 population that decides milestone M1.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Size1Census {
    pub live_count: u32,
    pub inert_count: u32,
    pub live_age_sum: u64,
    pub live_max_age: u32,
    /// `counts[byte]` = live size-1 programs whose only instruction is `byte`.
    pub live_opcode_counts: Vec<u32>,
}

/// Aggregates per-program lineage over the live population.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LineageCensus {
    /// Live programs with no parent: seed programs and spontaneous spawns.
    pub roots: u32,
    /// Live programs whose parent is not currently live.
    pub orphans: u32,
    pub generation: BucketHistogram,
    pub birth_tick_sum: u64,
    pub birth_tick_count: u32,
    /// Over live programs, bucketed by how many currently-live programs name
    /// them as parent. `count` equals the live population.
    pub offspring: BucketHistogram,
    pub origin_seed: u32,
    pub origin_spawn: u32,
    pub origin_append: u32,
}

/// Point-in-time census of the program population. Not a cumulative counter:
/// never diff these fields across samples the way [`EventTotals`] are diffed.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProgramCensus {
    pub live_sizes: BucketHistogram,
    pub inert_sizes: BucketHistogram,
    /// `counts[byte]` = instruction bytes equal to `byte` across live programs.
    pub opcode_counts: Vec<u64>,
    pub size1: Size1Census,
    pub lineage: LineageCensus,
    /// Stack depth over live and inert programs, so `count` equals `population`.
    /// Bucketed on a log2 scale; see [`HistogramScale`].
    pub stack_depths: BucketHistogram,
}

impl ProgramCensus {
    /// Builds an all-zero census with every histogram sized to its constants.
    fn empty() -> Self {
        Self {
            live_sizes: BucketHistogram::new(1, CENSUS_SIZE_BUCKETS),
            inert_sizes: BucketHistogram::new(1, CENSUS_SIZE_BUCKETS),
            opcode_counts: vec![0; CENSUS_OPCODE_BUCKETS],
            size1: Size1Census {
                live_opcode_counts: vec![0; CENSUS_OPCODE_BUCKETS],
                ..Size1Census::default()
            },
            lineage: LineageCensus {
                generation: BucketHistogram::new(0, CENSUS_GENERATION_BUCKETS),
                offspring: BucketHistogram::new(0, CENSUS_OFFSPRING_BUCKETS),
                ..LineageCensus::default()
            },
            stack_depths: BucketHistogram::log2(),
        }
    }
}

/// Describes one cell in a human-readable inspection response.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct CellInspection {
    pub index: usize,
    pub x: u32,
    pub y: u32,
    pub free_energy: u32,
    pub free_mass: u32,
    pub bg_radiation: u32,
    pub bg_mass: u32,
    pub program: Option<ProgramInspection>,
}

/// Describes one program in a human-readable inspection response.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct ProgramInspection {
    pub code: Vec<u8>,
    pub disassembly: Vec<String>,
    pub size: u16,
    pub live: bool,
    pub age: u32,
    pub ip: u16,
    pub src: u16,
    pub dst: u16,
    pub dir: u8,
    pub flag: bool,
    pub msg: i16,
    pub id: u8,
    pub lc: i16,
    pub stack: Vec<i16>,
    pub abandonment_timer: Option<u32>,
    pub uid: u64,
    /// `None` when the program is a lineage root (seed program or spawn).
    pub parent_uid: Option<u64>,
    /// `None` while the program has never been live.
    pub birth_tick: Option<u32>,
    pub generation: u32,
    /// `"seed"`, `"spawn"`, or `"append"`, decoded from the uid.
    pub origin: Option<&'static str>,
    /// Tick at which the program was first created, decoded from the uid.
    pub created_tick: Option<u64>,
}

/// Computes one metrics snapshot from current state, latest events, and cumulative events.
pub fn collect_metrics(
    grid: &Grid,
    epoch: u64,
    tick: u64,
    report: TickReport,
    event_totals: EventTotals,
) -> MetricsSnapshot {
    let mut live_count = 0_u32;
    let mut inert_count = 0_u32;
    let mut total_energy = 0_u64;
    let mut total_mass = 0_u64;
    let mut live_program_size_sum = 0_u64;
    let mut max_program_size = 0_u32;
    let mut genomes = HashSet::<&[u8]>::new();

    for cell in grid.cells() {
        total_energy += u64::from(cell.free_energy) + u64::from(cell.bg_radiation);
        total_mass += u64::from(cell.free_mass) + u64::from(cell.bg_mass);

        let Some(program) = cell.program.as_ref() else {
            continue;
        };

        let size = u64::from(program.size());
        total_mass += size;

        if program.live {
            live_count += 1;
            live_program_size_sum += size;
            max_program_size = max_program_size.max(size as u32);
            let _ = genomes.insert(program.code.as_slice());
        } else {
            inert_count += 1;
        }
    }

    let mean_program_size = if live_count == 0 {
        0.0
    } else {
        live_program_size_sum as f64 / f64::from(live_count)
    };

    MetricsSnapshot {
        epoch,
        tick,
        population: live_count + inert_count,
        live_count,
        inert_count,
        // API-SPEC §10 currently defines totals over cell-local pools only.
        total_energy,
        // Each in-flight packet carries exactly 1 energy (SPEC §Physics).
        packet_energy: u64::from(report.packet_count),
        total_mass,
        mean_program_size,
        max_program_size,
        unique_genomes: genomes.len() as u32,
        births: report.births,
        boot_births: report.boot_births,
        spawn_births: report.spawn_births,
        deaths: report.deaths,
        mutations: report.mutations,
        event_totals,
        census: None,
    }
}

/// Computes a metrics snapshot with the point-in-time census attached.
pub fn collect_metrics_with_census(
    grid: &Grid,
    epoch: u64,
    tick: u64,
    report: TickReport,
    event_totals: EventTotals,
) -> MetricsSnapshot {
    MetricsSnapshot {
        census: Some(collect_census(grid)),
        ..collect_metrics(grid, epoch, tick, report, event_totals)
    }
}

/// Computes a point-in-time program census.
///
/// Serial in both feature configurations: this is off the tick path, so a
/// serial implementation is trivially parity-safe and costs nothing to verify.
/// Cost is O(live code bytes), the same order as the genome hashing
/// [`collect_metrics`] already performs on every call.
pub fn collect_census(grid: &Grid) -> ProgramCensus {
    let mut census = ProgramCensus::empty();
    let mut live_uids = HashSet::<ProgramUid>::new();
    let mut children = HashMap::<ProgramUid, u32>::new();

    for cell in grid.cells() {
        let Some(program) = cell.program.as_ref() else {
            continue;
        };

        let size = u32::from(program.size());
        census
            .stack_depths
            .record(u32::try_from(program.stack.len()).unwrap_or(u32::MAX));

        if !program.live {
            census.inert_sizes.record(size);
            if size == 1 {
                census.size1.inert_count = census.size1.inert_count.saturating_add(1);
            }
            continue;
        }

        census.live_sizes.record(size);
        for byte in &program.code {
            census.opcode_counts[usize::from(*byte)] += 1;
        }

        if size == 1 {
            census.size1.live_count = census.size1.live_count.saturating_add(1);
            census.size1.live_age_sum = census
                .size1
                .live_age_sum
                .saturating_add(u64::from(program.age));
            census.size1.live_max_age = census.size1.live_max_age.max(program.age);
            let opcode = usize::from(program.code[0]);
            census.size1.live_opcode_counts[opcode] =
                census.size1.live_opcode_counts[opcode].saturating_add(1);
        }

        let lineage = program.lineage;
        let _ = live_uids.insert(lineage.uid);
        census.lineage.generation.record(lineage.generation);

        if let Some(birth_tick) = lineage.birth_tick() {
            census.lineage.birth_tick_sum = census
                .lineage
                .birth_tick_sum
                .saturating_add(u64::from(birth_tick));
            census.lineage.birth_tick_count = census.lineage.birth_tick_count.saturating_add(1);
        }

        if lineage.parent.is_none() {
            census.lineage.roots = census.lineage.roots.saturating_add(1);
        } else {
            *children.entry(lineage.parent).or_insert(0) += 1;
        }

        // The origin tag lives in the low bits of the uid, so it decodes
        // without the grid size. Uids are NONE only for programs that never
        // passed through a Simulation, which count toward no origin.
        match lineage.uid.origin() {
            Some(ProgramOrigin::Seed) => {
                census.lineage.origin_seed = census.lineage.origin_seed.saturating_add(1);
            }
            Some(ProgramOrigin::Spawn) => {
                census.lineage.origin_spawn = census.lineage.origin_spawn.saturating_add(1);
            }
            Some(ProgramOrigin::Append) => {
                census.lineage.origin_append = census.lineage.origin_append.saturating_add(1);
            }
            None => {}
        }
    }

    // Second pass: offspring counts and orphan detection both need the complete
    // live set, so they cannot be folded into the first traversal.
    for cell in grid.cells() {
        let Some(program) = cell.program.as_ref().filter(|program| program.live) else {
            continue;
        };
        let lineage = program.lineage;

        census
            .lineage
            .offspring
            .record(children.get(&lineage.uid).copied().unwrap_or(0));

        if !lineage.parent.is_none() && !live_uids.contains(&lineage.parent) {
            census.lineage.orphans = census.lineage.orphans.saturating_add(1);
        }
    }

    census
}

/// Encodes the current grid into the compact binary frame used by observers.
pub fn encode_grid_frame(grid: &Grid, tick: u64) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(16 + (grid.len() * 8));
    bytes.extend_from_slice(&tick.to_le_bytes());
    bytes.extend_from_slice(&grid.width().to_le_bytes());
    bytes.extend_from_slice(&grid.height().to_le_bytes());

    for cell in grid.cells() {
        let (flags, program_id, program_size) = cell_view_program_fields(cell);
        bytes.push(flags);
        bytes.push(program_id);
        bytes.push(program_size);
        bytes.push(clamp_to_u8(cell.free_energy));
        bytes.push(clamp_to_u8(cell.free_mass));
        bytes.push(clamp_to_u8(cell.bg_radiation));
        bytes.push(clamp_to_u8(cell.bg_mass));
        bytes.push(0);
    }

    bytes
}

/// Returns an inspection view for one cell by flat index.
pub fn inspect_cell(grid: &Grid, index: usize) -> CellInspection {
    let cell = grid.get(index).expect("cell index must be in bounds");

    CellInspection {
        index,
        x: grid.x(index),
        y: grid.y(index),
        free_energy: cell.free_energy,
        free_mass: cell.free_mass,
        bg_radiation: cell.bg_radiation,
        bg_mass: cell.bg_mass,
        program: cell
            .program
            .as_ref()
            .map(|program| program_inspection(program, grid.len())),
    }
}

/// Returns inspection views for a rectangular region of cells.
pub fn inspect_region(grid: &Grid, x: u32, y: u32, w: u32, h: u32) -> Vec<CellInspection> {
    let mut cells = Vec::with_capacity((w * h) as usize);

    for row in y..(y + h) {
        for column in x..(x + w) {
            let index = grid.index(column, row);
            cells.push(inspect_cell(grid, index));
        }
    }

    cells
}

/// Converts raw bytecode into the API-facing mnemonic strings.
pub fn disassemble(code: &[u8]) -> Vec<String> {
    code.iter()
        .map(|byte| Opcode::decode(*byte).to_string())
        .collect()
}

/// Extracts the compact frame fields that describe program occupancy.
fn cell_view_program_fields(cell: &Cell) -> (u8, u8, u8) {
    match cell.program.as_ref() {
        Some(program) => {
            let mut flags = 0b001;
            if program.live {
                flags |= 0b010;
            }
            if program.tick.is_open {
                flags |= 0b100;
            }

            (flags, program.registers.id, encode_program_size(program.size()))
        }
        None => (0b100, 0, 0),
    }
}

/// Encodes a program size into the log2-scaled `CellView` byte (API-SPEC Â§11).
///
/// The byte advances 17 steps per doubling: `round(17 * log2(size))`, clamped to
/// 255. This keeps dynamic range for the small programs that dominate early
/// simulation (a linear `size / 128` byte was 0 for every program under 128
/// instructions) while still separating the largest programs.
///
/// Purely an observation encoding: it reads program state and never touches the
/// RNG or any draw path.
fn encode_program_size(size: u16) -> u8 {
    if size == 0 {
        return 0;
    }

    let scaled = (17.0 * f64::from(size).log2()).round();
    scaled.clamp(0.0, 255.0) as u8
}

/// Clamps a resource count into the byte-sized frame encoding.
fn clamp_to_u8(value: u32) -> u8 {
    value.min(u32::from(u8::MAX)) as u8
}

/// Converts an internal program into the API inspection shape.
///
/// `cell_count` is the grid size, needed to decode the creation site out of the
/// program's uid.
fn program_inspection(program: &Program, cell_count: usize) -> ProgramInspection {
    let lineage = program.lineage;
    let site = lineage.uid.site(cell_count);

    ProgramInspection {
        code: program.code.clone(),
        disassembly: disassemble(&program.code),
        size: program.size(),
        live: program.live,
        age: program.age,
        ip: program.registers.ip,
        src: program.registers.src,
        dst: program.registers.dst,
        dir: program.registers.dir as u8,
        flag: program.registers.flag,
        msg: program.registers.msg,
        id: program.registers.id,
        lc: program.registers.lc,
        stack: program.stack.clone(),
        abandonment_timer: if program.live {
            None
        } else {
            Some(program.abandonment_timer)
        },
        uid: lineage.uid.get(),
        parent_uid: (!lineage.parent.is_none()).then(|| lineage.parent.get()),
        birth_tick: lineage.birth_tick(),
        generation: lineage.generation,
        origin: site.map(|site| site.origin.label()),
        created_tick: site.map(|site| site.tick),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        collect_census, collect_metrics, collect_metrics_with_census, disassemble,
        encode_grid_frame, encode_program_size, inspect_cell, BucketHistogram, EventTotals,
        HistogramScale, MetricsSnapshot, CENSUS_OPCODE_BUCKETS, CENSUS_SIZE_BUCKETS,
        CENSUS_STACK_BUCKETS,
    };
    use crate::model::{Cell, Direction, Lineage, Program, ProgramOrigin, ProgramUid};
    use crate::opcode::op;
    use crate::simulation::TickReport;
    use crate::Grid;

    /// Builds a live program carrying explicit lineage.
    fn live(code: Vec<u8>, lineage: Lineage) -> Program {
        Program::new_live(code, Direction::Right, 1)
            .expect("program should build")
            .with_lineage(lineage)
    }

    /// Builds an inert program carrying explicit lineage.
    fn inert(code: Vec<u8>, lineage: Lineage) -> Program {
        Program::new_inert(code, Direction::Right, 1)
            .expect("program should build")
            .with_lineage(lineage)
    }

    /// Wraps programs into a one-row grid, padding with empty cells.
    fn grid_of(programs: Vec<Option<Program>>) -> Grid {
        let width = programs.len() as u32;
        let cells = programs
            .into_iter()
            .map(|program| Cell {
                program,
                ..Cell::default()
            })
            .collect();
        Grid::from_cells(width, 1, cells).expect("grid should build")
    }

    /// Returns the non-zero buckets of a histogram as (bucket index, count).
    ///
    /// Used for log2 histograms, where a bucket index names a range rather than
    /// a single value.
    fn occupied_indices(histogram: &BucketHistogram) -> Vec<(usize, u32)> {
        histogram
            .counts
            .iter()
            .enumerate()
            .filter(|(_, count)| **count > 0)
            .map(|(index, count)| (index, *count))
            .collect()
    }

    /// Returns the non-zero buckets of a histogram as (value, count) pairs.
    fn occupied(histogram: &BucketHistogram) -> Vec<(u32, u32)> {
        histogram
            .counts
            .iter()
            .enumerate()
            .filter(|(_, count)| **count > 0)
            .map(|(index, count)| (histogram.first_value + index as u32, *count))
            .collect()
    }

    #[test]
    fn log2_histogram_places_edge_values_in_the_documented_buckets() {
        // Bucket 0 is the value 0; bucket i covers [2^(i-1), 2^i).
        let cases: [(u32, usize); 14] = [
            (0, 0),
            (1, 1),
            (2, 2),
            (3, 2),
            (4, 3),
            (5, 3),
            (7, 3),
            (8, 4),
            (15, 4),
            (16, 5),
            (1_023, 10),
            (1_024, 11),
            (2_147_483_648, 32),
            (u32::MAX, 32),
        ];

        for (value, expected_bucket) in cases {
            let mut histogram = BucketHistogram::log2();
            histogram.record(value);

            assert_eq!(
                occupied_indices(&histogram),
                vec![(expected_bucket, 1)],
                "value {value} should land in log2 bucket {expected_bucket}"
            );
            assert_eq!(histogram.overflow, 0, "value {value} must not overflow");
            assert_eq!(histogram.count, 1);
            assert_eq!(histogram.sum, u64::from(value));
            assert_eq!(histogram.max, value);
        }
    }

    #[test]
    fn log2_histogram_keeps_exact_totals_across_many_magnitudes() {
        let mut histogram = BucketHistogram::log2();
        let values = [0_u32, 1, 1, 5, 5, 5, 300, 70_000, u32::MAX];
        for value in values {
            histogram.record(value);
        }

        // Powers-of-two boundaries: 5 -> [4,8) = 3, 300 -> [256,512) = 9,
        // 70000 -> [65536,131072) = 17, u32::MAX -> [2^31, 2^32) = 32.
        assert_eq!(
            occupied_indices(&histogram),
            vec![(0, 1), (1, 2), (3, 3), (9, 1), (17, 1), (32, 1)]
        );
        assert_eq!(histogram.count, values.len() as u32);
        assert_eq!(
            histogram.sum,
            values.iter().map(|value| u64::from(*value)).sum::<u64>()
        );
        assert_eq!(histogram.max, u32::MAX);
        assert_eq!(histogram.overflow, 0);
        assert_eq!(
            histogram.counts.iter().sum::<u32>(),
            histogram.count,
            "every observation is bucketed, none is lost"
        );
    }

    #[test]
    fn log2_histogram_round_trips_and_legacy_rows_default_to_linear() {
        let mut histogram = BucketHistogram::log2();
        histogram.record(9);

        let json = serde_json::to_string(&histogram).expect("histogram should serialize");
        assert!(
            json.contains("\"scale\":\"log2\""),
            "scale must always be written: {json}"
        );
        let restored: BucketHistogram =
            serde_json::from_str(&json).expect("histogram should deserialize");
        assert_eq!(restored, histogram);

        // A histogram written before `scale` existed is linear by definition.
        let legacy = serde_json::json!({
            "first_value": 1, "counts": [0, 2, 1], "overflow": 0,
            "count": 3, "sum": 7, "max": 3
        });
        let legacy: BucketHistogram =
            serde_json::from_value(legacy).expect("legacy histograms should deserialize");
        assert_eq!(legacy.scale, HistogramScale::Linear);
        assert_eq!(legacy.first_value, 1);
    }

    #[test]
    fn linear_histograms_are_unchanged_by_the_scale_field() {
        let mut histogram = BucketHistogram::new(1, 4);
        for value in [1_u32, 2, 2, 9] {
            histogram.record(value);
        }

        assert_eq!(histogram.scale, HistogramScale::Linear);
        assert_eq!(occupied(&histogram), vec![(1, 1), (2, 2)]);
        assert_eq!(histogram.overflow, 1, "9 is past the last linear bucket");
        assert_eq!(histogram.count, 4);
        assert_eq!(histogram.sum, 14);
        assert_eq!(histogram.max, 9);
    }

    #[test]
    fn collect_metrics_leaves_the_census_absent_and_off_the_wire() {
        let grid = grid_of(vec![Some(live(vec![op::NOP], Lineage::default()))]);
        let metrics = collect_metrics(&grid, 0, 1, TickReport::default(), EventTotals::default());

        assert_eq!(metrics.census, None);
        let json = serde_json::to_string(&metrics).expect("metrics should serialize");
        assert!(
            !json.contains("census"),
            "the census key must not appear when absent: {json}"
        );
    }

    #[test]
    fn collect_metrics_with_census_describes_the_same_snapshot() {
        let grid = grid_of(vec![Some(live(
            vec![op::NOP, op::ABSORB],
            Lineage::default(),
        ))]);
        let plain = collect_metrics(&grid, 2, 3, TickReport::default(), EventTotals::default());
        let with_census =
            collect_metrics_with_census(&grid, 2, 3, TickReport::default(), EventTotals::default());

        assert_eq!(with_census.census, Some(collect_census(&grid)));
        assert_eq!(
            MetricsSnapshot {
                census: None,
                ..with_census
            },
            plain,
            "attaching a census must not disturb any other field"
        );
    }

    #[test]
    fn census_size_histograms_split_live_from_inert_and_overflow_past_the_cap() {
        let big = vec![op::NOP; CENSUS_SIZE_BUCKETS + 44];
        let grid = grid_of(vec![
            Some(live(vec![op::NOP], Lineage::default())),
            Some(live(vec![op::NOP, op::ABSORB], Lineage::default())),
            Some(live(vec![op::NOP, op::ABSORB], Lineage::default())),
            Some(live(big.clone(), Lineage::default())),
            Some(inert(vec![op::NOP, op::NOP, op::NOP], Lineage::default())),
            None,
        ]);

        let census = collect_census(&grid);

        assert_eq!(census.live_sizes.first_value, 1);
        assert_eq!(occupied(&census.live_sizes), vec![(1, 1), (2, 2)]);
        assert_eq!(
            census.live_sizes.overflow, 1,
            "the 300-byte program overflows"
        );
        assert_eq!(census.live_sizes.count, 4);
        assert_eq!(census.live_sizes.sum, 1 + 2 + 2 + big.len() as u64);
        assert_eq!(census.live_sizes.max, big.len() as u32);

        assert_eq!(census.inert_sizes.first_value, 1);
        assert_eq!(occupied(&census.inert_sizes), vec![(3, 1)]);
        assert_eq!(census.inert_sizes.overflow, 0);
        assert_eq!(census.inert_sizes.count, 1);
        assert_eq!(census.inert_sizes.sum, 3);
        assert_eq!(census.inert_sizes.max, 3);
    }

    #[test]
    fn census_opcode_counts_cover_live_code_only() {
        let grid = grid_of(vec![
            Some(live(vec![op::NOP, op::ABSORB, op::NOP], Lineage::default())),
            Some(live(vec![op::ABSORB], Lineage::default())),
            Some(inert(vec![op::NOP, op::NOP], Lineage::default())),
        ]);

        let census = collect_census(&grid);

        assert_eq!(census.opcode_counts.len(), CENSUS_OPCODE_BUCKETS);
        assert_eq!(census.opcode_counts[usize::from(op::NOP)], 2);
        assert_eq!(census.opcode_counts[usize::from(op::ABSORB)], 2);
        assert_eq!(
            census.opcode_counts.iter().sum::<u64>(),
            4,
            "inert bytes must not be counted"
        );
    }

    #[test]
    fn census_size1_tracks_live_ages_and_the_single_opcode() {
        let mut old = live(vec![op::ABSORB], Lineage::default());
        old.age = 900;
        let mut young = live(vec![op::ABSORB], Lineage::default());
        young.age = 20;

        let grid = grid_of(vec![
            Some(old),
            Some(young),
            Some(live(vec![op::NOP], Lineage::default())),
            Some(inert(vec![op::NOP], Lineage::default())),
            Some(live(vec![op::NOP, op::NOP], Lineage::default())),
        ]);

        let census = collect_census(&grid);

        assert_eq!(census.size1.live_count, 3);
        assert_eq!(census.size1.inert_count, 1);
        assert_eq!(census.size1.live_age_sum, 920);
        assert_eq!(census.size1.live_max_age, 900);
        assert_eq!(census.size1.live_opcode_counts[usize::from(op::ABSORB)], 2);
        assert_eq!(census.size1.live_opcode_counts[usize::from(op::NOP)], 1);
        assert_eq!(
            census.size1.live_opcode_counts.iter().sum::<u32>(),
            3,
            "the inert size-1 body contributes no opcode"
        );
    }

    #[test]
    fn census_lineage_counts_roots_orphans_offspring_and_origins() {
        let cell_count = 8;
        let seed_uid = ProgramUid::create(0, 0, cell_count, ProgramOrigin::Seed);
        let spawn_uid = ProgramUid::create(3, 1, cell_count, ProgramOrigin::Spawn);
        let missing_parent = ProgramUid::create(1, 7, cell_count, ProgramOrigin::Append);

        // Two live children of the seed, one live orphan whose parent is gone.
        let grid = grid_of(vec![
            Some(live(vec![op::NOP], Lineage::root(seed_uid, 0))),
            Some(live(vec![op::NOP], Lineage::root(spawn_uid, 3))),
            Some(live(
                vec![op::NOP],
                Lineage {
                    uid: ProgramUid::create(5, 2, cell_count, ProgramOrigin::Append),
                    parent: seed_uid,
                    birth_tick: 6,
                    generation: 1,
                },
            )),
            Some(live(
                vec![op::NOP],
                Lineage {
                    uid: ProgramUid::create(5, 3, cell_count, ProgramOrigin::Append),
                    parent: seed_uid,
                    birth_tick: 7,
                    generation: 1,
                },
            )),
            Some(live(
                vec![op::NOP],
                Lineage {
                    uid: ProgramUid::create(5, 4, cell_count, ProgramOrigin::Append),
                    parent: missing_parent,
                    birth_tick: 8,
                    generation: 9,
                },
            )),
            // An inert body contributes to no lineage aggregate.
            Some(inert(
                vec![op::NOP],
                Lineage::child(
                    ProgramUid::create(6, 5, cell_count, ProgramOrigin::Append),
                    seed_uid,
                    1,
                ),
            )),
        ]);

        let census = collect_census(&grid);
        let lineage = &census.lineage;

        assert_eq!(lineage.roots, 2, "the seed and the spawn have no parent");
        assert_eq!(lineage.orphans, 1);
        assert_eq!(lineage.origin_seed, 1);
        assert_eq!(lineage.origin_spawn, 1);
        assert_eq!(lineage.origin_append, 3);

        assert_eq!(lineage.generation.first_value, 0);
        assert_eq!(occupied(&lineage.generation), vec![(0, 2), (1, 2), (9, 1)]);
        assert_eq!(lineage.generation.count, 5, "live programs only");
        assert_eq!(lineage.generation.max, 9);

        assert_eq!(lineage.birth_tick_count, 5);
        let expected_birth_ticks: u64 = [0, 3, 6, 7, 8].into_iter().sum();
        assert_eq!(lineage.birth_tick_sum, expected_birth_ticks);

        // The seed has two live children; every other live program has none.
        assert_eq!(lineage.offspring.first_value, 0);
        assert_eq!(occupied(&lineage.offspring), vec![(0, 4), (2, 1)]);
        assert_eq!(lineage.offspring.count, 5);
        assert_eq!(lineage.offspring.max, 2);
    }

    #[test]
    fn census_generation_histogram_overflows_past_its_last_bucket() {
        let grid = grid_of(vec![Some(live(
            vec![op::NOP],
            Lineage {
                uid: ProgramUid::create(1, 0, 4, ProgramOrigin::Append),
                parent: ProgramUid::create(0, 0, 4, ProgramOrigin::Seed),
                birth_tick: 1,
                generation: 4_000,
            },
        ))]);

        let census = collect_census(&grid);

        assert_eq!(census.lineage.generation.overflow, 1);
        assert_eq!(census.lineage.generation.count, 1);
        assert_eq!(census.lineage.generation.max, 4_000);
        assert_eq!(census.lineage.generation.mean(), 4_000.0);
    }

    #[test]
    fn census_stack_depths_cover_live_and_inert_programs() {
        let mut deep = live(vec![op::NOP], Lineage::default());
        deep.stack = vec![1, 2, 3];
        let mut inert_with_stack = inert(vec![op::NOP], Lineage::default());
        inert_with_stack.stack = vec![7];

        let grid = grid_of(vec![
            Some(deep),
            Some(inert_with_stack),
            Some(live(vec![op::NOP], Lineage::default())),
            None,
        ]);

        let census = collect_census(&grid);

        // Depths 0, 1, and 3 land in log2 buckets 0, 1, and 2 respectively
        // (bucket 2 covers [2, 4)).
        assert_eq!(census.stack_depths.scale, HistogramScale::Log2);
        assert_eq!(census.stack_depths.counts.len(), CENSUS_STACK_BUCKETS);
        assert_eq!(
            occupied_indices(&census.stack_depths),
            vec![(0, 1), (1, 1), (2, 1)]
        );
        assert_eq!(
            census.stack_depths.count, 3,
            "stack depth is recorded over the whole population"
        );
        assert_eq!(
            census.stack_depths.sum, 4,
            "sum stays exact on a log2 scale"
        );
        assert_eq!(
            census.stack_depths.max, 3,
            "max stays exact on a log2 scale"
        );
        assert_eq!(
            census.stack_depths.overflow, 0,
            "a log2 histogram covers every u32 and can never overflow"
        );
    }

    #[test]
    fn metrics_round_trip_through_json_with_and_without_a_census() {
        let grid = grid_of(vec![Some(live(vec![op::NOP], Lineage::default()))]);
        let metrics =
            collect_metrics_with_census(&grid, 1, 2, TickReport::default(), EventTotals::default());

        let json = serde_json::to_string(&metrics).expect("metrics should serialize");
        let restored: MetricsSnapshot =
            serde_json::from_str(&json).expect("metrics should deserialize");
        assert_eq!(restored, metrics);
        assert!(restored.census.is_some());

        // A row written before this field existed still reads back.
        let legacy = serde_json::json!({
            "epoch": 0, "tick": 5, "population": 0, "live_count": 0, "inert_count": 0,
            "total_energy": 0, "packet_energy": 0, "total_mass": 0,
            "mean_program_size": 0.0, "max_program_size": 0, "unique_genomes": 0,
            "births": 0, "boot_births": 0, "spawn_births": 0, "deaths": 0, "mutations": 0,
            "event_totals": { "births": 0, "boot_births": 0, "spawn_births": 0,
                              "deaths": 0, "mutations": 0 }
        });
        let legacy: MetricsSnapshot =
            serde_json::from_value(legacy).expect("legacy rows should still deserialize");
        assert_eq!(legacy.census, None);
    }

    #[test]
    fn inspection_reports_lineage_and_nulls_what_does_not_apply() {
        let cell_count = 4;
        let seed_uid = ProgramUid::create(0, 0, cell_count, ProgramOrigin::Seed);
        let child_uid = ProgramUid::create(12, 1, cell_count, ProgramOrigin::Append);

        let grid = grid_of(vec![
            Some(live(vec![op::NOP], Lineage::root(seed_uid, 0))),
            Some(inert(vec![op::NOP], Lineage::child(child_uid, seed_uid, 0))),
            None,
            None,
        ]);

        let root = inspect_cell(&grid, 0)
            .program
            .expect("cell 0 should hold a program");
        assert_eq!(root.uid, seed_uid.get());
        assert_eq!(root.parent_uid, None, "a lineage root has no parent");
        assert_eq!(root.birth_tick, Some(0));
        assert_eq!(root.generation, 0);
        assert_eq!(root.origin, Some("seed"));
        assert_eq!(root.created_tick, Some(0));

        let body = inspect_cell(&grid, 1)
            .program
            .expect("cell 1 should hold a program");
        assert_eq!(body.uid, child_uid.get());
        assert_eq!(body.parent_uid, Some(seed_uid.get()));
        assert_eq!(body.birth_tick, None, "this body has never booted");
        assert_eq!(body.generation, 1);
        assert_eq!(body.origin, Some("append"));
        assert_eq!(body.created_tick, Some(12));
    }

    #[test]
    fn frame_encoding_uses_specified_header_and_cell_layout() {
        let mut cells = vec![Cell::default(), Cell::default()];
        let mut program = Program::new_live(vec![op::NOP, op::BOOT], Direction::Up, 7)
            .expect("program should build");
        program.tick.is_open = true;
        cells[0].program = Some(program);
        cells[0].free_energy = 300;
        cells[0].bg_mass = 2;

        let grid = Grid::from_cells(2, 1, cells).expect("grid should build");
        let frame = encode_grid_frame(&grid, 9);

        assert_eq!(&frame[0..8], &9_u64.to_le_bytes());
        assert_eq!(&frame[8..12], &2_u32.to_le_bytes());
        assert_eq!(&frame[12..16], &1_u32.to_le_bytes());
        assert_eq!(frame.len(), 32);
        assert_eq!(&frame[16..24], &[0b111, 7, 17, 255, 0, 0, 2, 0]);
        assert_eq!(&frame[24..32], &[0b100, 0, 0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn program_size_byte_is_log2_scaled() {
        for (size, expected) in [
            (0_u16, 0_u8),
            (1, 0),
            (2, 17),
            (3, 27),
            (4, 34),
            (128, 119),
            (1024, 170),
            (32767, 255),
        ] {
            assert_eq!(
                encode_program_size(size),
                expected,
                "size {size} should encode to {expected}"
            );
        }
    }

    #[test]
    fn program_size_byte_is_monotonic_and_saturates() {
        let mut previous = encode_program_size(1);
        for size in 2..=32767_u16 {
            let current = encode_program_size(size);
            assert!(
                current >= previous,
                "size {size} encoded to {current} after {previous}"
            );
            previous = current;
        }
    }

    #[test]
    fn cell_inspection_uses_spec_direction_encoding() {
        for (direction, expected) in [
            (Direction::Right, 0),
            (Direction::Up, 1),
            (Direction::Left, 2),
            (Direction::Down, 3),
        ] {
            let mut grid = Grid::new(1, 1).expect("grid should build");
            let program =
                Program::new_live(vec![op::NOP], direction, 2).expect("program should build");
            grid.get_mut(0).expect("cell should exist").program = Some(program);

            let cell = inspect_cell(&grid, 0);
            assert_eq!(cell.program.expect("program should exist").dir, expected);
        }
    }

    #[test]
    fn metrics_use_live_programs_for_size_statistics() {
        let mut cells = vec![Cell::default(), Cell::default()];
        cells[0].program = Some(
            Program::new_live(vec![op::NOP, op::ABSORB], Direction::Right, 1)
                .expect("live program"),
        );
        cells[1].program =
            Some(Program::new_inert(vec![op::NOP], Direction::Right, 2).expect("inert program"));
        cells[0].free_energy = 3;
        cells[1].bg_radiation = 4;

        let grid = Grid::from_cells(2, 1, cells).expect("grid should build");
        let metrics = collect_metrics(
            &grid,
            4,
            5,
            TickReport {
                births: 3,
                boot_births: 2,
                spawn_births: 1,
                deaths: 2,
                mutations: 3,
                packet_count: 5,
            },
            EventTotals {
                births: 30,
                boot_births: 20,
                spawn_births: 10,
                deaths: 12,
                mutations: 8,
            },
        );

        assert_eq!(metrics.epoch, 4);
        assert_eq!(metrics.population, 2);
        assert_eq!(metrics.live_count, 1);
        assert_eq!(metrics.inert_count, 1);
        assert_eq!(metrics.total_energy, 7);
        assert_eq!(metrics.packet_energy, 5);
        assert_eq!(metrics.total_mass, 3);
        assert_eq!(metrics.mean_program_size, 2.0);
        assert_eq!(metrics.max_program_size, 2);
        assert_eq!(metrics.unique_genomes, 1);
        assert_eq!(metrics.births, 3);
        assert_eq!(metrics.boot_births, 2);
        assert_eq!(metrics.spawn_births, 1);
        assert_eq!(metrics.deaths, 2);
        assert_eq!(metrics.mutations, 3);
        assert_eq!(metrics.event_totals.births, 30);
        assert_eq!(metrics.event_totals.boot_births, 20);
        assert_eq!(metrics.event_totals.spawn_births, 10);
        assert_eq!(metrics.event_totals.deaths, 12);
        assert_eq!(metrics.event_totals.mutations, 8);
    }

    #[test]
    fn event_totals_accumulate_every_tick_report() {
        let mut totals = EventTotals::default();
        totals.record(TickReport {
            births: 3,
            boot_births: 2,
            spawn_births: 1,
            deaths: 4,
            mutations: 5,
            packet_count: 99,
        });
        totals.record(TickReport {
            births: 7,
            boot_births: 6,
            spawn_births: 1,
            deaths: 8,
            mutations: 9,
            packet_count: 1,
        });

        assert_eq!(totals.births, 10);
        assert_eq!(totals.boot_births, 8);
        assert_eq!(totals.spawn_births, 2);
        assert_eq!(totals.deaths, 12);
        assert_eq!(totals.mutations, 14);
        assert_eq!(totals.births, totals.boot_births + totals.spawn_births);
    }

    #[test]
    fn disassembly_uses_api_facing_mnemonics() {
        assert_eq!(
            disassemble(&[op::push(0), op::CW, op::SET_SRC, op::APPEND_ADJ, 0xff]),
            vec!["push 0", "cw", "setSrc", "appendAdj", "noop 0xff"]
        );
    }
}
