/**
 * Client-side mirror of the engine opcode table (`rust/src/opcode.rs`).
 *
 * `disassembleByte` must render exactly what `Opcode::decode(byte).to_string()`
 * renders, because the inspector shows backend `disassembly` strings and those
 * strings have to paste straight back into {@link assemble}. Mnemonics are the
 * camelCase forms that `strum(serialize_all = "camelCase")` derives from the
 * Rust variant names, and undefined bytes render as `noop 0xNN`.
 */

export interface OpcodeInfo {
  byte: number;
  mnemonic: string;
  summary: string;
}

/**
 * Every non-push opcode, in byte order.
 *
 * Push literals (`0x00..=0x0f`) are excluded: they carry an operand and are
 * handled by {@link disassembleByte} directly. Summaries are condensed from the
 * `docs/SPEC.md` instruction tables.
 */
export const OPCODES: ReadonlyArray<OpcodeInfo> = [
  // Stack
  { byte: 0x10, mnemonic: 'dup', summary: 'Duplicate top of stack.' },
  { byte: 0x11, mnemonic: 'drop', summary: 'Remove top of stack.' },
  { byte: 0x12, mnemonic: 'swap', summary: 'Swap top two stack values.' },
  { byte: 0x13, mnemonic: 'over', summary: 'Copy second element to top of stack.' },
  { byte: 0x14, mnemonic: 'rand', summary: 'Push a random value 0-255.' },

  // Arithmetic / logic
  { byte: 0x20, mnemonic: 'add', summary: 'Push second + top.' },
  { byte: 0x21, mnemonic: 'sub', summary: 'Push second - top.' },
  { byte: 0x22, mnemonic: 'neg', summary: 'Push -top.' },
  { byte: 0x23, mnemonic: 'eq', summary: 'Push 1 if second == top, else 0.' },
  { byte: 0x24, mnemonic: 'lt', summary: 'Push 1 if second < top, else 0.' },
  { byte: 0x25, mnemonic: 'gt', summary: 'Push 1 if second > top, else 0.' },
  { byte: 0x26, mnemonic: 'not', summary: 'Push 1 if top is 0, else 0.' },
  { byte: 0x27, mnemonic: 'and', summary: 'Push 1 if both operands are nonzero, else 0.' },
  { byte: 0x28, mnemonic: 'or', summary: 'Push 1 if either operand is nonzero, else 0.' },

  // Control flow
  { byte: 0x30, mnemonic: 'for', summary: 'Pop count into LC; if LC <= 0, skip past the matching next.' },
  { byte: 0x31, mnemonic: 'next', summary: 'Decrement LC; if LC > 0, jump back after the matching for.' },
  { byte: 0x32, mnemonic: 'jmp', summary: 'Pop offset; jump to IP + offset.' },
  { byte: 0x33, mnemonic: 'jmpNZ', summary: 'Pop value then offset; jump if value != 0.' },
  { byte: 0x34, mnemonic: 'jmpZ', summary: 'Pop value then offset; jump if value == 0.' },

  // Direction, registers, local resource access
  { byte: 0x40, mnemonic: 'cw', summary: 'Rotate Dir 90 degrees clockwise.' },
  { byte: 0x41, mnemonic: 'ccw', summary: 'Rotate Dir 90 degrees counterclockwise.' },
  { byte: 0x42, mnemonic: 'getSize', summary: "Push this program's instruction count." },
  { byte: 0x43, mnemonic: 'getIP', summary: 'Push IP.' },
  { byte: 0x44, mnemonic: 'getFlag', summary: 'Push Flag (0 or 1).' },
  { byte: 0x45, mnemonic: 'getMsg', summary: 'Push Msg.' },
  { byte: 0x46, mnemonic: 'getID', summary: 'Push ID.' },
  { byte: 0x47, mnemonic: 'getSrc', summary: 'Push Src.' },
  { byte: 0x48, mnemonic: 'getDst', summary: 'Push Dst.' },
  { byte: 0x49, mnemonic: 'setDir', summary: 'Pop value; set Dir to value mod 4.' },
  { byte: 0x4a, mnemonic: 'setSrc', summary: 'Pop value; set Src.' },
  { byte: 0x4b, mnemonic: 'setDst', summary: 'Pop value; set Dst.' },
  { byte: 0x4c, mnemonic: 'setID', summary: 'Pop value; set ID (low 8 bits).' },
  { byte: 0x4d, mnemonic: 'getE', summary: "Push this cell's free energy (excludes background)." },
  { byte: 0x4e, mnemonic: 'getM', summary: "Push this cell's free mass." },

  // World interaction - local
  { byte: 0x50, mnemonic: 'nop', summary: 'No operation. Opens the cell.' },
  {
    byte: 0x51,
    mnemonic: 'absorb',
    summary: 'Mark for background-radiation harvest; repeats widen the footprint (cap 4).',
  },
  { byte: 0x52, mnemonic: 'listen', summary: 'Capture directed radiation in Pass 3. Opens the cell.' },
  { byte: 0x53, mnemonic: 'collect', summary: "Convert this cell's background mass to free mass in Pass 3." },
  { byte: 0x54, mnemonic: 'emit', summary: 'Pop message; send a 1-energy directed packet along Dir. Cost 1.' },
  { byte: 0x55, mnemonic: 'read', summary: 'Push self[Src mod size]; increment Src.' },
  { byte: 0x56, mnemonic: 'write', summary: 'Pop value; overwrite self[Dst mod size]; increment Dst. Cost 1.' },
  { byte: 0x57, mnemonic: 'del', summary: 'Delete self[Dst mod size], freeing 1 mass. Cost 1; fails at size 1.' },
  { byte: 0x58, mnemonic: 'synthesize', summary: 'Spend N_synth free energy to make 1 free mass. Cost 1.' },

  // World interaction - local sensing
  { byte: 0x59, mnemonic: 'senseSize', summary: 'Push neighbor program size along Dir (0 if empty).' },
  { byte: 0x5a, mnemonic: 'senseE', summary: 'Push neighbor free energy along Dir (0 if empty).' },
  { byte: 0x5b, mnemonic: 'senseM', summary: 'Push neighbor free mass along Dir (0 if empty).' },
  { byte: 0x5c, mnemonic: 'senseID', summary: 'Push neighbor ID along Dir (0 and Flag = 1 if empty).' },

  // World interaction - nonlocal
  { byte: 0x5d, mnemonic: 'readAdj', summary: 'Push neighbor[Src mod size]; increment Src. Ignores protection.' },
  {
    byte: 0x5e,
    mnemonic: 'writeAdj',
    summary: 'Pop value; overwrite neighbor[Dst mod size]. Cost 1; needs an open target.',
  },
  {
    byte: 0x5f,
    mnemonic: 'appendAdj',
    summary: 'Pop value; append to the neighbor (creates an inert program if empty). Cost 1 + 1 mass.',
  },
  {
    byte: 0x60,
    mnemonic: 'delAdj',
    summary: "Delete neighbor[Dst mod size]; costs the target's strength in energy. Cost 1.",
  },
  { byte: 0x61, mnemonic: 'giveE', summary: 'Pop amount; transfer that much free energy to the neighbor. Cost 0.' },
  { byte: 0x62, mnemonic: 'giveM', summary: 'Pop amount; transfer that much free mass to the neighbor. Cost 1.' },
  {
    byte: 0x63,
    mnemonic: 'move',
    summary: 'Relocate this program and its free resources into an empty neighbor. Cost 1.',
  },
  { byte: 0x64, mnemonic: 'boot', summary: 'Make an inert neighbor live from the next tick. Cost 0.' },
];

