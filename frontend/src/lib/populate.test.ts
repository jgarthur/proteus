import { describe, expect, it } from 'vitest';
import {
  allocateCounts,
  cellKey,
  type ComposerRow,
  createComposerSession,
  foldSeed,
  liveGenerated,
  MAX_COMPOSER_WEIGHT,
  scatter,
} from './populate';
import { SEED_LIBRARY, type SeedOrganism } from './seedLibrary';
import type { SeedProgram } from '../types';

const LIBRARY: readonly SeedOrganism[] = SEED_LIBRARY;
// A fixed three-organism slice: the apportionment assertions below are about
// exact counts, so they must not shift when the library gains an entry.
const TRIO: readonly SeedOrganism[] = LIBRARY.slice(0, 3);
const [FIRST, SECOND, THIRD] = TRIO;

function row(organismId: string, weight: number, active = true): ComposerRow {
  return { organismId, active, weight };
}

function equalRows(weight = 1): ComposerRow[] {
  return TRIO.map((organism) => row(organism.id, weight));
}

function sum(counts: Map<string, number>): number {
  return [...counts.values()].reduce((total, count) => total + count, 0);
}

describe('allocateCounts', () => {
  it('sums to the requested total', () => {
    [0, 1, 7, 100, 999].forEach((total) => {
      expect(sum(allocateCounts(total, [row('a', 1), row('b', 2.5), row('c', 0.5)]))).toBe(total);
    });
  });

  it('splits equal weights as evenly as possible', () => {
    expect([...allocateCounts(9, equalRows()).values()]).toEqual([3, 3, 3]);

    const remainderCounts = [...allocateCounts(10, equalRows()).values()];
    expect(remainderCounts.reduce((total, count) => total + count, 0)).toBe(10);
    expect(Math.max(...remainderCounts) - Math.min(...remainderCounts)).toBe(1);
  });

  it('breaks remainder ties by row order', () => {
    // Three equal weights over 10 leaves one unit; the first row takes it.
    const counts = allocateCounts(10, equalRows());
    expect(counts.get(FIRST!.id)).toBe(4);
    expect(counts.get(SECOND!.id)).toBe(3);
    expect(counts.get(THIRD!.id)).toBe(3);
  });

  it('honours proportions', () => {
    const counts = allocateCounts(100, [row('a', 1), row('b', 3)]);
    expect(counts.get('a')).toBe(25);
    expect(counts.get('b')).toBe(75);
  });

  it('supports decimal weights', () => {
    const counts = allocateCounts(10, [row('a', 0.5), row('b', 1.5)]);
    expect(counts.get('a')).toBe(3);
    expect(counts.get('b')).toBe(7);
  });

  it('drops inactive and zero-weight rows', () => {
    const counts = allocateCounts(10, [row('a', 1, false), row('b', 0), row('c', 2)]);
    expect([...counts.keys()]).toEqual(['c']);
    expect(counts.get('c')).toBe(10);
  });

  it('returns an empty map when nothing is eligible', () => {
    expect(allocateCounts(10, [])).toEqual(new Map());
    expect(allocateCounts(10, [row('a', 0), row('b', 1, false)])).toEqual(new Map());
  });

  it('apportions exactly where float remainders would misassign a unit', () => {
    // Exact quotas are 1 2/3, 1 2/3, 11 2/3 — the two equal weights must tie
    // and take the leftovers by row order. Computed in floats, 15 * 0.1 / 0.9
    // and 15 * 0.7 / 0.9 land on remainders that differ in the last bit and the
    // third row steals a unit, giving [2, 1, 12].
    const counts = allocateCounts(15, [row('a', 0.1), row('b', 0.1), row('c', 0.7)]);
    expect([counts.get('a'), counts.get('b'), counts.get('c')]).toEqual([2, 2, 11]);
    expect(sum(counts)).toBe(15);
  });

  it('keeps equal decimal weights tied at every total', () => {
    for (let total = 0; total <= 60; total += 1) {
      const counts = [...allocateCounts(total, [row('a', 0.3), row('b', 0.3), row('c', 0.3)]).values()];
      expect(counts.reduce((carry, count) => carry + count, 0)).toBe(total);
      expect(Math.max(...counts) - Math.min(...counts)).toBeLessThanOrEqual(1);
    }
  });

  it('keeps a tiny positive weight eligible instead of rounding it to nothing', () => {
    // 1e-7 is below one 2^-20 scale unit; the whole total must still land.
    const solo = allocateCounts(10, [row('a', 1e-7)]);
    expect(solo.get('a')).toBe(10);
    const paired = allocateCounts(10, [row('a', 1e-7), row('b', 1)]);
    expect(sum(paired)).toBe(10);
    expect(paired.get('b')).toBe(10);
  });

  it('clamps absurd weights instead of overflowing to Infinity', () => {
    const counts = allocateCounts(1000, [row('a', 1e308), row('b', 1)]);
    expect(sum(counts)).toBe(1000);
    [...counts.values()].forEach((count) => {
      expect(Number.isFinite(count)).toBe(true);
      expect(Number.isInteger(count)).toBe(true);
    });
    // The clamp is a ceiling on the ratio, not a licence to drop the row.
    expect(counts.get('a')).toBe(1000);
    expect(counts.get('b')).toBe(0);
  });

  it('survives a huge total against a huge weight', () => {
    const counts = allocateCounts(1_048_576, [row('a', MAX_COMPOSER_WEIGHT), row('b', MAX_COMPOSER_WEIGHT)]);
    expect(sum(counts)).toBe(1_048_576);
    expect(counts.get('a')).toBe(524_288);
  });

  it('ignores non-finite weights and totals', () => {
    expect(allocateCounts(10, [row('a', Number.NaN), row('b', Number.POSITIVE_INFINITY)])).toEqual(new Map());
    expect(sum(allocateCounts(Number.NaN, equalRows()))).toBe(0);
    expect(sum(allocateCounts(Number.POSITIVE_INFINITY, equalRows()))).toBe(0);
  });

  it('treats a non-positive total as zero of everything', () => {
    const counts = allocateCounts(-5, equalRows());
    expect(sum(counts)).toBe(0);
    expect([...counts.keys()]).toHaveLength(3);
  });
});

