//! Proteus stripped back: programs in cells, mass and energy, one instruction per tick.
//!
//! A world is a torus of cells. Each cell holds loose energy, loose mass, and
//! at most one body: a string of instructions that is either a live program or
//! a child still under construction. Each instruction is one unit of mass.
//!
//! On every tick each live program executes exactly one instruction. A program
//! reproduces by copying its own code, one instruction per `copy`, into the
//! cell it faces. The child wakes by itself once the parent has been through
//! its whole code. Nothing attacks anything: room is made only by programs
//! wearing out. An executed instruction occasionally mutates, a program that
//! can no longer feed itself cannot pay its upkeep, and its body dissolves
//! back into the cell.
//!
//! [`Laws`] lists everything that can vary. [`World::step`] is the whole tick.

pub mod code;
pub mod observe;
mod rng;

use code::Op;
use rng::{Rng, Stream};

/// The longest code a body may have. Only duplication errors can reach it.
pub const MAX_CODE: usize = 255;

/// The laws of one world.
#[derive(Clone, Debug, PartialEq)]
pub struct Laws {
    /// Grid width in cells. The grid wraps.
    pub width: usize,
    /// Grid height in cells.
    pub height: usize,
    /// Mean units of energy arriving in each cell per tick.
    pub energy_rate: f64,
    /// Mean units of mass arriving in each cell per tick.
    pub mass_rate: f64,
    /// Every unit of loose energy or mass, in a cell or held by a program,
    /// dissipates with probability `2^-k` per tick. `None` means never.
    pub decay_log2: Option<u32>,
    /// Every instruction of a live program costs 1 energy with probability
    /// `2^-k` per tick. An instruction whose upkeep goes unpaid dissolves.
    pub upkeep_log2: Option<u32>,
    /// An instruction mutates with probability `2^-k` each time it executes.
    pub slip_log2: Option<u32>,
    /// A copy goes wrong with probability `2^-k`: half the errors write a
    /// wrong instruction, a quarter skip one, and a quarter repeat one.
    pub miscopy_log2: Option<u32>,
    /// Selects all randomness.
    pub seed: u64,
}

impl Default for Laws {
    /// Resource rates and decay are the v0.4.0 defaults.
    fn default() -> Self {
        Self {
            width: 64,
            height: 64,
            energy_rate: 0.25,
            mass_rate: 0.05,
            decay_log2: Some(7),
            upkeep_log2: Some(7),
            slip_log2: Some(10),
            miscopy_log2: Some(7),
            seed: 1,
        }
    }
}

/// How a world begins.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Start {
    /// One live program in the middle of an otherwise empty world.
    Seed(Vec<u8>),
    /// A live program of random code, of this length, in every cell.
    Soup(usize),
}

/// A string of instructions in a cell: a live program, or a child being built.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Body {
    /// The instructions. Each is one unit of mass.
    pub code: Vec<u8>,
    /// Whether this body executes. A child under construction does not.
    pub live: bool,
    /// Index of the next instruction to execute.
    pub ip: usize,
    /// The direction faced: 0 east, 1 south, 2 west, 3 north.
    pub dir: u8,
    /// The one general register, as wide as an instruction.
    pub a: u8,
    /// Index of the next instruction to copy into the child.
    pub cursor: usize,
    /// The cell holding the child this program is building, if any.
    pub child: Option<usize>,
    /// Energy this program holds.
    pub energy: u32,
    /// Mass this program holds, beyond its own instructions.
    pub mass: u32,
    /// The tick on which this body woke. Observation only.
    pub born: u64,
    /// How many ancestors this body has. Observation only.
    pub generation: u32,
    /// How many children this body has completed. Observation only.
    pub children: u32,
}

impl Body {
    fn new(code: Vec<u8>, live: bool) -> Self {
        Self {
            code,
            live,
            ip: 0,
            dir: 0,
            a: 0,
            cursor: 0,
            child: None,
            energy: 0,
            mass: 0,
            born: 0,
            generation: 0,
            children: 0,
        }
    }
}

/// One site of the grid.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Cell {
    /// Loose energy lying in this cell.
    pub energy: u32,
    /// Loose mass lying in this cell.
    pub mass: u32,
    /// The body occupying this cell, if any.
    pub body: Option<Body>,
}

