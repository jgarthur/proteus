//! The instruction set: eight instructions in a four-bit code.
//!
//! Eight instructions need three bits. The code has one bit more, so half of
//! all values are no-ops, and a mutation is as likely to silence an
//! instruction as to turn it into a different one.

/// Bits per instruction, and so also the width of the `A` register.
pub const BITS: u32 = 4;

/// How many distinct instruction values exist.
pub const VALUES: u8 = 1 << BITS;

/// The largest number the `A` register can hold.
pub const A_MAX: u8 = VALUES - 1;

/// What a program does on the tick it executes one instruction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    /// Take all ambient energy in this cell.
    Absorb,
    /// Take all ambient mass in this cell.
    Collect,
    /// Extend the body in the facing cell with the next instruction of this
    /// program's own code. Costs 1 energy and 1 mass.
    Copy,
    /// Turn to face the next cell clockwise.
    Turn,
    /// Set `A` to the size of the live program in the facing cell, or 0 if
    /// there is none.
    Look,
    /// Set `A` to the mass this program holds.
    Weigh,
    /// Subtract 1 from `A`, stopping at 0.
    Dec,
    /// Skip the next instruction if `A` is 0.
    Skip,
    /// Do nothing.
    Nop,
}

/// Each instruction with its value and the letter that spells it.
pub const TABLE: [(Op, char); 8] = [
    (Op::Absorb, 'A'),
    (Op::Collect, 'C'),
    (Op::Copy, 'K'),
    (Op::Turn, 'T'),
    (Op::Look, 'L'),
    (Op::Weigh, 'W'),
    (Op::Dec, 'D'),
    (Op::Skip, 'S'),
];

/// Returns what an instruction value means.
pub fn decode(value: u8) -> Op {
    TABLE.get(value as usize).map_or(Op::Nop, |&(op, _)| op)
}

/// Spells code as one letter per instruction, with `.` for every no-op.
pub fn spell(code: &[u8]) -> String {
    code.iter()
        .map(|&value| TABLE.get(value as usize).map_or('.', |&(_, letter)| letter))
        .collect()
}

/// Reads code spelled by [`spell`]. A `.` becomes the first no-op value.
pub fn parse(text: &str) -> Result<Vec<u8>, String> {
    text.chars()
        .map(
            |c| match TABLE.iter().position(|&(_, letter)| letter == c) {
                Some(value) => Ok(value as u8),
                None if c == '.' => Ok(TABLE.len() as u8),
                None => Err(format!("unknown instruction letter {c:?} in {text:?}")),
            },
        )
        .collect()
}

/// Reports whether code contains an instruction.
pub fn has(code: &[u8], op: Op) -> bool {
    code.iter().any(|&value| decode(value) == op)
}