describe('foldSeed', () => {
  it('leaves small seeds alone and stays inside 32 bits', () => {
    expect(foldSeed(0)).toBe(0);
    expect(foldSeed(1)).toBe(1);
    expect(foldSeed(4294967295)).toBe(4294967295);
    expect(foldSeed(Number.MAX_SAFE_INTEGER)).toBeLessThanOrEqual(4294967295);
    expect(foldSeed(Number.MAX_SAFE_INTEGER)).toBeGreaterThanOrEqual(0);
  });

  it('mixes the high bits in', () => {
    expect(foldSeed(2 ** 32)).toBe(1);
    expect(foldSeed(2 ** 33 + 5)).toBe(7);
  });
});

describe('createComposerSession', () => {
  it('starts with every organism active at weight 1 and nothing generated', () => {
    const session = createComposerSession(LIBRARY.map((organism) => organism.id));
    expect(session.total).toBe(1);
    expect(session.generated).toEqual([]);
    expect(session.clampedTo).toBeNull();
    expect(session.rows).toEqual(
      LIBRARY.map((organism) => ({ organismId: organism.id, active: true, weight: 1 })),
    );
  });

  it('hands out independent row arrays', () => {
    const first = createComposerSession(['a']);
    const second = createComposerSession(['a']);
    first.rows[0]!.weight = 9;
    expect(second.rows[0]!.weight).toBe(1);
  });
});

describe('liveGenerated', () => {
  const make = (x: number): SeedProgram => ({ x, y: 0, code: [0x50], free_energy: 0, free_mass: 0 });

  it('keeps only entries still present in the config, by identity', () => {
    const a = make(1);
    const b = make(2);
    const c = make(3);
    expect(liveGenerated([a, b, c], [a, c])).toEqual([a, c]);
  });

  it('drops an entry that was edited into a new object', () => {
    const a = make(1);
    const edited = { ...a, code: [0x51] };
    expect(liveGenerated([a], [edited])).toEqual([]);
  });

  it('drops an entry that was removed', () => {
    const a = make(1);
    const b = make(2);
    expect(liveGenerated([a, b], [b])).toEqual([b]);
    expect(liveGenerated([a], [])).toEqual([]);
  });

  it('does not match a structurally identical but distinct object', () => {
    const a = make(1);
    const twin = { ...a };
    expect(liveGenerated([a], [twin])).toEqual([]);
  });

  it('drops everything when the config is replaced wholesale', () => {
    const a = make(1);
    const b = make(2);
    const loaded = [make(1), make(2)];
    expect(liveGenerated([a, b], loaded)).toEqual([]);
  });
});

const EMPTY = new Set<string>();

function scatterOn(overrides: Partial<Parameters<typeof scatter>[0]> = {}) {
  return scatter({
    width: 16,
    height: 16,
    seed: 1,
    counts: allocateCounts(12, equalRows()),
    occupied: EMPTY,
    library: LIBRARY,
    ...overrides,
  });
}