/// Counts what one tick did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Tally {
    /// Copies that succeeded.
    pub copies: u64,
    /// Children that woke.
    pub births: u64,
    /// Programs whose last instruction dissolved.
    pub deaths: u64,
    /// Total ticks lived by the programs that died.
    pub lifespans: u64,
    /// Total children completed by the programs that died.
    pub offspring: u64,
    /// Programs that died having completed at least one child.
    pub parents: u64,
    /// Instructions changed in a live body by an execution slip.
    pub slips: u64,
    /// Copies that went wrong.
    pub miscopies: u64,
    /// Units of mass that arrived from outside.
    pub mass_in: u64,
    /// Units of mass that dissipated.
    pub mass_out: u64,
}

impl std::ops::AddAssign for Tally {
    fn add_assign(&mut self, other: Self) {
        self.copies += other.copies;
        self.births += other.births;
        self.deaths += other.deaths;
        self.lifespans += other.lifespans;
        self.offspring += other.offspring;
        self.parents += other.parents;
        self.slips += other.slips;
        self.miscopies += other.miscopies;
        self.mass_in += other.mass_in;
        self.mass_out += other.mass_out;
    }
}

/// A grid of cells and the laws that move it forward.
pub struct World {
    laws: Laws,
    cells: Vec<Cell>,
    tick: u64,
}

impl World {
    /// Creates a world whose cells hold the long-run average of loose energy
    /// and mass that an untouched cell would settle at.
    pub fn new(laws: Laws, start: Start) -> Result<Self, String> {
        if laws.width == 0 || laws.height == 0 {
            return Err("a world needs at least one cell".into());
        }
        if !(laws.energy_rate >= 0.0 && laws.mass_rate >= 0.0) {
            return Err("arrival rates cannot be negative".into());
        }
        let rates = [
            laws.decay_log2,
            laws.upkeep_log2,
            laws.slip_log2,
            laws.miscopy_log2,
        ];
        if rates.iter().flatten().any(|&k| k > 63) {
            return Err("a log2 rate must be at most 63".into());
        }

        // Arrival at rate r against decay at 2^-k settles at r * 2^k.
        let settled = |rate: f64| {
            laws.decay_log2
                .map_or(0.0, |k| rate * f64::from(1u32 << k.min(31)))
        };
        let cell = Cell {
            energy: settled(laws.energy_rate).round() as u32,
            mass: settled(laws.mass_rate).round() as u32,
            body: None,
        };
        let mut world = Self {
            cells: vec![cell; laws.width * laws.height],
            laws,
            tick: 0,
        };

        match start {
            Start::Seed(code) => {
                let middle = (world.laws.height / 2) * world.laws.width + world.laws.width / 2;
                world.place(middle, code)?;
            }
            Start::Soup(length) => {
                for cell in 0..world.cells.len() {
                    let mut rng = Rng::at(world.laws.seed, 0, cell, Stream::Soup);
                    let code = (0..length).map(|_| rng.below(code::VALUES.into()) as u8);
                    world.place(cell, code.collect())?;
                }
            }
        }
        Ok(world)
    }

    /// Puts a new live program in a cell, facing a random direction.
    pub fn place(&mut self, cell: usize, code: Vec<u8>) -> Result<(), String> {
        if code.is_empty() || code.len() > MAX_CODE {
            return Err(format!("code must have 1 to {MAX_CODE} instructions"));
        }
        if code.iter().any(|&value| value >= code::VALUES) {
            return Err("an instruction value does not fit the code width".into());
        }
        let mut body = Body::new(code, true);
        body.dir = self.facing(cell);
        body.born = self.tick;
        self.cells[cell].body = Some(body);
        Ok(())
    }

