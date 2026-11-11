//! Proteus stripped back to its skeleton: programs in cells, a speed of light, and ticks.
//!
//! A world is a torus of cells. Each cell holds one fixed-length tape of bytes,
//! and the tape is all there is: no registers survive a tick, no stack, no mass
//! or energy, no lifecycle. On every tick the cells split into disjoint pairs
//! that lie within the speed of light of each other. Each pair's tapes are
//! joined end to end, run as one self-modifying program, and split again.
//! Then every byte mutates with a small probability.
//!
//! With the default rules this is BFF (Agüera y Arcas et al., arXiv:2406.19108)
//! on a Proteus grid. [`Rules`] lists the laws that can vary.

pub mod observe;
mod rng;
pub mod vm;

use rng::{Rng, Stream};
use vm::OpTable;

/// How the cells pair up on each tick.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Schedule {
    /// Cells are visited in random order and each proposes to one random cell
    /// within the speed of light; the pair forms if both are still free.
    Random,
    /// Cells are tiled with dominoes. The tiling cycles through all four
    /// offsets and both orientations, so every cell meets each neighbour twice
    /// in eight ticks, once in each role. Uses no randomness.
    Tiling,
}

/// Where head 1 starts in a joined pair of tapes.
///
/// The instruction pointer and head 0 always start on the first tape, so this
/// decides whether a copy begun at once stays within that tape or crosses over.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Head1 {
    /// On the first byte of the first tape, as in BFF.
    First,
    /// On the first byte of the second tape.
    Second,
}

/// The laws of one world. Every field is a law that can be varied.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rules {
    /// Grid width in cells. The grid wraps.
    pub width: usize,
    /// Grid height in cells.
    pub height: usize,
    /// Bytes on each cell's tape.
    pub tape: usize,
    /// The instruction set, as the characters of its enabled instructions.
    pub ops: String,
    /// Time: how many bytes a pair may read as instructions in one tick.
    pub steps: u32,
    /// Speed of light: the furthest two interacting cells may be apart, in
    /// grid steps. Zero removes locality, so any two cells may meet.
    pub light: u32,
    /// How the cells pair up on each tick.
    pub schedule: Schedule,
    /// Where head 1 starts.
    pub head1: Head1,
    /// Each byte is replaced by a random byte with probability `2^-k` per
    /// tick; `None` means never.
    pub mutation_log2: Option<u32>,
    /// Selects the initial soup and all later randomness.
    pub seed: u64,
}

impl Default for Rules {
    /// BFF's published rules on a nearest-neighbour grid.
    fn default() -> Self {
        Self {
            width: 64,
            height: 64,
            tape: 64,
            ops: vm::BFF.to_string(),
            steps: 1 << 13,
            light: 1,
            schedule: Schedule::Random,
            head1: Head1::First,
            mutation_log2: Some(12),
            seed: 1,
        }
    }
}

/// Counts what one tick did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Tick {
    /// Pairs that interacted.
    pub pairs: u64,
    /// Bytes read as instructions across all pairs.
    pub reads: u64,
    /// Reads that were instructions, not inert data.
    pub ops: u64,
}

/// A grid of tapes and the laws that move it forward.
pub struct World {
    rules: Rules,
    table: OpTable,
    reach: Vec<(i64, i64)>,
    tapes: Vec<u8>,
    tick: u64,
}

impl World {
    /// Creates a world whose tapes are uniformly random bytes.
    pub fn new(rules: Rules) -> Result<Self, String> {
        let table = OpTable::parse(&rules.ops)?;
        let cells = rules.width * rules.height;
        if cells < 2 || rules.tape == 0 {
            return Err("a world needs at least two cells and a nonempty tape".into());
        }
        if rules.mutation_log2.is_some_and(|k| k > 63) {
            return Err("mutation_log2 must be at most 63".into());
        }
        if rules.schedule == Schedule::Tiling
            && (rules.light != 1 || rules.width % 2 == 1 || rules.height % 2 == 1)
        {
            return Err("the tiling schedule needs light = 1 and even width and height".into());
        }

        let mut tapes = vec![0u8; cells * rules.tape];
        for (cell, tape) in tapes.chunks_mut(rules.tape).enumerate() {
            let mut rng = Rng::at(rules.seed, 0, cell as u64, Stream::Soup);
            tape.fill_with(|| rng.next() as u8);
        }
        let reach = offsets_within(rules.light);
        Ok(Self {
            rules,
            table,
            reach,
            tapes,
            tick: 0,
        })
    }

    /// Advances the world by one tick: pair, run, mutate.
    pub fn step(&mut self) -> Tick {
        let len = self.rules.tape;
        let head1 = match self.rules.head1 {
            Head1::First => 0,
            Head1::Second => len,
        };
        let mut joined = vec![0u8; 2 * len];
        let mut tick = Tick::default();

        // Pairs are disjoint and fixed before anything runs, so their order
        // here cannot matter.
        for (first, second) in self.pairs() {
            joined[..len].copy_from_slice(self.tape(first));
            joined[len..].copy_from_slice(self.tape(second));
            let run = vm::run(&mut joined, &self.table, self.rules.steps, head1);
            self.tape_mut(first).copy_from_slice(&joined[..len]);
            self.tape_mut(second).copy_from_slice(&joined[len..]);
            tick.pairs += 1;
            tick.reads += u64::from(run.reads);
            tick.ops += u64::from(run.ops);
        }

        self.mutate();
        self.tick += 1;
        tick
    }

