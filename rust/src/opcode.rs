pub const SPEC_OPCODE_COUNT: usize = 71;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Opcode {
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
    NoOp(u8),
}

impl Opcode {
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

    pub fn additional_cost(self) -> AdditionalCost {
        match self {
            Self::Synthesize => AdditionalCost::FixedEnergySymbolic,
            Self::AppendAdj => AdditionalCost::FixedMass(1),
            Self::DelAdj => AdditionalCost::TargetStrengthEnergy,
            _ => AdditionalCost::None,
        }
    }

    pub fn is_noop(self) -> bool {
        matches!(self, Self::NoOp(_))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Locality {
    Local,
    Nonlocal,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AdditionalCost {
    None,
    FixedEnergySymbolic,
    FixedMass(u32),
    TargetStrengthEnergy,
}

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
}