    /// Advances the world by one tick.
    pub fn step(&mut self) -> Tally {
        let mut tally = Tally::default();

        // Only programs that are live as the tick begins take part in it.
        let actors: Vec<usize> = (0..self.cells.len())
            .filter(|&cell| self.cells[cell].body.as_ref().is_some_and(|body| body.live))
            .collect();

        // 1. Every program executes one instruction. Everything but `copy`
        //    touches only the program's own cell, so the order is immaterial.
        let mut executed = Vec::with_capacity(actors.len());
        let mut copies = Vec::new();
        for &cell in &actors {
            let (ip, wants_copy) = self.execute(cell);
            executed.push((cell, ip));
            if wants_copy {
                let target = self.neighbour(cell, self.body(cell).dir);
                copies.push((target, cell));
            }
        }

        // 2. Copies are judged against the world as it stood before any of
        //    them landed, then all the winners land.
        copies.sort_unstable();
        let winners: Vec<(usize, usize)> = copies
            .chunk_by(|a, b| a.0 == b.0)
            .filter_map(|rivals| self.judge(rivals))
            .collect();
        for (target, source) in winners {
            self.copy(source, target, &mut tally);
        }

        // 3. The instruction a program just executed may have slipped.
        for &(cell, ip) in &executed {
            let mut rng = Rng::at(self.laws.seed, self.tick, cell, Stream::Slip);
            if rng.chance(self.laws.slip_log2) {
                let code = &mut self.cells[cell].body.as_mut().expect("actors persist").code;
                // Always a different value, so a slip always changes something.
                let shift = 1 + rng.below(u64::from(code::VALUES) - 1) as u8;
                code[ip] = (code[ip] + shift) % code::VALUES;
                tally.slips += 1;
            }
        }

        // 4. Upkeep falls due, and a program that cannot pay starts to dissolve.
        for &cell in &actors {
            self.upkeep(cell, &mut tally);
        }

        // 5. Loose energy and mass dissipate everywhere, then more arrives.
        self.weather(&mut tally);

        self.tick += 1;
        tally
    }

    /// Executes one instruction of the program in `cell`.
    ///
    /// Returns the index of that instruction and whether it was a `copy` the
    /// program can pay for.
    fn execute(&mut self, cell: usize) -> (usize, bool) {
        let body = self.body(cell);
        let ip = body.ip;
        let op = code::decode(body.code[ip]);
        // `look` reads a neighbour, which no instruction changes during this step.
        let seen = match op {
            Op::Look => self.cells[self.neighbour(cell, body.dir)]
                .body
                .as_ref()
                .filter(|other| other.live)
                .map_or(0, |other| other.code.len().min(code::A_MAX.into()) as u8),
            _ => 0,
        };

        let Cell { energy, mass, body } = &mut self.cells[cell];
        let body = body.as_mut().expect("actors are occupied cells");
        let mut skip = 0;
        let mut wants_copy = false;
        match op {
            Op::Absorb => body.energy += std::mem::take(energy),
            Op::Collect => body.mass += std::mem::take(mass),
            Op::Copy => wants_copy = body.energy > 0 && body.mass > 0,
            Op::Turn => body.dir = (body.dir + 1) % 4,
            Op::Look => body.a = seen,
            Op::Weigh => body.a = body.mass.min(code::A_MAX.into()) as u8,
            Op::Dec => body.a = body.a.saturating_sub(1),
            Op::Skip => skip = usize::from(body.a == 0),
            Op::Nop => {}
        }
        body.ip = (ip + 1 + skip) % body.code.len();
        (ip, wants_copy)
    }

    /// Decides which of the programs copying into one cell succeeds, if any.
    ///
    /// `rivals` are (target, source) pairs that share a target.
    fn judge(&self, rivals: &[(usize, usize)]) -> Option<(usize, usize)> {
        let target = rivals[0].0;
        match &self.cells[target].body {
            // An occupied cell takes a copy only from the program building the
            // child in it. A live program is nobody's unfinished child.
            Some(_) => rivals
                .iter()
                .copied()
                .find(|&(_, source)| self.body(source).child == Some(target)),
            // An empty cell goes to one of the programs that want it.
            None => {
                let mut rng = Rng::at(self.laws.seed, self.tick, target, Stream::Contest);
                let pick = match rivals.len() {
                    1 => 0,
                    n => rng.below(n as u64) as usize,
                };
                Some(rivals[pick])
            }
        }
    }