const MNEMONIC_BY_BYTE = new Map<number, string>(OPCODES.map((entry) => [entry.byte, entry.mnemonic]));

const BYTE_BY_LOWER_MNEMONIC = new Map<string, number>(
  OPCODES.map((entry) => [entry.mnemonic.toLowerCase(), entry.byte]),
);

/** Highest byte value that still decodes as a push literal. */
const PUSH_LITERAL_MAX_BYTE = 0x0f;

/** Sign-extends the low nibble, matching `sign_extend_4bit` in the engine. */
function signExtend4Bit(value: number): number {
  const nibble = value & 0x0f;
  return nibble & 0x08 ? nibble - 16 : nibble;
}

/** Renders one byte the way `Opcode::decode(byte).to_string()` does. */
export function disassembleByte(byte: number): string {
  const value = byte & 0xff;
  if (value <= PUSH_LITERAL_MAX_BYTE) {
    return `push ${signExtend4Bit(value)}`;
  }

  const mnemonic = MNEMONIC_BY_BYTE.get(value);
  if (mnemonic !== undefined) {
    return mnemonic;
  }

  return `noop 0x${value.toString(16).padStart(2, '0')}`;
}

/** Renders a whole program, one instruction string per byte. */
export function disassemble(code: readonly number[]): string[] {
  return code.map(disassembleByte);
}

export interface AsmError {
  line: number;
  token: string;
  message: string;
  suggestions: string[];
}

interface Token {
  text: string;
  line: number;
}

/** Splits source text into tokens, dropping `;` comments. */
function tokenize(text: string): Token[] {
  const tokens: Token[] = [];
  text.split('\n').forEach((rawLine, index) => {
    const commentStart = rawLine.indexOf(';');
    const line = commentStart >= 0 ? rawLine.slice(0, commentStart) : rawLine;
    line
      .split(/[\s,]+/)
      .filter((part) => part.length > 0)
      .forEach((part) => tokens.push({ text: part, line: index + 1 }));
  });
  return tokens;
}

