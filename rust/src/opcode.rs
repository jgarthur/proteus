//! Defines opcode decoding, metadata, and byte constants for program code.

/// Stores the number of spec-defined opcode meanings exposed by the decoder.
pub const SPEC_OPCODE_COUNT: usize = 71;

/// Represents one decoded instruction from program bytecode.
#[derive(Clone, Copy, Debug, PartialEq, Eq, strum::Display)]
#[strum(serialize_all = "camelCase")]
pub enum Opcode {
    #[strum(to_string = "push {0}")]
    PushLiteral(i16),
    Dup,
    Drop,
    Swap,
    Over,
    Rand,
    Add,
    Sub,
    Neg,
    Eq,
    Lt,
    Gt,
    Not,
    And,
    Or,
    For,
    Next,
    Jmp,
    JmpNz,
    JmpZ,
    Cw,
    Ccw,
    GetSize,
    GetIp,
    GetFlag,
    GetMsg,
    GetId,
    GetSrc,
    GetDst,
    SetDir,
    SetSrc,
    SetDst,
    SetId,
    GetE,
    GetM,
    Nop,
    Absorb,
    Listen,
    Collect,
    Emit,
    Read,
    Write,
    Del,
    Synthesize,
    SenseSize,
    SenseE,
    SenseM,
    SenseId,
    ReadAdj,
    WriteAdj,
    AppendAdj,
    DelAdj,
    GiveE,
    GiveM,
    Move,
    Boot,
    #[strum(to_string = "noop 0x{0:02x}")]
    NoOp(u8),
}

impl Opcode {
    /// Decodes one raw byte into the corresponding opcode representation.
    pub fn decode(byte: u8) -> Self {
        match byte {
            0x00..=0x0f => Self::PushLiteral(sign_extend_4bit(byte & 0x0f)),
            0x10 => Self::Dup,
            0x11 => Self::Drop,
            0x12 => Self::Swap,
            0x13 => Self::Over,
            0x14 => Self::Rand,
            0x20 => Self::Add,
            0x21 => Self::Sub,
            0x22 => Self::Neg,
            0x23 => Self::Eq,
            0x24 => Self::Lt,
            0x25 => Self::Gt,
            0x26 => Self::Not,
            0x27 => Self::And,
            0x28 => Self::Or,
            0x30 => Self::For,
            0x31 => Self::Next,
            0x32 => Self::Jmp,
            0x33 => Self::JmpNz,
            0x34 => Self::JmpZ,
            0x40 => Self::Cw,
            0x41 => Self::Ccw,
            0x42 => Self::GetSize,
            0x43 => Self::GetIp,
            0x44 => Self::GetFlag,
            0x45 => Self::GetMsg,
            0x46 => Self::GetId,
            0x47 => Self::GetSrc,
            0x48 => Self::GetDst,
            0x49 => Self::SetDir,
            0x4a => Self::SetSrc,
            0x4b => Self::SetDst,
            0x4c => Self::SetId,
            0x4d => Self::GetE,
            0x4e => Self::GetM,
            0x50 => Self::Nop,
            0x51 => Self::Absorb,
            0x52 => Self::Listen,
            0x53 => Self::Collect,
            0x54 => Self::Emit,
            0x55 => Self::Read,
            0x56 => Self::Write,
            0x57 => Self::Del,
            0x58 => Self::Synthesize,
            0x59 => Self::SenseSize,
            0x5a => Self::SenseE,
            0x5b => Self::SenseM,
            0x5c => Self::SenseId,
            0x5d => Self::ReadAdj,
            0x5e => Self::WriteAdj,
            0x5f => Self::AppendAdj,
            0x60 => Self::DelAdj,
            0x61 => Self::GiveE,
            0x62 => Self::GiveM,
            0x63 => Self::Move,
            0x64 => Self::Boot,
            other => Self::NoOp(other),
        }
    }

    /// Classifies whether an opcode resolves locally or in Pass 2.
    pub fn locality(self) -> Locality {
        match self {
            Self::ReadAdj
            | Self::WriteAdj
            | Self::AppendAdj
            | Self::DelAdj
            | Self::GiveE
            | Self::GiveM
            | Self::Move
            | Self::Boot => Locality::Nonlocal,
            _ => Locality::Local,
        }
    }

    /// Returns the base energy cost that must be paid during Pass 1.
    pub fn base_cost(self) -> u32 {
        match self {
            Self::Emit
            | Self::Write
            | Self::Del
            | Self::Synthesize
            | Self::WriteAdj
            | Self::AppendAdj
            | Self::DelAdj
            | Self::GiveM
            | Self::Move => 1,
            _ => 0,
        }
    }

