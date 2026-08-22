//! Defines the core world, program, and queued-action data structures.

use std::error::Error;
use std::fmt;

use crate::config::PROGRAM_SIZE_CAP;

/// Represents one of the four cardinal directions used throughout the world.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Direction {
    #[default]
    Right = 0,
    Up = 1,
    Left = 2,
    Down = 3,
}

impl Direction {
    /// Lists all cardinal directions in spec encoding order.
    pub const ALL: [Self; 4] = [Self::Right, Self::Up, Self::Left, Self::Down];

    /// Converts an arbitrary signed value into a wrapped direction.
    pub fn from_i16(value: i16) -> Self {
        match value.rem_euclid(4) {
            0 => Self::Right,
            1 => Self::Up,
            2 => Self::Left,
            _ => Self::Down,
        }
    }

    /// Rotates a direction one step clockwise.
    pub fn clockwise(self) -> Self {
        match self {
            Self::Right => Self::Down,
            Self::Up => Self::Right,
            Self::Left => Self::Up,
            Self::Down => Self::Left,
        }
    }

    /// Rotates a direction one step counterclockwise.
    pub fn counterclockwise(self) -> Self {
        match self {
            Self::Right => Self::Up,
            Self::Up => Self::Left,
            Self::Left => Self::Down,
            Self::Down => Self::Right,
        }
    }

    /// Returns the opposite cardinal direction.
    pub fn opposite(self) -> Self {
        match self {
            Self::Right => Self::Left,
            Self::Up => Self::Down,
            Self::Left => Self::Right,
            Self::Down => Self::Up,
        }
    }
}

/// Stores the mutable registers carried by one program.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Registers {
    pub ip: u16,
    pub dir: Direction,
    pub src: u16,
    pub dst: u16,
    pub flag: bool,
    pub msg: i16,
    pub id: u8,
    pub lc: i16,
}

/// Stores per-tick transient flags and counters for one program.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TickState {
    pub absorb_count: u8,
    pub absorb_dir: Option<Direction>,
    pub did_listen: bool,
    pub did_collect: bool,
    pub did_nop: bool,
    pub is_open: bool,
    pub bg_radiation_consumed: u32,
    pub is_newborn: bool,
    /// Whether this program existed when the current tick began.
    pub existed_at_tick_start: bool,
    /// Whether this program was live and eligible when the current tick began.
    pub was_live_at_tick_start: bool,
}

impl TickState {
    /// Clears all transient tick state back to defaults.
    pub fn reset_for_new_tick(&mut self) {
        *self = Self::default();
    }

    /// Prepares the transient state for the start of Pass 1.
    pub fn reset_for_pass1(&mut self, is_inert: bool) {
        let is_newborn = self.is_newborn;
        let existed_at_tick_start = self.existed_at_tick_start;
        let was_live_at_tick_start = self.was_live_at_tick_start;
        *self = Self {
            is_open: is_inert,
            is_newborn,
            existed_at_tick_start,
            was_live_at_tick_start,
            ..Self::default()
        };
    }
}

/// Identifies one program uniquely within a single simulation instance.
///
/// Encodes the creation site: `1 + ((tick * cell_count + cell_index) * 4 + origin)`.
/// Never reused within a simulation, because `tick` is monotone. Uids are
/// *epoch-scoped*: they are not comparable across simulations, across
/// `POST /v1/sim/reset` (each reset builds a fresh simulation and a new metrics
/// epoch), or across grids of different size.
///
/// Uniqueness holds while `(tick * cell_count + cell_index) * 4 + 1 < 2^64`,
/// i.e. roughly 2.8e14 ticks at 16 384 cells. Past that ceiling the encoding
/// wraps and uids may repeat.
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    serde::Serialize,
    serde::Deserialize,
)]
#[serde(transparent)]
pub struct ProgramUid(pub u64);

impl ProgramUid {
    /// Marks "no parent" or "unidentified program".
    pub const NONE: Self = Self(0);

    /// Reports whether this uid identifies no program.
    pub fn is_none(self) -> bool {
        self == Self::NONE
    }

