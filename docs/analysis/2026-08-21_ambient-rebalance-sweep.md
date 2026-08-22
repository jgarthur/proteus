# Ambient-rate rebalance sweep

Date: 2026-08-21

**Context**: Non-authoritative working notes. Follow-up to the ecology shift flagged in
`2026-08-21_dyadic-sampler-results.md`. Nothing here changes the spec, and **no configuration
default was changed** — the recommendation in §9 is for approval, not applied.

## Summary

1. **The rebalance is not just unnecessary, it is backwards.** Lowering `r_energy` to 0.1953125 to
   restore the old background of 25 makes this ecology significantly *worse*. Paired over 16 seeds,
   moving from background 25 to the current 32 helps 12 worlds, hurts 2, and is neutral on 2; grid
   fill goes from 4/16 to 13/16 (Fisher exact p = 0.004). → §4
2. **Recommendation: change nothing.** Keep `r_energy = 0.25` and `r_mass = 1.0`. → §9
3. **The reported "collapse" is one unlucky seed.** The `tick_bench` fixture seed is one of only two
   worlds out of 16 that the higher background hurts. Reproduced exactly — 6,404 programs at tick
   6,500 — but it is not representative: that same seed fills the grid at backgrounds 25, 28 *and*
   36, and drops into the low mode only at 32. → §2, §4
4. **Response to background energy is monotone over 20–36 and strong**: grid fill rises 1/8 → 4/16 →
   5/8 → 13/16 → 7/8 across backgrounds 20 / 25 / 28 / 32 / 36. There is no case for lowering
   `r_energy`; the case for raising it is real but small and comes with a mutation-load cost. → §6
5. **The ambient background is simultaneously the food and the mutation dose.**
   `bg_radiation_consumed` is both the energy absorbed and the numerator of the mutation
   probability, so `r_energy` moves income and mutation load together by the same factor and cannot
   separate them. → §7
6. **The knob that actually decouples them is `mutation_background_log2`.** Halving the dose at
   fixed income (exponent 8 → 9, background held at 32) takes grid fill to **8/8 with no run below
   57,175 programs** — it removes the failure mode outright rather than shifting its odds. This is a
   real biology change, not a rebalance, and is offered as a finding rather than a recommendation.
   → §7
7. **`r_mass` is inert here.** Background mass 100 vs 128 never moves a world across the basin
   boundary. → §5
8. **The scenario is bistable**, decided between ticks ~500 and ~3,000. Stalled worlds pin at
   deaths/births = 1.00 with background energy *undepleted* — the limit is vital rates, not
   resources. → §8

## 1. Method

Runs use the headless runner (`docs/RUNNER-SPEC.md`) rather than `tick_bench`, so every run carries
a manifest, an input digest, and full census rows.

Each manifest reproduces the `tick_bench` `web-256x256-single` fixture
(`rust/examples/tick_bench.rs`) exactly: 256x256, the `frontend_config` parameter set, a single
12-byte lithotroph at `(32, 32)` with 20 free energy and 12 free mass, no environmental preload.
Only `r_energy`, `r_mass`, `seed`, and — for the §7 diagnostic alone — `mutation_background_log2`
vary. `d_energy = d_mass = maintenance_rate = 2^-7` throughout, as merged.

- 8,000 ticks per run, census sampled every 100 ticks. 80 runs total, plus two 2,000-tick
  perturbation runs for the pairing check below.
- Seed set A (8): the fixture seed `6846702536457205`, `987654321`, `11/22/33/44/55/66`.
  Seed set B (8, used to extend the two head-to-head arms to n = 16):
  `101/202/303/404/505/606/707/808`.
- "Grid fill" (takeoff) below means final population >= 20,000, about 30% occupancy.

The threshold is not a judgement call. Across the 32 runs of the §3 grid the single largest gap in
the entire final-population distribution — 18,336 programs, between 16,669 and 35,005 — falls
exactly there. The distribution is two clusters with nothing in between.

Sweep manifests and analysis scripts were kept out of the repository tree as one-off experiment
material; §1 plus the configuration columns below fully specify every run.

### The comparison is paired

Parameter changes do **not** re-roll the random stream. The engine derives a fresh
`cell_rng(seed ^ SALT, tick, cell_index)` per pool, per tick, per cell, so the underlying noise
field depends only on the world seed. Verified directly: `r_energy = 0.25` and
`r_energy = 0.2500001` on the fixture seed produce identical populations at every sampled tick
through tick 2,000.

Per-seed differences between configurations are therefore genuine causal responses of the same
world, not different draws. This is what makes the 16-seed sign test in §4 legitimate, and it is
why the `r_mass` contrast in §5 is a clean controlled comparison.

## 2. Reproduction check

