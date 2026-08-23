import { describe, expect, it } from 'vitest';
import { assemble, disassemble, disassembleByte, OPCODES, suggestMnemonics } from './opcodes';

/**
 * Transcription of the `Opcode::decode` match arms in `rust/src/opcode.rs`,
 * paired with the camelCase string `strum(serialize_all = "camelCase")`
 * produces for each variant. Kept as data so drift in either table is caught
 * by a failing assertion rather than by a surprised user.
 */
const RUST_DECODE_TABLE: ReadonlyArray<[number, string]> = [
  [0x10, 'dup'],
  [0x11, 'drop'],
  [0x12, 'swap'],
  [0x13, 'over'],
  [0x14, 'rand'],
  [0x20, 'add'],
  [0x21, 'sub'],
  [0x22, 'neg'],
  [0x23, 'eq'],
  [0x24, 'lt'],
  [0x25, 'gt'],
  [0x26, 'not'],
  [0x27, 'and'],
  [0x28, 'or'],
  [0x30, 'for'],
  [0x31, 'next'],
  [0x32, 'jmp'],
  [0x33, 'jmpNz'],
  [0x34, 'jmpZ'],
  [0x40, 'cw'],
  [0x41, 'ccw'],
  [0x42, 'getSize'],
  [0x43, 'getIp'],
  [0x44, 'getFlag'],
  [0x45, 'getMsg'],
  [0x46, 'getId'],
  [0x47, 'getSrc'],
  [0x48, 'getDst'],
  [0x49, 'setDir'],
  [0x4a, 'setSrc'],
  [0x4b, 'setDst'],
  [0x4c, 'setId'],
  [0x4d, 'getE'],
  [0x4e, 'getM'],
  [0x50, 'nop'],
  [0x51, 'absorb'],
  [0x52, 'listen'],
  [0x53, 'collect'],
  [0x54, 'emit'],
  [0x55, 'read'],
  [0x56, 'write'],
  [0x57, 'del'],
  [0x58, 'synthesize'],
  [0x59, 'senseSize'],
  [0x5a, 'senseE'],
  [0x5b, 'senseM'],
  [0x5c, 'senseId'],
  [0x5d, 'readAdj'],
  [0x5e, 'writeAdj'],
  [0x5f, 'appendAdj'],
  [0x60, 'delAdj'],
  [0x61, 'giveE'],
  [0x62, 'giveM'],
  [0x63, 'move'],
  [0x64, 'boot'],
];

/** `SPEC_OPCODE_COUNT` in `rust/src/opcode.rs`: 16 push literals + 55 named. */
const SPEC_OPCODE_COUNT = 71;

const ALL_BYTES = Array.from({ length: 256 }, (_, byte) => byte);

describe('opcode table', () => {
  it('matches the Rust decode table byte for byte', () => {
    expect(OPCODES.map((entry) => [entry.byte, entry.mnemonic])).toEqual(
      RUST_DECODE_TABLE.map(([byte, mnemonic]) => [byte, mnemonic]),
    );
  });

  it('covers exactly the spec opcode count', () => {
    expect(OPCODES.length + 16).toBe(SPEC_OPCODE_COUNT);
  });

  it('gives every opcode a non-empty summary', () => {
    OPCODES.forEach((entry) => {
      expect(entry.summary.length).toBeGreaterThan(0);
    });
  });

  it('leaves every other byte as a noop', () => {
    const defined = new Set(RUST_DECODE_TABLE.map(([byte]) => byte));
    ALL_BYTES.filter((byte) => byte > 0x0f && !defined.has(byte)).forEach((byte) => {
      expect(disassembleByte(byte)).toBe(`noop 0x${byte.toString(16).padStart(2, '0')}`);
    });
  });
});