    /// Returns any extra success-only resource cost tied to the opcode.
    pub fn additional_cost(self) -> AdditionalCost {
        match self {
            Self::Synthesize => AdditionalCost::FixedEnergySymbolic,
            Self::AppendAdj => AdditionalCost::FixedMass(1),
            Self::DelAdj => AdditionalCost::TargetStrengthEnergy,
            _ => AdditionalCost::None,
        }
    }

    /// Reports whether the decoded opcode is a reserved no-op byte.
    pub fn is_noop(self) -> bool {
        matches!(self, Self::NoOp(_))
    }
}

/// Splits opcodes into Pass-1-local and Pass-2-nonlocal classes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Locality {
    Local,
    Nonlocal,
}

/// Describes the success-only extra resource cost for an opcode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AdditionalCost {
    None,
    FixedEnergySymbolic,
    FixedMass(u32),
    TargetStrengthEnergy,
}

/// Byte-level constants for every opcode, usable in `vec![]` and `&[..]` contexts.
///
/// ```
/// use proteus::op::*;
/// let program = vec![push(2), EMIT];     // instead of vec![0x02, 0x54]
/// let code: &[u8] = &[ABSORB, CW, NOP]; // instead of &[0x51, 0x40, 0x50]
/// ```
pub mod op {
    // Push literal: use the `push` const fn below.

    /// Encode a push-literal value (−8 ..= 7) to its byte.
    pub const fn push(n: i16) -> u8 {
        (n & 0x0f) as u8
    }

    // Stack
    pub const DUP: u8 = 0x10;
    pub const DROP: u8 = 0x11;
    pub const SWAP: u8 = 0x12;
    pub const OVER: u8 = 0x13;
    pub const RAND: u8 = 0x14;

    // Arithmetic / logic
    pub const ADD: u8 = 0x20;
    pub const SUB: u8 = 0x21;
    pub const NEG: u8 = 0x22;
    pub const EQ: u8 = 0x23;
    pub const LT: u8 = 0x24;
    pub const GT: u8 = 0x25;
    pub const NOT: u8 = 0x26;
    pub const AND: u8 = 0x27;
    pub const OR: u8 = 0x28;

    // Control flow
    pub const FOR: u8 = 0x30;
    pub const NEXT: u8 = 0x31;
    pub const JMP: u8 = 0x32;
    pub const JMP_NZ: u8 = 0x33;
    pub const JMP_Z: u8 = 0x34;

    // Direction / registers / local access
    pub const CW: u8 = 0x40;
    pub const CCW: u8 = 0x41;
    pub const GET_SIZE: u8 = 0x42;
    pub const GET_IP: u8 = 0x43;
    pub const GET_FLAG: u8 = 0x44;
    pub const GET_MSG: u8 = 0x45;
    pub const GET_ID: u8 = 0x46;
    pub const GET_SRC: u8 = 0x47;
    pub const GET_DST: u8 = 0x48;
    pub const SET_DIR: u8 = 0x49;
    pub const SET_SRC: u8 = 0x4a;
    pub const SET_DST: u8 = 0x4b;
    pub const SET_ID: u8 = 0x4c;
    pub const GET_E: u8 = 0x4d;
    pub const GET_M: u8 = 0x4e;

    // World interaction
    pub const NOP: u8 = 0x50;
    pub const ABSORB: u8 = 0x51;
    pub const LISTEN: u8 = 0x52;
    pub const COLLECT: u8 = 0x53;
    pub const EMIT: u8 = 0x54;
    pub const READ: u8 = 0x55;
    pub const WRITE: u8 = 0x56;
    pub const DEL: u8 = 0x57;
    pub const SYNTHESIZE: u8 = 0x58;
    pub const SENSE_SIZE: u8 = 0x59;
    pub const SENSE_E: u8 = 0x5a;
    pub const SENSE_M: u8 = 0x5b;
    pub const SENSE_ID: u8 = 0x5c;
    pub const READ_ADJ: u8 = 0x5d;
    pub const WRITE_ADJ: u8 = 0x5e;
    pub const APPEND_ADJ: u8 = 0x5f;
    pub const DEL_ADJ: u8 = 0x60;
    pub const GIVE_E: u8 = 0x61;
    pub const GIVE_M: u8 = 0x62;
    pub const MOVE: u8 = 0x63;
    pub const BOOT: u8 = 0x64;
}

/// Sign-extends a 4-bit literal into the VM stack value range.
fn sign_extend_4bit(value: u8) -> i16 {
    let nibble = value & 0x0f;
    if nibble & 0x08 == 0 {
        i16::from(nibble)
    } else {
        i16::from(nibble) - 16
    }
}

#[cfg(test)]
mod tests {
    use super::{AdditionalCost, Locality, Opcode, SPEC_OPCODE_COUNT};