The runner reproduction matches the recorded `tick_bench` observations exactly. At
`r_energy = 0.25, r_mass = 1.0` on the fixture seed this sweep gets **6,404 programs at tick 6,500**,
the same number reported in `2026-08-21_dyadic-sampler-results.md`.

The old-default trajectory is not rerunnable (`d = 0.01` is rejected by dyadic validation), so the
recorded checkpoints are the comparison target:

| tick | old defaults (`d = 0.01`, background 25) | source |
|---|---|---|
| 1,000 | 1,635 programs, 1,561 live, 1,673 births, 120 deaths, 525 mutations, 2.5% occupancy | `2026-08-12_rayon-optimization-results.md` |
| 6,500 | 52,560 programs, 52,289 live, 80.2% occupancy | `2026-08-12_rayon-optimization-results.md` |

## 3. The requested 2x2 grid

Final population at tick 8,000, seed set A. `e195` = `r_energy` 0.1953125, `e250` = 0.25;
`m781` = `r_mass` 0.78125, `m1000` = 1.0. Backgrounds are `r_*/d_*`.

| seed | e195/m781 (25/100) | e195/m1000 (25/128) | e250/m781 (32/100) | e250/m1000 (32/128) |
|---|---:|---:|---:|---:|
| 6846702536457205 (fixture) | 52,546 | 52,247 | 7,062 | 8,110 |
| 987654321 | 802 | 1,596 | 40,588 | 35,005 |
| 11 | 64,539 | 64,648 | 16,289 | 16,669 |
| 22 | 2,741 | 2,669 | 53,690 | 49,382 |
| 33 | 60,619 | 60,014 | 64,661 | 64,571 |
| 44 | 2,710 | 2,774 | 57,931 | 59,124 |
| 55 | 4,016 | 3,220 | 65,414 | 65,464 |
| 66 | 64,022 | 63,601 | 61,420 | 62,246 |
| **grid fill** | **4/8** | **4/8** | **6/8** | **6/8** |
| **median** | **28,281** | **27,734** | **55,810** | **54,253** |

The `{0.25, 1.0}` corner is the current post-dyadic default and already fills the grid on 6 of 8
seeds. The proposed rebalance corner does so on 4 of 8.

## 4. Head-to-head at n = 16

Seed set A + B, `r_mass = 1.0`, i.e. exactly the change the rebalance would make.

| | background 25 (`r_energy = 0.1953125`) | background 32 (`r_energy = 0.25`, current) |
|---|---:|---:|
| grid fill | 4/16 | **13/16** |
| median final population | 4,400 (6.7% occupancy) | **56,312 (85.9%)** |
| min / max | 1,140 / 64,648 | 8,110 / 65,464 |

Paired by seed, the effect of *raising* the background from 25 to 32:

| direction | seeds | count |
|---|---|---:|
| helps | 987654321, 22, 44, 55, 101, 202, 303, 404, 505, 606, 707, 808 | **12** |
| hurts | 6846702536457205 (fixture), 11 | 2 |
| neutral | 33, 66 | 2 |

Sign test on 12 vs 2: p = 0.013. Fisher exact on the grid-fill counts 4/16 vs 13/16: **p = 0.0039**.

Note that the eight-seed estimate in §3 was optimistic for the rebalance corner: extending
`e195/m1000` from 8 to 16 seeds moved its median from 27,734 down to 4,400. The extension was worth
running.

The fixture seed is one of only two worlds in which the rebalance helps, and it is the seed the
original collapse report rested on.

That seed is worth following across the whole response curve of §6, because it shows how little the
single-seed result means:

| background | 20 | 25 | 28 | **32 (current)** | 36 |
|---|---:|---:|---:|---:|---:|
| fixture seed, final population | 3,704 | 52,247 | 55,170 | **8,110** | 59,882 |

The response is not monotone for this world — it fills the grid at 25, 28 *and* 36, and falls into
the low mode only at 32. Near a bifurcation an individual trajectory is chaotic in the parameter;
only the ensemble carries a trend. So the observed collapse is not evidence that background 32 is
bad, and one notch higher would have shown this same seed filling the grid.

`docs/BACKLOG.md` independently records a `web-256x256` run
reaching 65,062 programs / 99.3% occupancy at tick 8,000 on this same post-dyadic commit during
EMERGENCE-CENSUS. No seed was recorded for it, but it is exactly what §4 predicts for a typical
seed under the current defaults.

## 5. `r_mass` is inert

Because the comparison is paired (§1), the `r_mass` columns of §3 are a clean controlled contrast.
Final populations at background mass 100 vs 128 differ by 0.4%–1.2% on six of eight seeds at
`e195` (52,546/52,247; 64,539/64,648; 60,619/60,014; 64,022/63,601; 2,741/2,669; 2,710/2,774) and
grid fill is identical in both columns at both `r_energy` levels. The two larger gaps (802 vs 1,596
and 4,016 vs 3,220) are both inside the stalled mode, where populations are small and noisy.