    /// Returns the raw encoded value.
    pub fn get(self) -> u64 {
        self.0
    }

    /// Derives the uid for a program created at one site.
    ///
    /// Purely a function of the creation site, so serial and Rayon builds agree
    /// by construction without any shared counter.
    ///
    /// Panics in every build when the encoding exceeds `u64` (≈ 2.8e14 ticks
    /// at 16 384 cells): a wrapped uid would silently alias an earlier program,
    /// which is worse than stopping.
    pub fn create(tick: u64, cell_index: usize, cell_count: usize, origin: ProgramOrigin) -> Self {
        let encoded = tick
            .checked_mul(cell_count as u64)
            .and_then(|base| base.checked_add(cell_index as u64))
            .and_then(|slot| slot.checked_mul(ProgramOrigin::RESERVED_SLOTS))
            .and_then(|slot| slot.checked_add(origin as u64))
            .and_then(|slot| slot.checked_add(1))
            .expect("program uid encoding overflowed u64");
        Self(encoded)
    }

    /// Decodes just the creation origin, which needs no grid size.
    ///
    /// `None` for [`ProgramUid::NONE`] and for the reserved fourth slot.
    pub fn origin(self) -> Option<ProgramOrigin> {
        if self.is_none() {
            return None;
        }
        ProgramOrigin::from_code((self.0 - 1) % ProgramOrigin::RESERVED_SLOTS)
    }

    /// Decodes the creation site; `None` for [`ProgramUid::NONE`].
    pub fn site(self, cell_count: usize) -> Option<ProgramSite> {
        if self.is_none() || cell_count == 0 {
            return None;
        }

        let encoded = self.0 - 1;
        let origin = self.origin()?;
        let slot = encoded / ProgramOrigin::RESERVED_SLOTS;

        Some(ProgramSite {
            tick: slot / cell_count as u64,
            cell_index: (slot % cell_count as u64) as usize,
            origin,
        })
    }
}

/// Names how a program first came into existence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ProgramOrigin {
    Seed = 0,
    Spawn = 1,
    Append = 2,
}

impl ProgramOrigin {
    /// Reserves four uid slots per cell per tick, so two creations at the same
    /// `(tick, cell_index)` with different origins never collide.
    pub const RESERVED_SLOTS: u64 = 4;

    /// Returns the wire label for this origin.
    pub fn label(self) -> &'static str {
        match self {
            Self::Seed => "seed",
            Self::Spawn => "spawn",
            Self::Append => "append",
        }
    }

    /// Decodes one origin tag, rejecting the reserved fourth slot.
    pub fn from_code(code: u64) -> Option<Self> {
        match code {
            0 => Some(Self::Seed),
            1 => Some(Self::Spawn),
            2 => Some(Self::Append),
            _ => None,
        }
    }
}

/// Decoded creation site of one [`ProgramUid`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProgramSite {
    pub tick: u64,
    pub cell_index: usize,
    pub origin: ProgramOrigin,
}

/// Records where one program came from. Observation-only: no simulation rule
/// reads it and no instruction can observe it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Lineage {
    pub uid: ProgramUid,
    pub parent: ProgramUid,
    /// Tick at which this program became live, or [`Lineage::NEVER_LIVE`].
    pub birth_tick: u32,
    pub generation: u32,
}

impl Lineage {
    /// Marks a program that has never been live (an inert body awaiting `boot`).
    pub const NEVER_LIVE: u32 = u32::MAX;

    /// Builds a parentless root (seed program or spontaneous spawn).
    pub fn root(uid: ProgramUid, birth_tick: u64) -> Self {
        Self {
            uid,
            parent: ProgramUid::NONE,
            birth_tick: clamp_tick(birth_tick),
            generation: 0,
        }
    }

    /// Builds an inert child of `parent`; birth tick is set when it boots.
    pub fn child(uid: ProgramUid, parent: ProgramUid, parent_generation: u32) -> Self {
        Self {
            uid,
            parent,
            birth_tick: Self::NEVER_LIVE,
            generation: parent_generation.saturating_add(1),
        }
    }