describe('disassembleByte', () => {
  it('sign-extends push literals', () => {
    expect(disassembleByte(0x00)).toBe('push 0');
    expect(disassembleByte(0x07)).toBe('push 7');
    expect(disassembleByte(0x08)).toBe('push -8');
    expect(disassembleByte(0x0f)).toBe('push -1');
  });

  it('renders undefined bytes as two lowercase hex digits', () => {
    expect(disassembleByte(0x15)).toBe('noop 0x15');
    expect(disassembleByte(0xff)).toBe('noop 0xff');
    expect(disassembleByte(0x65)).toBe('noop 0x65');
  });

  it('renders every named opcode with its camelCase mnemonic', () => {
    RUST_DECODE_TABLE.forEach(([byte, mnemonic]) => {
      expect(disassembleByte(byte)).toBe(mnemonic);
    });
  });
});

describe('assemble', () => {
  it('round-trips every byte through disassembly', () => {
    const result = assemble(disassemble(ALL_BYTES).join('\n'));
    expect(result.errors).toEqual([]);
    expect(result.code).toEqual(ALL_BYTES);
  });

  it('round-trips each byte individually', () => {
    ALL_BYTES.forEach((byte) => {
      expect(assemble(disassembleByte(byte)).code).toEqual([byte]);
    });
  });

  it('encodes push literals in two-complement nibbles', () => {
    expect(assemble('push -8').code).toEqual([0x08]);
    expect(assemble('push -1').code).toEqual([0x0f]);
    expect(assemble('push 7').code).toEqual([0x07]);
    expect(assemble('push 0').code).toEqual([0x00]);
  });

  it('accepts bare negative literals as pushes and bare bytes as raw code', () => {
    expect(assemble('-8 -1 9 0x51 255').code).toEqual([0x08, 0x0f, 9, 0x51, 255]);
  });

  it('accepts commas, extra whitespace, and comments', () => {
    const result = assemble('absorb, collect ; harvest\n  cw\n\npush 0 , setSrc');
    expect(result.errors).toEqual([]);
    expect(result.code).toEqual([0x51, 0x53, 0x40, 0x00, 0x4a]);
  });

  it('matches mnemonics case-insensitively', () => {
    expect(assemble('ABSORB AppendAdj JMPNZ').code).toEqual([0x51, 0x5f, 0x33]);
  });

  it('assembles a plain decimal byte list', () => {
    const bytes = [81, 83, 64, 0, 74, 66, 48, 85, 95, 49, 100, 80];
    expect(assemble(bytes.join(', ')).code).toEqual(bytes);
  });

  it('reports unknown tokens with line, token, and suggestions', () => {
    const result = assemble('absorb\nabsrob\ncollect');
    expect(result.code).toEqual([0x51, 0x53]);
    expect(result.errors).toHaveLength(1);
    expect(result.errors[0]!.line).toBe(2);
    expect(result.errors[0]!.token).toBe('absrob');
    expect(result.errors[0]!.message).toBe('Unknown instruction');
    expect(result.errors[0]!.suggestions).toContain('absorb');
    expect(result.errors[0]!.suggestions.length).toBeLessThanOrEqual(3);
  });

  it('rejects out-of-range push operands', () => {
    const result = assemble('push 8');
    expect(result.code).toEqual([]);
    expect(result.errors[0]!.token).toBe('8');
  });

  it('reports a dangling push with no operand', () => {
    const result = assemble('absorb push');
    expect(result.code).toEqual([0x51]);
    expect(result.errors).toHaveLength(1);
    expect(result.errors[0]!.token).toBe('push');
  });

  it('rejects out-of-range noop operands', () => {
    const result = assemble('noop 0x1ff');
    expect(result.code).toEqual([]);
    expect(result.errors).toHaveLength(1);
  });
});

describe('suggestMnemonics', () => {
  it('suggests by prefix', () => {
    expect(suggestMnemonics('sense')).toEqual(['senseE', 'senseM', 'senseId']);
  });

  it('returns nothing useful for gibberish', () => {
    expect(suggestMnemonics('qqqqqqqqqqqq')).toEqual([]);
  });
});