Mass is not radiation: it supplies `appendAdj` build material and nothing else on the hot path, and
at both settings there is far more of it than the population consumes — stalled worlds finish with
mass still near its initial level. There is no measured benefit to moving it.

## 6. Background-energy response curve

`r_mass = 1.0` throughout, seed set A (plus B where n = 16). Vital rates are per live program per
1,000 ticks over ticks 500–1,000, averaged across seeds; "growth" is mean population at tick 1,000
over mean population at tick 500.

| `r_energy` | background | n | grid fill | median final | births | deaths | growth | mutations @1k |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| 0.15625 | 20 | 8 | 1/8 | 4,249 (6.5%) | 1.8 | 0.3 | 2.23x | 206 |
| 0.1953125 | 25 | 16 | 4/16 | 4,400 (6.7%) | 1.9 | 0.3 | 2.53x | 344 |
| 0.21875 | 28 | 8 | 5/8 | 38,601 (58.9%) | 2.1 | 0.3 | 2.66x | 474 |
| **0.25 (current)** | **32** | **16** | **13/16** | **56,312 (85.9%)** | **2.2** | **0.4** | **2.72x** | **616** |
| 0.28125 | 36 | 8 | 7/8 | 62,637 (95.6%) | 2.3 | 0.4 | 2.87x | 765 |

Grid fill is monotone increasing across the whole range; endpoints 1/8 vs 7/8 give Fisher exact
p = 0.010. Both the constant-background target (25) and anything below it sit deep in the failing
half of the curve.

The mechanism is visible in the vital-rate columns. More background raises the per-program birth
rate substantially (1.8 → 2.3) and the death rate only slightly (0.3 → 0.4). Because growth is
exponential, a per-window multiplier moving from 2.23x to 2.87x is the difference between a world
that outruns its losses and one that does not.

Extrapolating past 36 was not tested and is not recommended blind: the mutation column is rising
faster than the birth column, so the curve must turn over somewhere.

## 7. Mechanism: background level *is* the mutation dose

`mutate_end_of_tick_cell` in `rust/src/pass3.rs` sets the per-tick mutation probability of any
program that absorbed background radiation to
`min(bg_radiation_consumed / 2^mutation_background_log2, 1)` — `consumed / 256` at the default
exponent. Programs that absorbed nothing fall back to the `2^-16` baseline. On undisturbed ground
`bg_radiation_consumed` is the stationary background `r_energy / d_energy`
(`initialize_background_steady_state` in `rust/src/simulation.rs` seeds every cell from
Poisson(`r/d`), and the per-tick binomial-decay/Poisson-arrival update holds it there).

One quantity is therefore both the meal and the dose:

| | background | energy per absorb | p(mutate) per absorb |
|---|---:|---:|---:|
| old defaults | 25 | 25 | 9.8% |
| current defaults | 32 | 32 | 12.5% |
| `r_energy = 0.1953125` | 25 | 25 | 9.8% |

`r_energy` moves income and mutation load together by the same 28%, and cannot separate them.

### The diagnostic: separating the two channels

To test whether the mutation arm is real and binding, one arm held `r_energy = 0.25` (background 32,
income unchanged) and raised `mutation_background_log2` from 8 to 9, halving the dose per absorb.

| config | background | dose per absorb | n | grid fill | median | min | mutations @1k |
|---|---:|---:|---:|---:|---:|---:|---:|
| `r_energy = 0.25`, exponent 8 (current) | 32 | 12.5% | 16 | 13/16 | 56,312 | 8,110 | 616 |
| `r_energy = 0.25`, exponent 9 | 32 | 6.25% | 8 | **8/8** | **63,196 (96.4%)** | **57,175** | 326 |

Fisher exact against the background-25 arm: p = 0.0014. The intervention did what it was designed to
do (mutations at tick 1,000 roughly halve, 616 → 326) while leaving energy income untouched, and the
result is qualitatively different from anything `r_energy` achieves: it does not shift the odds of
the bimodal outcome, it **eliminates the low mode** — the worst of eight runs finished at 57,175
programs, above the median of every other configuration tested.

So both channels are real and they oppose each other:

- **income channel** — more background raises the birth rate; net positive.
- **mutation channel** — more background raises mutational erosion; net negative.

Over 20–36 the income channel dominates, which is why the observed `r_energy` response is monotone
increasing despite the rising mutation load. `mutation_background_log2` is the only knob tested that
addresses the mutation channel alone.

