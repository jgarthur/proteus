import { describe, expect, it } from 'vitest';
import { MetricsBuffer } from './metricsBuffer';
import type { EventTotals, MetricsSnapshot } from '../types';

function makeTotals(overrides: Partial<EventTotals>): EventTotals {
  return {
    births: 0,
    boot_births: 0,
    spawn_births: 0,
    deaths: 0,
    mutations: 0,
    base_mutations: 0,
    background_mutations: 0,
    ...overrides,
  };
}

function makeSnapshot(tick: number, totals: Partial<EventTotals>, epoch = 1): MetricsSnapshot {
  const eventTotals = makeTotals(totals);
  return {
    epoch,
    tick,
    population: 0,
    live_count: 0,
    inert_count: 0,
    total_energy: 0,
    packet_energy: 0,
    total_mass: 0,
    mean_program_size: 0,
    max_program_size: 0,
    unique_genomes: 0,
    births: eventTotals.births,
    boot_births: eventTotals.boot_births,
    spawn_births: eventTotals.spawn_births,
    deaths: eventTotals.deaths,
    mutations: eventTotals.mutations,
    base_mutations: eventTotals.base_mutations,
    background_mutations: eventTotals.background_mutations,
    event_totals: eventTotals,
  };
}

describe('MetricsBuffer mutation-cause series', () => {
  it('derives base_mutations_per_tick and background_mutations_per_tick as event_totals deltas over elapsed ticks', () => {
    const buffer = new MetricsBuffer();

    buffer.push(makeSnapshot(100, { base_mutations: 20, background_mutations: 5, mutations: 25 }));
    buffer.push(makeSnapshot(110, { base_mutations: 70, background_mutations: 25, mutations: 95 }));

    const snapshot = buffer.snapshot();
    expect(snapshot.count).toBe(2);
    // First observation has no prior baseline, so both rates start at 0 (same shape as mutations_per_tick).
    expect(snapshot.base_mutations_per_tick[0]).toBe(0);
    expect(snapshot.background_mutations_per_tick[0]).toBe(0);
    // Second observation: (70-20)/10 and (25-5)/10.
    expect(snapshot.base_mutations_per_tick[1]).toBeCloseTo(5);
    expect(snapshot.background_mutations_per_tick[1]).toBeCloseTo(2);
  });

  it('carries forward the last written per-tick rate when a sample repeats the same epoch/tick (isSameSample), and does not poison the baseline for later samples', () => {
    const buffer = new MetricsBuffer();

    buffer.push(makeSnapshot(100, { base_mutations: 20, background_mutations: 5, mutations: 25 }));
    buffer.push(makeSnapshot(110, { base_mutations: 70, background_mutations: 25, mutations: 95 }));
    // Same tick observed again (e.g. a duplicate WS delivery) with different totals in the payload:
    // elapsedTicks is 0 (tick is not > baseline.tick), so the buffer must carry forward the
    // previously computed rate at the last index rather than deriving a fresh delta from the
    // new (unrelated) totals.
    buffer.push(makeSnapshot(110, { base_mutations: 9999, background_mutations: 9999, mutations: 19998 }));

    const duplicateSnapshot = buffer.snapshot();
    // The repeated sample overwrites the existing row in place; it does not append a new one.
    expect(duplicateSnapshot.count).toBe(2);
    expect(duplicateSnapshot.base_mutations_per_tick[1]).toBeCloseTo(5);
    expect(duplicateSnapshot.background_mutations_per_tick[1]).toBeCloseTo(2);

    // A normal, higher-tick sample follows the poisoned duplicate. If the duplicate's 9999/9999
    // had become the baseline, this sample's totals would compare LOWER than that baseline,
    // trip totalsRegressed, and clear the buffer down to a single zero-rate row. Instead the
    // baseline must still be the ORIGINAL tick-110 totals (70/25/95), so history survives and
    // the new row's rate is the modest (80-70)/10 and (30-25)/10 — not a regression-triggered
    // reset and not a delta against the poisoned totals.
    buffer.push(makeSnapshot(120, { base_mutations: 80, background_mutations: 30, mutations: 110 }));

    const snapshot = buffer.snapshot();
    expect(snapshot.count).toBe(3);
    expect(snapshot.tick[2]).toBe(120);
    expect(snapshot.base_mutations_per_tick[2]).toBeCloseTo(1);
    expect(snapshot.background_mutations_per_tick[2]).toBeCloseTo(0.5);
  });

  it('retains the original cumulative baseline for a pre-existing series (births) across a duplicate, not just the new mutation series', () => {
    const buffer = new MetricsBuffer();

    buffer.push(makeSnapshot(100, { births: 10, boot_births: 6, spawn_births: 4 }));
    buffer.push(makeSnapshot(110, { births: 50, boot_births: 30, spawn_births: 20 }));
    // A poisoned duplicate at the same tick, far exceeding the original births total.
    buffer.push(makeSnapshot(110, { births: 9999, boot_births: 5000, spawn_births: 4999 }));
    // A normal follow-up sample: if births had adopted the duplicate's 9999 as its baseline,
    // 65 < 9999 would trip totalsRegressed and clear the buffer. The fix (shared across all
    // cumulative series, not mutation-specific) keeps the original 50 as the baseline instead.
    buffer.push(makeSnapshot(120, { births: 65, boot_births: 39, spawn_births: 26 }));

    const snapshot = buffer.snapshot();
    expect(snapshot.count).toBe(3);
    expect(snapshot.tick[2]).toBe(120);
    expect(snapshot.births_per_tick[2]).toBeCloseTo(1.5);
  });

  it('behaves identically when a duplicate carries totals identical to the original (well-behaved backend)', () => {
    const buffer = new MetricsBuffer();

    buffer.push(makeSnapshot(100, { base_mutations: 20, background_mutations: 5, mutations: 25 }));
    buffer.push(makeSnapshot(110, { base_mutations: 70, background_mutations: 25, mutations: 95 }));
    // A harmless duplicate: identical totals to the original tick-110 sample.
    buffer.push(makeSnapshot(110, { base_mutations: 70, background_mutations: 25, mutations: 95 }));
    buffer.push(makeSnapshot(120, { base_mutations: 80, background_mutations: 30, mutations: 110 }));

    const snapshot = buffer.snapshot();
    expect(snapshot.count).toBe(3);
    expect(snapshot.tick[2]).toBe(120);
    expect(snapshot.base_mutations_per_tick[2]).toBeCloseTo(1);
    expect(snapshot.background_mutations_per_tick[2]).toBeCloseTo(0.5);
  });

  it('clears the buffered history when event_totals.base_mutations regresses within the same epoch', () => {
    const buffer = new MetricsBuffer();

    buffer.push(makeSnapshot(100, { base_mutations: 20, background_mutations: 5, mutations: 25 }));
    buffer.push(makeSnapshot(110, { base_mutations: 70, background_mutations: 25, mutations: 95 }));
    // base_mutations regresses (e.g. the server restarted the sim under the same epoch) while
    // background_mutations climbs enough that the combined `mutations` total does NOT regress
    // (100 > 95) — isolating the base_mutations-specific regression check from the pre-existing
    // mutations check that would otherwise also catch a simultaneous drop in the total.
    buffer.push(makeSnapshot(120, { base_mutations: 60, background_mutations: 40, mutations: 100 }));

    const snapshot = buffer.snapshot();
    expect(snapshot.count).toBe(1);
    expect(snapshot.tick[0]).toBe(120);
    expect(snapshot.base_mutations_per_tick[0]).toBe(0);
    expect(snapshot.background_mutations_per_tick[0]).toBe(0);
  });
});
