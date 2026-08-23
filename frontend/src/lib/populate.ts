/**
 * Pure allocation and scattering logic behind the config editor's population
 * composer: turn a total plus per-organism weights into exact counts, then
 * place those organisms on distinct free cells, deterministically from the
 * config seed.
 *
 * Nothing here touches React or `SimConfig`; the editor calls these functions
 * and writes the result into `seed_programs`.
 */

import type { SeedProgram } from '../types';
import type { SeedOrganism } from './seedLibrary';

export interface ComposerRow {
  organismId: string;
  active: boolean;
  /** Relative share of the total. Non-negative; integer or decimal. */
  weight: number;
}

export interface ComposerState {
  total: number;
  rows: ComposerRow[];
}

/**
 * Everything the population composer remembers between renders, including the
 * entries it generated. This lives beside the draft `SimConfig` rather than in
 * the editor component, because the editor unmounts whenever the sidebar
 * collapses or switches to the Inspector tab.
 */
export interface ComposerSession extends ComposerState {
  /** Entries the composer placed, held by identity, not by index or tag. */
  generated: SeedProgram[];
  /** Free-cell count from the last clamped scatter, else null. */
  clampedTo: number | null;
}

/** Builds a fresh composer session with every organism active at weight 1. */
export function createComposerSession(organismIds: readonly string[]): ComposerSession {
  return {
    total: 1,
    rows: organismIds.map((organismId) => ({ organismId, active: true, weight: 1 })),
    generated: [],
    clampedTo: null,
  };
}

/**
 * Narrows remembered generated entries to the ones still present in the config.
 *
 * Membership is by object identity: editing an entry replaces it with a new
 * object and removing one drops it entirely, so either way it stops counting as
 * composer-generated. Deriving this at every use is what keeps a removed or
 * replaced entry from being resurrected by the next scatter.
 */
export function liveGenerated(
  generated: readonly SeedProgram[],
  seedPrograms: readonly SeedProgram[],
): SeedProgram[] {
  const present = new Set<SeedProgram>(seedPrograms);
  return generated.filter((program) => present.has(program));
}

/**
 * Upper bound on a composer weight.
 *
 * Weights only ever express a ratio, so there is no reason to allow a value
 * large enough to overflow the apportionment arithmetic or to make every other
 * row round to zero.
 */
export const MAX_COMPOSER_WEIGHT = 1e6;

/**
 * Upper bound on the composer total: the largest grid the config editor allows
 * is 1024 x 1024, so nothing above this could ever be placed anyway.
 */
export const MAX_COMPOSER_TOTAL = 1024 * 1024;

/**
 * Fixed-point scale for weights during apportionment.
 *
 * Weights are rounded onto this grid and compared as exact integers, which is
 * what makes equal weights produce bit-identical remainders (and therefore a
 * genuine tie, broken by row order) instead of the near-misses that raw float
 * remainders produce.
 */
const WEIGHT_SCALE = 2 ** 20;

/** Formats a grid coordinate as the `"x,y"` key used for occupancy sets. */
export function cellKey(x: number, y: number): string {
  return `${x},${y}`;
}

/**
 * Splits `total` across the active, positively-weighted rows by the
 * Hamilton (largest-remainder) method: each row takes the floor of its exact
 * quota, then the leftover units go to the largest fractional remainders, ties
 * broken by row order. The result always sums to `total` exactly, and equal
 * weights come out as even as the total allows. Deterministic; no RNG.
 *
 * Rows that are inactive or carry weight <= 0 are dropped entirely and do not
 * appear in the returned map. Rows sharing an organism id are summed.
 */
export function allocateCounts(total: number, rows: readonly ComposerRow[]): Map<string, number> {
  const eligible = rows.filter((row) => row.active && row.weight > 0 && Number.isFinite(row.weight));
  const counts = new Map<string, number>();
  if (eligible.length === 0) {
    return counts;
  }

  const wanted = Number.isFinite(total) ? Math.max(0, Math.floor(total)) : 0;

  // Apportion in exact integers. Floats make equal weights land on remainders
  // that differ in the last bit, which turns a tie into an arbitrary winner and
  // loses a unit to the wrong row; BigInt keeps the products exact no matter
  // how large the total or the weights are.
  // Tiny positive weights still count as eligible: floor at one scale unit so a
  // row the user left active can never round to nothing.
  const scaled = eligible.map((row) =>
    BigInt(Math.max(1, Math.round(Math.min(row.weight, MAX_COMPOSER_WEIGHT) * WEIGHT_SCALE))),
  );
  const weightSum = scaled.reduce((sum, weight) => sum + weight, 0n);
  if (weightSum === 0n) {
    eligible.forEach((row) => counts.set(row.organismId, (counts.get(row.organismId) ?? 0) + 0));
    return counts;
  }

  const wantedBig = BigInt(wanted);
  const quotas = scaled.map((weight, index) => {
    const numerator = wantedBig * weight;
    const base = numerator / weightSum;
    return { index, base: Number(base), remainder: numerator - base * weightSum };
  });

  const assigned = quotas.reduce((sum, quota) => sum + quota.base, 0);
  let leftover = wanted - assigned;

  // Largest remainder first; equal remainders fall back to row order so the
  // allocation is a pure function of the inputs.
  const byRemainder = [...quotas].sort((left, right) => {
    if (left.remainder !== right.remainder) {
      return right.remainder > left.remainder ? 1 : -1;
    }
    return left.index - right.index;
  });

  for (const quota of byRemainder) {
    if (leftover <= 0) {
      break;
    }
    quota.base += 1;
    leftover -= 1;
  }

  quotas.forEach((quota) => {
    const row = eligible[quota.index]!;
    counts.set(row.organismId, (counts.get(row.organismId) ?? 0) + quota.base);
  });

  return counts;
}