const DECIMAL_PATTERN = /^[+-]?\d+$/;
const HEX_PATTERN = /^[+-]?0x[0-9a-f]+$/i;

/** Parses a decimal or `0x` literal, or returns null if the token is not numeric. */
function parseNumericToken(token: string): number | null {
  if (HEX_PATTERN.test(token)) {
    const negative = token.startsWith('-');
    const digits = token.replace(/^[+-]/, '').slice(2);
    const magnitude = Number.parseInt(digits, 16);
    return negative ? -magnitude : magnitude;
  }
  if (DECIMAL_PATTERN.test(token)) {
    return Number.parseInt(token, 10);
  }
  return null;
}

/** Standard Levenshtein distance, used to rank mnemonic suggestions. */
function editDistance(a: string, b: string): number {
  let previous = Array.from({ length: b.length + 1 }, (_, index) => index);
  for (let i = 1; i <= a.length; i += 1) {
    const current = [i];
    for (let j = 1; j <= b.length; j += 1) {
      const substitution = previous[j - 1]! + (a[i - 1] === b[j - 1] ? 0 : 1);
      current[j] = Math.min(substitution, previous[j]! + 1, current[j - 1]! + 1);
    }
    previous = current;
  }
  return previous[b.length]!;
}

const SUGGESTION_CANDIDATES: readonly string[] = ['push', 'noop', ...OPCODES.map((entry) => entry.mnemonic)];

/** Returns up to three mnemonics close to `token` by edit distance or prefix. */
export function suggestMnemonics(token: string): string[] {
  const needle = token.toLowerCase();
  if (!needle) {
    return [];
  }

  return SUGGESTION_CANDIDATES.map((candidate) => ({
    candidate,
    distance: editDistance(needle, candidate.toLowerCase()),
    prefix: candidate.toLowerCase().startsWith(needle),
  }))
    .filter((entry) => entry.distance <= 2 || entry.prefix)
    .sort((left, right) =>
      left.distance !== right.distance
        ? left.distance - right.distance
        : left.candidate.localeCompare(right.candidate),
    )
    .slice(0, 3)
    .map((entry) => entry.candidate);
}

export interface AsmResult {
  code: number[];
  errors: AsmError[];
}

/**
 * Assembles mnemonic text into bytes.
 *
 * Accepted tokens: any mnemonic (case-insensitive); `push N` with N in -8..=7,
 * or a bare -8..=-1; `noop 0xNN` for a raw byte; and a bare decimal 0..=255 or
 * `0xNN` hex literal, so a plain byte list assembles unchanged. Unknown tokens
 * become errors carrying suggestions, and assembly continues past them.
 */
export function assemble(text: string): AsmResult {
  const tokens = tokenize(text);
  const code: number[] = [];
  const errors: AsmError[] = [];

  const recordError = (token: Token, message: string, suggestions: string[] = []) => {
    errors.push({ line: token.line, token: token.text, message, suggestions });
  };

  for (let index = 0; index < tokens.length; index += 1) {
    const token = tokens[index]!;
    const lower = token.text.toLowerCase();

    if (lower === 'push') {
      const operand = tokens[index + 1];
      if (!operand) {
        recordError(token, 'push needs a literal operand from -8 to 7');
        continue;
      }
      index += 1;
      const value = parseNumericToken(operand.text);
      if (value === null || value < -8 || value > 7) {
        recordError(operand, 'push operand must be an integer from -8 to 7');
        continue;
      }
      code.push(value & 0x0f);
      continue;
    }

    if (lower === 'noop') {
      const operand = tokens[index + 1];
      if (!operand) {
        recordError(token, 'noop needs a byte operand from 0 to 255');
        continue;
      }
      index += 1;
      const value = parseNumericToken(operand.text);
      if (value === null || value < 0 || value > 255) {
        recordError(operand, 'noop operand must be a byte from 0 to 255');
        continue;
      }
      code.push(value);
      continue;
    }

    const mnemonicByte = BYTE_BY_LOWER_MNEMONIC.get(lower);
    if (mnemonicByte !== undefined) {
      code.push(mnemonicByte);
      continue;
    }

    const numeric = parseNumericToken(token.text);
    if (numeric !== null) {
      if (numeric >= -8 && numeric <= -1) {
        code.push(numeric & 0x0f);
        continue;
      }
      if (numeric >= 0 && numeric <= 255) {
        code.push(numeric);
        continue;
      }
      recordError(token, 'Byte literals must be -8..-1 (push) or 0..255');
      continue;
    }

    recordError(token, 'Unknown instruction', suggestMnemonics(token.text));
  }

  return { code, errors };
}