    /// Records the tick at which the program became live.
    pub fn mark_live(&mut self, tick: u64) {
        self.birth_tick = clamp_tick(tick);
    }

    /// Returns the birth tick, or `None` while the program has never been live.
    pub fn birth_tick(&self) -> Option<u32> {
        (self.birth_tick != Self::NEVER_LIVE).then_some(self.birth_tick)
    }
}

/// Clamps a tick into the `u32` birth-tick field.
///
/// Birth ticks saturate one below [`Lineage::NEVER_LIVE`], so the sentinel stays
/// unambiguous. Runs past ~4.29e9 ticks report a clamped birth tick.
fn clamp_tick(tick: u64) -> u32 {
    tick.min(u64::from(Lineage::NEVER_LIVE - 1)) as u32
}

/// Represents one program occupying a cell.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Program {
    pub code: Vec<u8>,
    pub registers: Registers,
    pub stack: Vec<i16>,
    pub live: bool,
    pub age: u32,
    pub abandonment_timer: u32,
    pub tick: TickState,
    /// Observation-only provenance; never read by a simulation rule.
    pub lineage: Lineage,
}

impl Program {
    /// Builds a live program with initialized registers and empty transient state.
    pub fn new_live(code: Vec<u8>, dir: Direction, id: u8) -> Result<Self, ProgramError> {
        Self::new(code, dir, id, true)
    }

    /// Builds an inert program with initialized registers and empty transient state.
    pub fn new_inert(code: Vec<u8>, dir: Direction, id: u8) -> Result<Self, ProgramError> {
        Self::new(code, dir, id, false)
    }

    /// Returns the program length as a validated `u16`.
    pub fn size(&self) -> u16 {
        u16::try_from(self.code.len())
            .expect("program size exceeds u16::MAX; constructors enforce the configured cap")
    }

    /// Reports whether the program is currently inert.
    pub fn is_inert(&self) -> bool {
        !self.live
    }

    /// Attaches lineage to a freshly constructed program.
    #[must_use]
    pub fn with_lineage(mut self, lineage: Lineage) -> Self {
        self.lineage = lineage;
        self
    }

    /// Builds a program after enforcing the shared constructor invariants.
    fn new(code: Vec<u8>, dir: Direction, id: u8, live: bool) -> Result<Self, ProgramError> {
        validate_code_size(code.len())?;

        let registers = Registers {
            dir,
            id,
            ..Registers::default()
        };

        Ok(Self {
            code,
            registers,
            stack: Vec::new(),
            live,
            age: 0,
            abandonment_timer: 0,
            tick: TickState::default(),
            lineage: Lineage::default(),
        })
    }
}

/// Validates that a program length fits the spec constraints.
fn validate_code_size(len: usize) -> Result<(), ProgramError> {
    if len == 0 {
        return Err(ProgramError::EmptyCode);
    }
    if len > usize::from(PROGRAM_SIZE_CAP) {
        return Err(ProgramError::SizeCapExceeded {
            attempted: len,
            cap: usize::from(PROGRAM_SIZE_CAP),
        });
    }
    Ok(())
}

/// Describes why a program could not be constructed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProgramError {
    EmptyCode,
    SizeCapExceeded { attempted: usize, cap: usize },
}

impl fmt::Display for ProgramError {
    /// Formats a human-readable program construction error.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyCode => write!(f, "programs must contain at least one instruction"),
            Self::SizeCapExceeded { attempted, cap } => {
                write!(f, "program size {attempted} exceeds cap {cap}")
            }
        }
    }
}

impl Error for ProgramError {}

/// Stores the full mutable state for one world cell.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Cell {
    pub program: Option<Program>,
    pub free_energy: u32,
    pub free_mass: u32,
    pub bg_radiation: u32,
    pub bg_mass: u32,
}

impl Cell {
    /// Builds an empty cell with no program or resources.
    pub fn empty() -> Self {
        Self::default()
    }

    /// Builds a cell that starts with a program and default resources.
    pub fn with_program(program: Program) -> Self {
        Self {
            program: Some(program),
            ..Self::default()
        }
    }