This is a significant caveat on the exponent finding: halving the background-stressed mutation rate
halves the substrate's operative evolutionary clock, which
`2026-08-13_mutation-load-and-emergence-milestones.md` §3 identifies as the rate that actually
governs evolution in a working ecology. A world that always fills the grid may be a *less*
interesting world. That trade is a design decision, not a rebalance, and is out of scope here.

### Matched steady state is not matched dynamics

At `r_energy = 0.1953125` the equilibrium background returns to 25, but the relaxation time does
not. A harvested cell refills toward `r/d` with time constant `1/d_energy` — 128 ticks now against
100 ticks under `d = 0.01` — so harvested ground recovers 28% slower.

This shows up twice. On the fixture seed that configuration reaches 52,546 programs at tick 8,000
where the old defaults reached 52,560 at tick 6,500: a 1.23x delay against the 1.28x the time
constants predict. And at tick 1,000 it records only 356 mutations against the old baseline's 525 at
the *same* background, because programs re-absorbing recently harvested cells collect smaller doses
under the slower refill. Matching `r/d` restores the equilibrium, not the dynamics, and `d_energy` —
the knob that would restore them — is dyadic-constrained.

## 8. What the stalled worlds are doing

All eight seeds at one configuration track each other closely to about tick 500, then bifurcate.
`e195/m781` populations at ticks 500 / 1,000 / 2,000 / 3,000 / 8,000:

| seed | 500 | 1,000 | 2,000 | 3,000 | 8,000 |
|---|---:|---:|---:|---:|---:|
| 11 | 438 | 1,523 | 4,191 | 6,517 | 64,539 |
| 33 | 564 | 1,623 | 4,976 | 9,852 | 60,619 |
| 22 | 570 | 1,457 | 2,376 | 2,550 | 2,741 |
| 44 | 464 | 1,168 | 1,889 | 2,327 | 2,710 |

Stalled worlds are not resource-limited. At tick 8,000 seed 22 holds 2,741 programs with total
energy at 24.5 per cell — essentially the undisturbed equilibrium of 25. What pins them is the vital
rate: deaths/births per window climbs to 1.00 and stays there, and mean offspring per live program
falls from 1.00 to 0.91.

### A caution about the `absorb` census

Per-program `absorb` count looks like an obvious fitness proxy and is a reliable predictor *within* a
configuration — takeoff runs carry more `absorb` than stalled runs in all seven configurations
tested. But it **inverts across configurations**: the worst configuration tested (background 20,
1/8 fill) has the *highest* absorb count at tick 1,000 (7.9 per program) and the best (background
36, 7/8 fill) has nearly the lowest (3.6).

The resolution is that absorb count tracks scarcity, not fitness. When each absorb yields less,
selection favours more absorb instructions per genome; the genomes are working harder for the same
meal. Read within a configuration it measures how well a lineage is holding its machinery together;
read across configurations it measures how poor the environment is. The two readings point opposite
ways, so the census column should not be quoted as a bare fitness signal.

`move` is not involved at all: it stays below 2 per 1,000 instructions in every run, because the
seed lithotroph contains none. The `move`-based viability argument in
`2026-08-13_mutation-load-and-emergence-milestones.md` §5 concerns evolved glider genomes and does
not bear on this fixture's outcome.

## 9. Recommendation

**Change nothing. Keep `r_energy = 0.25` and `r_mass = 1.0`** against the merged `d_* = 2^-7`.

- The constant-background rebalance to `r_energy = 0.1953125` is refuted, not merely unsupported: it
  loses on 12 of 16 paired seeds and drops grid fill from 13/16 to 4/16 (p = 0.004).
- `r_mass = 0.78125` is separately unsupported: the knob is inert in this scenario (§5).
- The collapse that motivated the sweep is a property of the fixture seed, not of the defaults.

Two things worth deciding separately, neither of them this task:

- **Benchmark practice, not configuration, is what should change.** `web-256x256-single` at the
  fixture seed is bimodal and that seed sits in the minority mode, so a single-seed grown-web
  checkpoint is not a stable basis for either performance or ecology comparison. Dense-regime
  benchmarking should use a synthetic dense fixture — as `2026-08-21_dyadic-sampler-results.md`
  already concluded — or report a seed ensemble.
- **`mutation_background_log2` is the real lever on this ecology** (§7) and it is currently untuned.
  Raising it to 9 gives 8/8 grid fill with no run below 57,175 programs. It is also a substantive
  change to the evolutionary clock, so it deserves its own investigation rather than being folded
  into a rate rebalance.

## 10. Machine conditions

Runs were executed 4-way concurrent (`proteus-batch` `jobs: 4`) on an 8-core machine (4 performance
+ 4 efficiency) with other users logged in. Load average was 1.89 at the start and 4.5–5.6 during
the batches. These are ecology comparisons, not timings; simulation results are deterministic and
unaffected by load. No wall-clock number from this sweep should be used as a benchmark.
