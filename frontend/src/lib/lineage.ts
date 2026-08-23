/**
 * Lineage helpers for the cell inspector.
 *
 * A program uid encodes the site where the program was created (API-SPEC §12):
 *
 *   uid = 1 + ((tick * cell_count + cell_index) * ORIGIN_SLOTS + origin)
 *
 * so the creation tick, cell, and origin can be recovered client-side as long
 * as the grid size is known.
 */

/** Origin tags the API reports in the `origin` field. */
export type ProgramOrigin = 'seed' | 'spawn' | 'append';

/** Uid slots reserved per (tick, cell) so two origins never collide. */
const ORIGIN_SLOTS = 4;

/** Origin tag by its encoded low-bit code; slot 3 is reserved and unused. */
const ORIGIN_BY_CODE: readonly ProgramOrigin[] = ['seed', 'spawn', 'append'];

const ORIGIN_LABELS: Record<ProgramOrigin, string> = {
  seed: 'Seed',
  spawn: 'Spawn',
  append: 'Append',
};

/** Creation site decoded out of a program uid. */
export interface ProgramSite {
  tick: number;
  cellIndex: number;
  origin: ProgramOrigin;
}

/** Renders an API origin tag as a display label, or an em dash when absent. */
export function originLabel(origin: string | null | undefined): string {
  if (origin === null || origin === undefined) {
    return '—';
  }

  return ORIGIN_LABELS[origin as ProgramOrigin] ?? origin;
}

/**
 * Decodes the creation site a uid encodes, mirroring `ProgramUid::site` in
 * `rust/src/model.rs`: the origin resolves first, and a uid carrying the
 * reserved fourth origin code is rejected outright rather than reported with a
 * usable tick and cell.
 *
 * Returns null when the uid identifies no program (0), when the origin code is
 * the reserved one, when the grid size is unknown, or when the uid exceeds
 * `Number.MAX_SAFE_INTEGER` — past that point JSON parsing has already rounded
 * the u64 and the decode would be a guess.
 *
 * Hand-checked vectors (`cellCount` 4096, i.e. a 64x64 grid), verified against
 * this implementation and against the encoding in `rust/src/model.rs`:
 *
 * | uid                    | cellCount | result                                |
 * |------------------------|-----------|---------------------------------------|
 * | 0                      | 4096      | null (identifies no program)          |
 * | 1                      | 4096      | tick 0, cell 0, seed (code 0)         |
 * | 2                      | 4096      | tick 0, cell 0, spawn (code 1)        |
 * | 3                      | 4096      | tick 0, cell 0, append (code 2)       |
 * | 4                      | 4096      | null (reserved code 3)                |
 * | 8321                   | 4096      | tick 0, cell 2080, seed               |
 * | 16385                  | 4096      | tick 1, cell 0, seed                  |
 * | 3333235                | 4096      | tick 203, cell 1820, append           |
 * | 8321                   | 0         | null (grid size unknown)              |
 * | 9007199254740991       | 4096      | tick 549755813887, cell 4095, append  |
 * | 9007199254740992       | 4096      | null (past MAX_SAFE_INTEGER)          |
 */
export function decodeProgramSite(uid: number, cellCount: number): ProgramSite | null {
  if (!Number.isSafeInteger(uid) || uid <= 0) {
    return null;
  }

  if (!Number.isSafeInteger(cellCount) || cellCount <= 0) {
    return null;
  }

  const encoded = uid - 1;
  const origin: ProgramOrigin | undefined = ORIGIN_BY_CODE[encoded % ORIGIN_SLOTS];
  if (origin === undefined) {
    return null;
  }

  const slot = Math.floor(encoded / ORIGIN_SLOTS);

  return {
    tick: Math.floor(slot / cellCount),
    cellIndex: slot % cellCount,
    origin,
  };
}

/** Converts a flat cell index into grid coordinates, or null when out of range. */
export function cellIndexToCoordinates(
  cellIndex: number,
  gridWidth: number,
  gridHeight: number,
): { x: number; y: number } | null {
  if (gridWidth <= 0 || gridHeight <= 0) {
    return null;
  }

  if (!Number.isSafeInteger(cellIndex) || cellIndex < 0 || cellIndex >= gridWidth * gridHeight) {
    return null;
  }

  return { x: cellIndex % gridWidth, y: Math.floor(cellIndex / gridWidth) };
}