    #[test]
    fn push_literals_use_signed_four_bit_encoding() {
        assert_eq!(Opcode::decode(0x00), Opcode::PushLiteral(0));
        assert_eq!(Opcode::decode(0x07), Opcode::PushLiteral(7));
        assert_eq!(Opcode::decode(0x08), Opcode::PushLiteral(-8));
        assert_eq!(Opcode::decode(0x0f), Opcode::PushLiteral(-1));
    }

    #[test]
    fn selected_world_opcodes_have_expected_metadata() {
        assert_eq!(Opcode::decode(0x54).base_cost(), 1);
        assert_eq!(
            Opcode::decode(0x58).additional_cost(),
            AdditionalCost::FixedEnergySymbolic
        );
        assert_eq!(
            Opcode::decode(0x5f).additional_cost(),
            AdditionalCost::FixedMass(1)
        );
        assert_eq!(
            Opcode::decode(0x60).additional_cost(),
            AdditionalCost::TargetStrengthEnergy
        );
        assert_eq!(Opcode::decode(0x63).locality(), Locality::Nonlocal);
        assert_eq!(Opcode::decode(0x59).locality(), Locality::Local);
    }

    #[test]
    fn unknown_byte_decodes_to_noop() {
        assert!(Opcode::decode(0xff).is_noop());
    }

    #[test]
    fn spec_opcode_count_matches_full_decode_table() {
        let count = (u8::MIN..=u8::MAX)
            .filter(|byte| !Opcode::decode(*byte).is_noop())
            .count();

        assert_eq!(count, SPEC_OPCODE_COUNT);
    }

    #[test]
    fn op_constants_roundtrip_through_decode() {
        use super::op::*;

        // Push literals
        for n in -8_i16..=7 {
            let byte = push(n);
            assert_eq!(Opcode::decode(byte), Opcode::PushLiteral(n), "push({n})");
        }

        // Every named constant must decode to the expected Opcode variant.
        let pairs: &[(u8, Opcode)] = &[
            (DUP, Opcode::Dup),
            (DROP, Opcode::Drop),
            (SWAP, Opcode::Swap),
            (OVER, Opcode::Over),
            (RAND, Opcode::Rand),
            (ADD, Opcode::Add),
            (SUB, Opcode::Sub),
            (NEG, Opcode::Neg),
            (EQ, Opcode::Eq),
            (LT, Opcode::Lt),
            (GT, Opcode::Gt),
            (NOT, Opcode::Not),
            (AND, Opcode::And),
            (OR, Opcode::Or),
            (FOR, Opcode::For),
            (NEXT, Opcode::Next),
            (JMP, Opcode::Jmp),
            (JMP_NZ, Opcode::JmpNz),
            (JMP_Z, Opcode::JmpZ),
            (CW, Opcode::Cw),
            (CCW, Opcode::Ccw),
            (GET_SIZE, Opcode::GetSize),
            (GET_IP, Opcode::GetIp),
            (GET_FLAG, Opcode::GetFlag),
            (GET_MSG, Opcode::GetMsg),
            (GET_ID, Opcode::GetId),
            (GET_SRC, Opcode::GetSrc),
            (GET_DST, Opcode::GetDst),
            (SET_DIR, Opcode::SetDir),
            (SET_SRC, Opcode::SetSrc),
            (SET_DST, Opcode::SetDst),
            (SET_ID, Opcode::SetId),
            (GET_E, Opcode::GetE),
            (GET_M, Opcode::GetM),
            (NOP, Opcode::Nop),
            (ABSORB, Opcode::Absorb),
            (LISTEN, Opcode::Listen),
            (COLLECT, Opcode::Collect),
            (EMIT, Opcode::Emit),
            (READ, Opcode::Read),
            (WRITE, Opcode::Write),
            (DEL, Opcode::Del),
            (SYNTHESIZE, Opcode::Synthesize),
            (SENSE_SIZE, Opcode::SenseSize),
            (SENSE_E, Opcode::SenseE),
            (SENSE_M, Opcode::SenseM),
            (SENSE_ID, Opcode::SenseId),
            (READ_ADJ, Opcode::ReadAdj),
            (WRITE_ADJ, Opcode::WriteAdj),
            (APPEND_ADJ, Opcode::AppendAdj),
            (DEL_ADJ, Opcode::DelAdj),
            (GIVE_E, Opcode::GiveE),
            (GIVE_M, Opcode::GiveM),
            (MOVE, Opcode::Move),
            (BOOT, Opcode::Boot),
        ];

        assert_eq!(
            pairs.len(),
            SPEC_OPCODE_COUNT - 16,
            "table should cover all non-push opcodes"
        );

        for &(byte, expected) in pairs {
            assert_eq!(Opcode::decode(byte), expected, "byte 0x{byte:02x}");
        }
    }
}
