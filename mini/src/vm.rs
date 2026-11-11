//! The interpreter: one instruction pointer and two heads on one tape.
//!
//! Code and data are the same bytes. There is no stack and nothing else to
//! address: the tape is the whole machine state apart from the three cursors,
//! which exist only while a run lasts.

/// The meaning a byte takes when the instruction pointer reads it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    /// Any byte the instruction set does not name.
    Nop,
    /// `<` moves head 0 one byte left.
    Head0Left,
    /// `>` moves head 0 one byte right.
    Head0Right,
    /// `{` moves head 1 one byte left.
    Head1Left,
    /// `}` moves head 1 one byte right.
    Head1Right,
    /// `-` decrements the byte under head 0.
    Dec,
    /// `+` increments the byte under head 0.
    Inc,
    /// `.` copies the byte under head 0 to head 1.
    Put,
    /// `,` copies the byte under head 1 to head 0.
    Get,
    /// `[` skips past the matching `]` when the byte under head 0 is zero.
    LoopStart,
    /// `]` returns to the matching `[` when the byte under head 0 is nonzero.
    LoopEnd,
    /// `:` copies the byte under head 0 to head 1, then advances both heads.
    Copy,
}

/// Every instruction the interpreter knows, with the character that spells it.
pub const ALPHABET: [(char, Op); 11] = [
    ('<', Op::Head0Left),
    ('>', Op::Head0Right),
    ('{', Op::Head1Left),
    ('}', Op::Head1Right),
    ('-', Op::Dec),
    ('+', Op::Inc),
    ('.', Op::Put),
    (',', Op::Get),
    ('[', Op::LoopStart),
    (']', Op::LoopEnd),
    (':', Op::Copy),
];

/// The ten instructions of BFF (Agüera y Arcas et al., arXiv:2406.19108).
pub const BFF: &str = "<>{}-+.,[]";

/// An instruction set: the meaning of each of the 256 byte values.
#[derive(Clone)]
pub struct OpTable([Op; 256]);

impl OpTable {
    /// Builds a table that enables exactly the listed instruction characters.
    pub fn parse(ops: &str) -> Result<Self, String> {
        let mut table = [Op::Nop; 256];
        for c in ops.chars() {
            let Some(&(_, op)) = ALPHABET.iter().find(|(name, _)| *name == c) else {
                let known: String = ALPHABET.iter().map(|(name, _)| *name).collect();
                return Err(format!("unknown instruction {c:?}; known: {known}"));
            };
            table[c as usize] = op;
        }
        Ok(Self(table))
    }

    /// Returns what a byte means under this instruction set.
    pub fn get(&self, byte: u8) -> Op {
        self.0[byte as usize]
    }

    /// Reports whether a byte is an instruction, not inert data.
    pub fn is_op(&self, byte: u8) -> bool {
        self.get(byte) != Op::Nop
    }
}

/// Counts what one run did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Run {
    /// Bytes the instruction pointer read, instructions and inert data alike.
    pub reads: u32,
    /// Reads that were instructions.
    pub ops: u32,
}

/// Runs the tape as a program until it halts or has read `steps` bytes.
///
/// The instruction pointer and head 0 start at byte 0 and head 1 at `head1`.
/// Heads wrap around the tape; the instruction pointer does not. A run halts
/// when the pointer leaves the tape or meets a bracket that has no partner.
pub fn run(tape: &mut [u8], table: &OpTable, steps: u32, head1: usize) -> Run {
    let len = tape.len();
    let left = |head: usize| if head == 0 { len - 1 } else { head - 1 };
    let right = |head: usize| if head + 1 == len { 0 } else { head + 1 };
    let (mut ip, mut head0, mut head1) = (0, 0, head1 % len);
    let mut run = Run::default();

    while run.reads < steps && ip < len {
        let op = table.get(tape[ip]);
        run.reads += 1;
        run.ops += u32::from(op != Op::Nop);
        match op {
            Op::Nop => {}
            Op::Head0Left => head0 = left(head0),
            Op::Head0Right => head0 = right(head0),
            Op::Head1Left => head1 = left(head1),
            Op::Head1Right => head1 = right(head1),
            Op::Dec => tape[head0] = tape[head0].wrapping_sub(1),
            Op::Inc => tape[head0] = tape[head0].wrapping_add(1),
            Op::Put => tape[head1] = tape[head0],
            Op::Get => tape[head0] = tape[head1],
            Op::Copy => {
                tape[head1] = tape[head0];
                head0 = right(head0);
                head1 = right(head1);
            }
            Op::LoopStart if tape[head0] == 0 => match matching_end(tape, table, ip) {
                Some(end) => ip = end,
                None => break,
            },
            Op::LoopEnd if tape[head0] != 0 => match matching_start(tape, table, ip) {
                Some(start) => ip = start,
                None => break,
            },
            Op::LoopStart | Op::LoopEnd => {}
        }
        ip += 1;
    }
    run
}

/// Finds the `]` that closes the `[` at `start`, scanning the live tape.
fn matching_end(tape: &[u8], table: &OpTable, start: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (ip, &byte) in tape.iter().enumerate().skip(start + 1) {
        match table.get(byte) {
            Op::LoopStart => depth += 1,
            Op::LoopEnd if depth == 0 => return Some(ip),
            Op::LoopEnd => depth -= 1,
            _ => {}
        }
    }
    None
}

/// Finds the `[` that opens the `]` at `end`, scanning the live tape.
fn matching_start(tape: &[u8], table: &OpTable, end: usize) -> Option<usize> {
    let mut depth = 0usize;
    for ip in (0..end).rev() {
        match table.get(tape[ip]) {
            Op::LoopEnd => depth += 1,
            Op::LoopStart if depth == 0 => return Some(ip),
            Op::LoopStart => depth -= 1,
            _ => {}
        }
    }
    None
}