/**
 * Folds a config seed (up to 53 significant bits in JavaScript) into the 32
 * bits mulberry32 wants, by XOR-ing the low 32 bits with the remaining high
 * bits. Every bit of the seed therefore influences the stream, and distinct
 * small seeds (the common case) stay distinct.
 */
export function foldSeed(seed: number): number {
  const magnitude = Number.isFinite(seed) ? Math.floor(Math.abs(seed)) : 0;
  const low = magnitude >>> 0;
  const high = Math.floor(magnitude / 2 ** 32) >>> 0;
  return (low ^ high) >>> 0;
}

/** mulberry32: a small, fast, well-distributed seeded 32-bit generator. */
export function mulberry32(seed: number): () => number {
  let state = seed >>> 0;
  return () => {
    state = (state + 0x6d2b79f5) >>> 0;
    let value = state;
    value = Math.imul(value ^ (value >>> 15), value | 1);
    value ^= value + Math.imul(value ^ (value >>> 7), value | 61);
    return ((value ^ (value >>> 14)) >>> 0) / 4294967296;
  };
}

export interface ScatterArgs {
  width: number;
  height: number;
  seed: number;
  counts: Map<string, number>;
  /** `"x,y"` keys of hand-placed entries, which are excluded from placement. */
  occupied: ReadonlySet<string>;
  library: readonly SeedOrganism[];
}

export interface ScatterResult {
  programs: SeedProgram[];
  /** Free-cell count when the request did not fit, otherwise null. */
  clampedTo: number | null;
}

/**
 * Places `counts` organisms on distinct free cells.
 *
 * Cells are drawn without replacement by a partial Fisher-Yates shuffle over
 * the free-cell list, so no cell can repeat and no rejection loop can stall on
 * a nearly-full grid. The organism assignment list (count copies of each
 * organism, in library order) is shuffled with the same stream and zipped with
 * the drawn cells, so output order is cell draw order.
 *
 * If the request exceeds the free-cell count, the totals are re-apportioned
 * over the same relative counts down to the number of free cells, and
 * `clampedTo` reports that number.
 */
export function scatter({ width, height, seed, counts, occupied, library }: ScatterArgs): ScatterResult {
  const gridWidth = Math.max(0, Math.floor(width));
  const gridHeight = Math.max(0, Math.floor(height));

  const free: Array<[number, number]> = [];
  for (let y = 0; y < gridHeight; y += 1) {
    for (let x = 0; x < gridWidth; x += 1) {
      if (!occupied.has(cellKey(x, y))) {
        free.push([x, y]);
      }
    }
  }

  const countFor = (source: Map<string, number>, id: string): number =>
    Math.max(0, Math.floor(source.get(id) ?? 0));

  const requested = library.reduce((sum, organism) => sum + countFor(counts, organism.id), 0);
  const clamped = requested > free.length;
  const placeCount = clamped ? free.length : requested;

  // Re-apportion proportionally when the grid cannot hold the request: the
  // existing counts stand in as the weights, which is the same largest-
  // remainder split applied to a smaller total.
  const effectiveCounts = clamped
    ? allocateCounts(
        placeCount,
        library.map((organism) => ({
          organismId: organism.id,
          active: true,
          weight: countFor(counts, organism.id),
        })),
      )
    : counts;

  const random = mulberry32(foldSeed(seed));

  // Partial Fisher-Yates: draw `placeCount` distinct cells from the free list.
  const drawn: Array<[number, number]> = [];
  for (let i = 0; i < placeCount; i += 1) {
    const j = i + Math.floor(random() * (free.length - i));
    const picked = free[j]!;
    free[j] = free[i]!;
    free[i] = picked;
    drawn.push(picked);
  }

  const assignment: SeedOrganism[] = [];
  library.forEach((organism) => {
    const count = countFor(effectiveCounts, organism.id);
    for (let i = 0; i < count; i += 1) {
      assignment.push(organism);
    }
  });

  for (let i = assignment.length - 1; i > 0; i -= 1) {
    const j = Math.floor(random() * (i + 1));
    const swapped = assignment[i]!;
    assignment[i] = assignment[j]!;
    assignment[j] = swapped;
  }

  const programs: SeedProgram[] = drawn.map(([x, y], index) => {
    const organism = assignment[index]!;
    return {
      x,
      y,
      code: [...organism.code],
      free_energy: organism.free_energy,
      free_mass: organism.free_mass,
    };
  });

  return { programs, clampedTo: clamped ? free.length : null };
}