    /// Reports whether the cell currently contains a program.
    pub fn has_program(&self) -> bool {
        self.program.is_some()
    }
}

/// Captures the immutable per-cell view used during Pass 1 sensing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CellSnapshot {
    pub free_energy: u32,
    pub free_mass: u32,
    pub bg_radiation: u32,
    pub bg_mass: u32,
    pub program_size: u16,
    pub program_id: u8,
    pub has_program: bool,
}

impl From<&Cell> for CellSnapshot {
    /// Projects a cell into the snapshot fields read by Pass 1.
    fn from(cell: &Cell) -> Self {
        let (program_size, program_id, has_program) =
            cell.program.as_ref().map_or((0, 0, false), |program| {
                (program.size(), program.registers.id, true)
            });

        Self {
            free_energy: cell.free_energy,
            free_mass: cell.free_mass,
            bg_radiation: cell.bg_radiation,
            bg_mass: cell.bg_mass,
            program_size,
            program_id,
            has_program,
        }
    }
}

/// Represents one directed radiation packet moving between cells.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Packet {
    pub position: usize,
    pub direction: Direction,
    pub message: i16,
}

/// Represents one nonlocal action queued during Pass 1 for Pass 2 resolution.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QueuedAction {
    ReadAdj {
        source: usize,
        target: usize,
        src_cursor: u16,
    },
    WriteAdj {
        source: usize,
        target: usize,
        value: u8,
        dst_cursor: u16,
    },
    AppendAdj {
        source: usize,
        target: usize,
        value: u8,
    },
    DelAdj {
        source: usize,
        target: usize,
        dst_cursor: u16,
    },
    GiveE {
        source: usize,
        target: usize,
        amount: i16,
    },
    GiveM {
        source: usize,
        target: usize,
        amount: i16,
    },
    Move {
        source: usize,
        target: usize,
    },
    Boot {
        source: usize,
        target: usize,
    },
}

#[cfg(test)]
mod tests {
    use super::{
        Cell, CellSnapshot, Direction, Lineage, Program, ProgramError, ProgramOrigin, ProgramSite,
        ProgramUid, TickState,
    };

    #[test]
    fn direction_rotation_matches_spec_clockwise_order() {
        assert_eq!(Direction::Right.clockwise(), Direction::Down);
        assert_eq!(Direction::Right.counterclockwise(), Direction::Up);
        assert_eq!(Direction::Up.opposite(), Direction::Down);
        assert_eq!(Direction::from_i16(-1), Direction::Down);
    }

    #[test]
    fn program_rejects_empty_code() {
        assert_eq!(
            Program::new_live(Vec::new(), Direction::Right, 7),
            Err(ProgramError::EmptyCode)
        );
    }

    #[test]
    fn snapshot_uses_zero_program_fields_for_empty_cells() {
        let snapshot = CellSnapshot::from(&Cell::empty());
        assert_eq!(snapshot.program_size, 0);
        assert_eq!(snapshot.program_id, 0);
        assert!(!snapshot.has_program);
    }

    #[test]
    fn pass1_reset_preserves_tick_start_eligibility() {
        let mut tick = TickState {
            did_nop: true,
            is_newborn: true,
            existed_at_tick_start: true,
            was_live_at_tick_start: false,
            ..TickState::default()
        };

        tick.reset_for_pass1(false);

        assert!(!tick.did_nop);
        assert!(tick.is_newborn);
        assert!(tick.existed_at_tick_start);
        assert!(!tick.was_live_at_tick_start);
    }

    #[test]
    fn program_uid_round_trips_through_its_creation_site() {
        let cases = [
            (0_u64, 0_usize, 1_usize, ProgramOrigin::Seed),
            (0, 4095, 4096, ProgramOrigin::Seed),
            (1, 0, 16_384, ProgramOrigin::Spawn),
            (26_212, 9_311, 16_384, ProgramOrigin::Append),
            (1_000_000, 65_535, 65_536, ProgramOrigin::Spawn),
        ];

        for (tick, cell_index, cell_count, origin) in cases {
            let uid = ProgramUid::create(tick, cell_index, cell_count, origin);
            assert_eq!(
                uid.site(cell_count),
                Some(ProgramSite {
                    tick,
                    cell_index,
                    origin,
                }),
                "uid {uid:?} should decode back to its creation site"
            );
        }
    }