    /// Carries out one successful `copy` from the program in `source`.
    fn copy(&mut self, source: usize, target: usize, tally: &mut Tally) {
        // A program builds one child at a time. Starting in a fresh cell
        // abandons any child left unfinished elsewhere.
        if self.body(source).child != Some(target) {
            if let Some(abandoned) = self.body(source).child {
                self.dissolve(abandoned);
            }
            self.cells[target].body = Some(Body::new(Vec::new(), false));
            let parent = self.body_mut(source);
            parent.child = Some(target);
            parent.cursor = 0;
        }

        let mut rng = Rng::at(self.laws.seed, self.tick, source, Stream::Miscopy);
        let error = match rng.chance(self.laws.miscopy_log2) {
            true => Some(rng.below(4)),
            false => None,
        };
        tally.copies += 1;
        tally.miscopies += u64::from(error.is_some());

        let child_len = self.body(target).code.len();
        let parent = self.body_mut(source);
        let mut written = None;
        if error == Some(2) {
            // Skip: the cursor moves on without copying, then copies as usual.
            parent.cursor += 1;
        }
        if parent.cursor < parent.code.len() {
            let mut value = parent.code[parent.cursor];
            if matches!(error, Some(0 | 1)) {
                // Wrong instruction: any value but the right one.
                let shift = 1 + rng.below(u64::from(code::VALUES) - 1) as u8;
                value = (value + shift) % code::VALUES;
            }
            // Repeat: the cursor stays, so the next copy writes this again.
            let repeat = error == Some(3) && child_len + 1 < MAX_CODE;
            parent.cursor += usize::from(!repeat);
            parent.energy -= 1;
            parent.mass -= 1;
            written = Some(value);
        }
        let finished = parent.cursor >= parent.code.len();
        let generation = parent.generation + 1;
        if finished {
            parent.child = None;
            parent.cursor = 0;
        }

        let dir = self.facing(target);
        let born = self.tick + 1;
        let child = &mut self.cells[target].body;
        let code = &mut child.as_mut().expect("the child was just checked").code;
        code.extend(written);
        if !finished {
            return;
        }
        if code.is_empty() {
            // A skip swallowed the only instruction: there is no child.
            *child = None;
            return;
        }
        let child = child.as_mut().expect("the child was just checked");
        child.live = true;
        child.dir = dir;
        child.born = born;
        child.generation = generation;
        self.body_mut(source).children += 1;
        tally.births += 1;
    }

    /// Charges a live program its upkeep and dissolves what it cannot pay for.
    fn upkeep(&mut self, cell: usize, tally: &mut Tally) {
        let mut rng = Rng::at(self.laws.seed, self.tick, cell, Stream::Upkeep);
        let Cell { mass, body, .. } = &mut self.cells[cell];
        let program = body.as_mut().expect("actors persist until upkeep");
        let due = rng.binomial(program.code.len() as u32, self.laws.upkeep_log2);
        let paid = due.min(program.energy);
        program.energy -= paid;

        // Each unpaid unit dissolves the last instruction back into the cell.
        let lost = ((due - paid) as usize).min(program.code.len());
        program.code.truncate(program.code.len() - lost);
        *mass += lost as u32;
        if !program.code.is_empty() {
            program.ip %= program.code.len();
            program.cursor = program.cursor.min(program.code.len());
            return;
        }

        // Nothing is left. What the program held returns to its cell, and the
        // child it was building dissolves where it lies.
        let dead = body.take().expect("the program was just charged");
        tally.deaths += 1;
        tally.lifespans += self.tick - dead.born;
        tally.offspring += u64::from(dead.children);
        tally.parents += u64::from(dead.children > 0);
        self.cells[cell].energy += dead.energy;
        self.cells[cell].mass += dead.mass;
        if let Some(orphan) = dead.child {
            self.dissolve(orphan);
        }
    }

    /// Turns the unfinished child in `cell` back into loose mass.
    fn dissolve(&mut self, cell: usize) {
        let body = self.cells[cell]
            .body
            .take()
            .expect("a child was recorded here");
        debug_assert!(!body.live);
        self.cells[cell].mass += body.code.len() as u32;
    }