describe('scatter', () => {
  it('is deterministic for a given seed', () => {
    expect(scatterOn()).toEqual(scatterOn());
  });

  it('re-rolls placement when the seed changes but keeps the counts', () => {
    const a = scatterOn({ seed: 1 });
    const b = scatterOn({ seed: 2 });
    expect(a.programs).not.toEqual(b.programs);

    const tally = (result: { programs: Array<{ code: number[] }> }) =>
      TRIO.map(
        (organism) =>
          result.programs.filter((program) => program.code.join(',') === organism.code.join(',')).length,
      );
    expect(tally(a)).toEqual([4, 4, 4]);
    expect(tally(b)).toEqual([4, 4, 4]);
  });

  it('places every organism on a distinct cell', () => {
    const { programs } = scatterOn({ counts: allocateCounts(200, equalRows()) });
    expect(programs).toHaveLength(200);
    const keys = new Set(programs.map((program) => cellKey(program.x, program.y)));
    expect(keys.size).toBe(programs.length);
  });

  it('keeps every placement inside the grid', () => {
    const { programs } = scatterOn({ width: 5, height: 7, counts: allocateCounts(30, equalRows()) });
    programs.forEach((program) => {
      expect(program.x).toBeGreaterThanOrEqual(0);
      expect(program.x).toBeLessThan(5);
      expect(program.y).toBeGreaterThanOrEqual(0);
      expect(program.y).toBeLessThan(7);
    });
  });

  it('never lands on a hand-placed cell', () => {
    const occupied = new Set([cellKey(0, 0), cellKey(1, 1), cellKey(2, 2), cellKey(3, 3)]);
    // 4x4 grid with 4 cells taken leaves exactly 12 free cells for 12 organisms.
    const { programs, clampedTo } = scatterOn({
      width: 4,
      height: 4,
      occupied,
      counts: allocateCounts(12, equalRows()),
    });
    expect(clampedTo).toBeNull();
    expect(programs).toHaveLength(12);
    programs.forEach((program) => {
      expect(occupied.has(cellKey(program.x, program.y))).toBe(false);
    });
  });

  it('clamps to the free-cell count and reports it', () => {
    const occupied = new Set([cellKey(0, 0)]);
    const { programs, clampedTo } = scatterOn({
      width: 3,
      height: 3,
      occupied,
      counts: allocateCounts(100, equalRows()),
    });
    expect(clampedTo).toBe(8);
    expect(programs).toHaveLength(8);
    expect(new Set(programs.map((program) => cellKey(program.x, program.y))).size).toBe(8);
  });

  it('keeps the clamped mix proportional to the requested one', () => {
    const counts = new Map([
      [FIRST!.id, 30],
      [SECOND!.id, 10],
    ]);
    const { programs, clampedTo } = scatterOn({ width: 2, height: 2, counts });
    expect(clampedTo).toBe(4);
    const firstCount = programs.filter(
      (program) => program.code.join(',') === FIRST!.code.join(','),
    ).length;
    expect(firstCount).toBe(3);
  });

  it('carries each organism free_energy and free_mass', () => {
    const { programs } = scatterOn();
    programs.forEach((program) => {
      const organism = LIBRARY.find((entry) => entry.code.join(',') === program.code.join(','))!;
      expect(program.free_energy).toBe(organism.free_energy);
      expect(program.free_mass).toBe(organism.free_mass);
    });
  });

  it('does not alias the library code arrays', () => {
    const { programs } = scatterOn();
    programs[0]!.code[0] = 0xff;
    expect(LIBRARY.some((organism) => organism.code[0] === 0xff)).toBe(false);
  });

  it('returns nothing when there is nothing to place', () => {
    expect(scatterOn({ counts: new Map() })).toEqual({ programs: [], clampedTo: null });
    expect(scatterOn({ width: 0, height: 0, counts: new Map() })).toEqual({
      programs: [],
      clampedTo: null,
    });
  });

  it('ignores counts for organisms outside the library', () => {
    const { programs } = scatterOn({ counts: new Map([['not-a-real-organism', 5]]) });
    expect(programs).toEqual([]);
  });

  it('does not depend on the insertion order of the occupied set', () => {
    const keys = [cellKey(0, 0), cellKey(1, 1), cellKey(2, 2)];
    const forward = scatterOn({ width: 8, height: 8, occupied: new Set(keys) });
    const reverse = scatterOn({ width: 8, height: 8, occupied: new Set([...keys].reverse()) });
    expect(forward).toEqual(reverse);
  });
});