    #[test]
    fn program_uid_separates_origins_at_the_same_site() {
        let seed = ProgramUid::create(7, 3, 64, ProgramOrigin::Seed);
        let spawn = ProgramUid::create(7, 3, 64, ProgramOrigin::Spawn);
        let append = ProgramUid::create(7, 3, 64, ProgramOrigin::Append);

        assert_ne!(seed, spawn);
        assert_ne!(spawn, append);
        assert_ne!(seed, append);
    }

    #[test]
    fn program_uid_none_decodes_to_nothing_and_is_never_created() {
        assert_eq!(ProgramUid::NONE.site(64), None);
        assert!(ProgramUid::NONE.is_none());

        for origin in [
            ProgramOrigin::Seed,
            ProgramOrigin::Spawn,
            ProgramOrigin::Append,
        ] {
            let uid = ProgramUid::create(0, 0, 64, origin);
            assert!(!uid.is_none(), "created uids must never collide with NONE");
            assert!(uid.get() > 0);
        }
    }

    #[test]
    fn program_origin_labels_and_codes_agree() {
        for origin in [
            ProgramOrigin::Seed,
            ProgramOrigin::Spawn,
            ProgramOrigin::Append,
        ] {
            assert_eq!(ProgramOrigin::from_code(origin as u64), Some(origin));
        }
        assert_eq!(ProgramOrigin::Seed.label(), "seed");
        assert_eq!(ProgramOrigin::Spawn.label(), "spawn");
        assert_eq!(ProgramOrigin::Append.label(), "append");
        assert_eq!(ProgramOrigin::from_code(3), None);
    }

    #[test]
    fn lineage_child_increments_generation_and_defers_birth_tick() {
        let parent = ProgramUid::create(4, 2, 64, ProgramOrigin::Spawn);
        let child_uid = ProgramUid::create(9, 3, 64, ProgramOrigin::Append);
        let mut child = Lineage::child(child_uid, parent, 5);

        assert_eq!(child.uid, child_uid);
        assert_eq!(child.parent, parent);
        assert_eq!(child.generation, 6);
        assert_eq!(child.birth_tick, Lineage::NEVER_LIVE);
        assert_eq!(child.birth_tick(), None);

        child.mark_live(11);
        assert_eq!(child.birth_tick(), Some(11));
        assert_eq!(child.uid, child_uid);
        assert_eq!(child.parent, parent);
        assert_eq!(child.generation, 6);
    }

    #[test]
    fn lineage_child_saturates_generation_at_u32_max() {
        let child = Lineage::child(ProgramUid(9), ProgramUid(5), u32::MAX);
        assert_eq!(child.generation, u32::MAX);
    }

    #[test]
    fn lineage_root_has_no_parent_and_clamps_the_tick() {
        let root = Lineage::root(ProgramUid(9), 12);
        assert_eq!(root.parent, ProgramUid::NONE);
        assert_eq!(root.generation, 0);
        assert_eq!(root.birth_tick(), Some(12));

        // Past the u32 ceiling a birth tick clamps below the NEVER_LIVE sentinel,
        // so a very long run still reports "has been live".
        let ancient = Lineage::root(ProgramUid(9), u64::from(u32::MAX) + 500);
        assert_eq!(ancient.birth_tick(), Some(Lineage::NEVER_LIVE - 1));
    }

    #[test]
    fn program_lineage_defaults_to_none_until_attached() {
        let program =
            Program::new_live(vec![0x00], Direction::Right, 1).expect("program should build");
        assert_eq!(program.lineage, Lineage::default());
        assert!(program.lineage.uid.is_none());

        let lineage = Lineage::root(ProgramUid::create(0, 1, 8, ProgramOrigin::Seed), 0);
        let program = program.with_lineage(lineage);
        assert_eq!(program.lineage, lineage);
    }
}