    /// Lets loose energy and mass dissipate, then lets more arrive.
    fn weather(&mut self, tally: &mut Tally) {
        let Laws {
            seed, decay_log2, ..
        } = self.laws;
        let arrivals = [self.laws.energy_rate, self.laws.mass_rate].map(|rate| (-rate).exp());
        for (index, cell) in self.cells.iter_mut().enumerate() {
            let mut rng = Rng::at(seed, self.tick, index, Stream::Decay);
            cell.energy -= rng.binomial(cell.energy, decay_log2);
            let mut lost = rng.binomial(cell.mass, decay_log2);
            cell.mass -= lost;
            if let Some(body) = cell.body.as_mut().filter(|body| body.live) {
                body.energy -= rng.binomial(body.energy, decay_log2);
                let held = rng.binomial(body.mass, decay_log2);
                body.mass -= held;
                lost += held;
            }
            tally.mass_out += u64::from(lost);

            let mut rng = Rng::at(seed, self.tick, index, Stream::Arrival);
            if arrivals[0] < 1.0 {
                cell.energy += rng.poisson(arrivals[0]);
            }
            if arrivals[1] < 1.0 {
                let arrived = rng.poisson(arrivals[1]);
                cell.mass += arrived;
                tally.mass_in += u64::from(arrived);
            }
        }
    }

    /// Picks the direction a body starting in `cell` this tick will face.
    fn facing(&self, cell: usize) -> u8 {
        Rng::at(self.laws.seed, self.tick, cell, Stream::Facing).below(4) as u8
    }

    /// Returns the cell next to `cell` in direction `dir`.
    pub fn neighbour(&self, cell: usize, dir: u8) -> usize {
        let (width, height) = (self.laws.width, self.laws.height);
        let (x, y) = (cell % width, cell / width);
        let (x, y) = match dir {
            0 => ((x + 1) % width, y),
            1 => (x, (y + 1) % height),
            2 => ((x + width - 1) % width, y),
            _ => (x, (y + height - 1) % height),
        };
        y * width + x
    }

    /// Returns the grid distance between two cells on the torus.
    pub fn distance(&self, a: usize, b: usize) -> usize {
        let (width, height) = (self.laws.width, self.laws.height);
        let dx = (a % width).abs_diff(b % width);
        let dy = (a / width).abs_diff(b / width);
        dx.min(width - dx) + dy.min(height - dy)
    }

    fn body(&self, cell: usize) -> &Body {
        self.cells[cell]
            .body
            .as_ref()
            .expect("the cell is occupied")
    }

    fn body_mut(&mut self, cell: usize) -> &mut Body {
        self.cells[cell]
            .body
            .as_mut()
            .expect("the cell is occupied")
    }

    /// Returns the laws this world runs under.
    pub fn laws(&self) -> &Laws {
        &self.laws
    }

    /// Returns how many ticks have elapsed.
    pub fn tick(&self) -> u64 {
        self.tick
    }

    /// Returns every cell, row by row.
    pub fn cells(&self) -> &[Cell] {
        &self.cells
    }

    /// Returns one cell for editing, for example to set up an experiment.
    pub fn cell_mut(&mut self, cell: usize) -> &mut Cell {
        &mut self.cells[cell]
    }

    /// Returns all the mass in the world: loose, held, and bound in bodies.
    pub fn mass(&self) -> u64 {
        self.cells
            .iter()
            .map(|cell| {
                let body = cell.body.as_ref();
                let bound = body.map_or(0, |body| body.code.len() as u64 + u64::from(body.mass));
                u64::from(cell.mass) + bound
            })
            .sum()
    }

    /// Returns a 64-bit FNV-1a fingerprint of the whole state.
    pub fn digest(&self) -> u64 {
        let mut hash = 0xcbf2_9ce4_8422_2325u64;
        let mut feed = |value: u64| {
            for byte in value.to_le_bytes() {
                hash = (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3);
            }
        };
        feed(self.tick);
        for cell in &self.cells {
            feed(u64::from(cell.energy) << 32 | u64::from(cell.mass));
            let Some(body) = &cell.body else {
                feed(u64::MAX);
                continue;
            };
            feed(body.code.len() as u64);
            body.code.iter().for_each(|&value| feed(value.into()));
            feed(u64::from(body.live) | (body.ip as u64) << 8 | (body.cursor as u64) << 32);
            feed(u64::from(body.dir) | u64::from(body.a) << 8);
            feed(body.child.map_or(u64::MAX, |child| child as u64));
            feed(u64::from(body.energy) << 32 | u64::from(body.mass));
        }
        hash
    }
}