    /// Lists this tick's interacting pairs as (first tape, second tape).
    ///
    /// The pairing depends on the seed and the tick, never on tape contents.
    pub fn pairs(&self) -> Vec<(usize, usize)> {
        match self.rules.schedule {
            Schedule::Random => self.random_pairs(),
            Schedule::Tiling => self.tiling_pairs(),
        }
    }

    fn random_pairs(&self) -> Vec<(usize, usize)> {
        let cells = self.cells();
        let mut rng = Rng::at(self.rules.seed, self.tick, 0, Stream::Schedule);
        let mut order: Vec<usize> = (0..cells).collect();
        for i in (1..cells).rev() {
            order.swap(i, rng.below(i as u64 + 1) as usize);
        }

        let mut taken = vec![false; cells];
        let mut pairs = Vec::with_capacity(cells / 2);
        for cell in order {
            let partner = if self.reach.is_empty() {
                rng.below(cells as u64) as usize
            } else {
                let (dx, dy) = self.reach[rng.below(self.reach.len() as u64) as usize];
                self.shifted(cell, dx, dy)
            };
            // The draw above happens even for a taken cell, so one cell's
            // luck never changes what the cells after it draw.
            if cell != partner && !taken[cell] && !taken[partner] {
                taken[cell] = true;
                taken[partner] = true;
                pairs.push((cell, partner));
            }
        }
        pairs
    }

    fn tiling_pairs(&self) -> Vec<(usize, usize)> {
        let phase = self.tick % 8;
        let (vertical, offset, flipped) = (phase % 2 == 1, (phase / 2) % 2, phase >= 4);
        let (width, height) = (self.rules.width, self.rules.height);
        let mut pairs = Vec::with_capacity(self.cells() / 2);
        for y in 0..height {
            for x in 0..width {
                let along = if vertical { y } else { x };
                if along % 2 != offset as usize {
                    continue;
                }
                let cell = y * width + x;
                let neighbour = if vertical {
                    self.shifted(cell, 0, 1)
                } else {
                    self.shifted(cell, 1, 0)
                };
                pairs.push(if flipped {
                    (neighbour, cell)
                } else {
                    (cell, neighbour)
                });
            }
        }
        pairs
    }

    fn mutate(&mut self) {
        let Some(k) = self.rules.mutation_log2 else {
            return;
        };
        let (seed, tick, len) = (self.rules.seed, self.tick, self.rules.tape);
        for (cell, tape) in self.tapes.chunks_mut(len).enumerate() {
            let mut rng = Rng::at(seed, tick, cell as u64, Stream::Mutation);
            for word in tape.chunks_mut(64) {
                // ANDing k uniform words leaves each bit set with probability 2^-k.
                let mut hits = (0..k).fold(u64::MAX, |mask, _| mask & rng.next());
                hits &= u64::MAX >> (64 - word.len());
                while hits != 0 {
                    word[hits.trailing_zeros() as usize] = rng.next() as u8;
                    hits &= hits - 1;
                }
            }
        }
    }

    fn shifted(&self, cell: usize, dx: i64, dy: i64) -> usize {
        let (width, height) = (self.rules.width as i64, self.rules.height as i64);
        let x = ((cell as i64 % width) + dx).rem_euclid(width);
        let y = ((cell as i64 / width) + dy).rem_euclid(height);
        (y * width + x) as usize
    }

    /// Returns the laws this world runs under.
    pub fn rules(&self) -> &Rules {
        &self.rules
    }

    /// Returns the instruction set as a lookup table.
    pub fn table(&self) -> &OpTable {
        &self.table
    }

    /// Returns how many ticks have elapsed.
    pub fn tick(&self) -> u64 {
        self.tick
    }

    /// Returns the number of cells.
    pub fn cells(&self) -> usize {
        self.rules.width * self.rules.height
    }

    /// Returns one cell's tape. Cells are numbered row by row.
    pub fn tape(&self, cell: usize) -> &[u8] {
        &self.tapes[cell * self.rules.tape..][..self.rules.tape]
    }

    /// Returns one cell's tape for editing, for example to plant a program.
    pub fn tape_mut(&mut self, cell: usize) -> &mut [u8] {
        &mut self.tapes[cell * self.rules.tape..][..self.rules.tape]
    }

    /// Returns every tape, concatenated in cell order.
    pub fn bytes(&self) -> &[u8] {
        &self.tapes
    }

    /// Returns the grid distance between two cells on the torus.
    pub fn distance(&self, a: usize, b: usize) -> usize {
        let (width, height) = (self.rules.width, self.rules.height);
        let dx = (a % width).abs_diff(b % width);
        let dy = (a / width).abs_diff(b / width);
        dx.min(width - dx) + dy.min(height - dy)
    }

    /// Returns a 64-bit FNV-1a fingerprint of the tick count and every tape.
    pub fn digest(&self) -> u64 {
        let tick = self.tick.to_le_bytes();
        tick.iter()
            .chain(&self.tapes)
            .fold(0xcbf2_9ce4_8422_2325, |hash, &byte| {
                (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3)
            })
    }
}

/// Lists the nonzero offsets within `light` grid steps of a cell.
fn offsets_within(light: u32) -> Vec<(i64, i64)> {
    let light = i64::from(light);
    let mut offsets = Vec::new();
    for dy in -light..=light {
        for dx in -light..=light {
            if (dx, dy) != (0, 0) && dx.abs() + dy.abs() <= light {
                offsets.push((dx, dy));
            }
        }
    }
    offsets
}
