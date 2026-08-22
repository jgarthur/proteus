# Tier 2a upper-bound diagnostic — results

**Date:** 2026-08-22
**Branch:** `diag/boxed-program` @ `b204e35`, based on `main` @ `0e3092f`
**Worktree:** session scratchpad (measurement branch; kept unmerged for reference)
**Question:** how much do the code-free full-grid sweeps (ambient / tail / prepare) gain if
the cell stride drops from 128 B to ~24 B? This bounds Tier 2a
(`docs/analysis/2026-08-13_performance-roadmap.md` §2a) before anyone builds it.

## Verdict

**NO-GO for Tier 2a.** The threshold was "GO if empty-256x256 improves ≥10% serial or ≥15%
Rayon-4". Measured: **−2.3% serial, −3.5% Rayon-4** — roughly a quarter of the bar on both
paths, and this branch is a *generous* upper bound (see "Why this bounds 2a from above").

The premise is wrong in a specific, measurable way: the dominant phase on a code-free grid is
`ambient`, and `ambient` is **RNG-compute-bound, not stride-bound**. It costs ~70 ns/cell
regardless of whether the cell is 128 B or 24 B.

## What was built

`Cell.program: Option<Program>` → `Option<Box<Program>>` (`rust/src/model.rs:403`).

| | `size_of::<Cell>()` | `size_of::<Program>()` |
|---|---|---|
| main | **128 B** | 112 B |
| branch | **24 B** | 112 B |

5.33× stride reduction. The production diff is 8 lines:

- 4 construction sites wrap in `Box::new` (`model.rs` `with_program`, `bootstrap.rs:138`,
  `pass2.rs:517` append-create, `pass3.rs:759` spontaneous creation) — one alloc per birth,
  alongside the `Vec<u8>` alloc a birth already pays.
- 2 helpers in `pass3.rs` (`program`, `program_mut`) switch `as_ref`/`as_mut` →
  `as_deref`/`as_deref_mut`.
- 1 site (`pass2.rs:306`) switches to `as_deref` so `is_some_and(Program::is_inert)` still
  type-checks against the fn pointer.

Everything else compiled untouched via deref coercion. The remaining 19 changed lines are
test/example fixtures.

### Behaviour neutrality — proven, not asserted

- `cargo test` (default/serial) and `cargo test --features rayon`: pass unmodified. No
  structural-invariant test (conservation, determinism, property, shuffled-order) was touched.
- `./scripts/check-rayon-parity.sh 1000 1 2 4 8`: **byte-identical** to the main baseline at
  `<scratchpad>/parity-main.txt` (`diff` empty). All six fixtures' `grid` / `reports` /
  `packets` digests and every event counter unchanged, serial and Rayon at 1/2/4/8 threads.
  Saved at `<scratchpad>/parity-branch.txt`.
- No RNG draw is added, removed, or reordered: no sampler, no branch condition, and no
  `p`/`k` argument changes. Draw-stream positions are identical by construction, and the
  digest equality above is the proof.
- `cargo clippy --all-targets` clean on both feature sets.
- Every `tick_bench` **activity line is identical** main vs branch on all 8 fixture/mode
  cells (births, deaths, mutations, final_packets, start/final programs, occupancy) — the
  compared worlds are the same worlds.

## Benchmark

Interleaved quiet-machine pattern. Four `--release` binaries (main serial, main rayon,
branch serial, branch rayon) built ahead of time into separate `--target-dir`s under the
scratchpad; main built from the read-only checkout at `/Users/joey/dev/proteus/rust` (left
clean). Waited for `load1 < 3.5` on three consecutive 20 s samples — reached at 16:38:20
with load1 = 2.71 — then 2 interleaved rounds of `1000 5`, main and branch back-to-back per
fixture. Cell values are **min-of-reps, min-of-rounds** (min of 10 per-rep means).

| mode | fixture | main µs/tick | branch µs/tick | delta | activity |
|---|---|---:|---:|---:|---|
| serial | empty-64x64 | 315.4 | 310.5 | **−1.6%** | identical |
| serial | **empty-256x256** | 5126.9 | 5006.9 | **−2.3%** | identical |
| serial | dense-additive-128x128 | 1722.3 | 1784.0 | **+3.6%** | identical |
| serial | dense-emit-64x64 | 562.7 | 598.9 | **+6.4%** | identical |
| serial | frontend-64x64 | 486.7 | 496.7 | **+2.1%** | identical |
| rayon-4 | **empty-256x256** | 1710.3 | 1650.1 | **−3.5%** | identical |
| rayon-4 | dense-additive-128x128 | 817.5 | 811.0 | −0.8% | identical |
| rayon-4 | dense-emit-64x64 | 432.6 | 439.5 | +1.6% | identical |

Negative = branch faster. Per-round reproducibility (round 1 | round 2) was tight on the
serial fixtures — e.g. dense-emit main `562.8 | 562.7`, branch `599.9 | 598.9` — so the
serial deltas are well outside round noise. The one soft cell is rayon-4
dense-additive (branch `849.4 | 811.0`, 4.7% spread); treat its −0.8% as "no change". Load
had climbed back to 6.26 by the end of the run, which is the likely source of that spread;
the headline empty-256x256 cells were stable in both rounds and both modes.

## Interpretation (a) — the sweep upper bound

**Empty-grid delta = −2.3% serial / −3.5% Rayon-4. That is the ceiling for Tier 2a.**

On an empty grid there are no programs at all, so the branch pays *zero* deref cost and
*zero* allocation cost. The measured delta is pure stride effect, uncontaminated.

Phase breakdown, serial empty-256x256 (µs/tick). Phase figures are per-run means over the 5
reps, min across the 2 rounds — a different aggregation from the min-of-reps table above,
which is why this table's total reads −3.1% against the headline −2.3%. The conservative
−2.3% is the one quoted against the threshold.

| phase | main | branch | delta | main ns/cell |
|---|---:|---:|---:|---:|
| prepare | 159.4 | 152.5 | −4.3% | 2.43 |
| pass1 | 70.7 | 54.1 | −23.5% | 1.08 |
| pass2 | 1.3 | 0.6 | −53.8% | — |
| **ambient** | **4574.6** | **4531.7** | **−0.9%** | **69.8** |
| tail | 300.6 | 233.3 | −22.4% | 4.59 |
| mutation | 42.2 | 29.8 | −29.4% | 0.64 |
| newborn-clear | 40.6 | 24.5 | −39.7% | 0.62 |
| **total** | **5189.5** | **5026.7** | **−3.1%** | |

The sweeps *are* stride-bound and the change works exactly as designed: prepare + pass1 +
tail + mutation + newborn-clear go 613.5 → 494.2 µs, a **−19.4%** collective win. But those
five phases are only **11.8%** of the empty-grid tick. Ambient is **88.1%**, and ambient
barely moved.

**Why ambient does not care about stride (first principles).** `pass3_ambient` per cell
(`rust/src/pass3.rs:585` and `:613`) seeds a fresh `cell_rng` twice (radiation salt, mass
salt) and runs `binomial_pow2` + `sample_poisson` on each — several RNG draws and a
rejection loop per cell, per traversal. That work is ~70 ns/cell, and it is a function of
the *simulation rules*, not of the cell layout.

Now price the memory side. Streaming a 128 B cell at ~60 GB/s is ~2.1 ns/cell; a 24 B cell
is ~0.4 ns. So the entire memory traffic of the ambient sweep is ~3% of its ~70 ns/cell
budget, and out-of-order execution hides most of it behind the RNG arithmetic. Shrinking the
cell can recover **at most ~2.4%** of ambient even with an infinitely fast memory system. We
measured −0.9%. The model and the measurement agree.

The same model predicts the phases that *did* win, which is the check that it is not a
just-so story: `tail` costs 4.59 ns/cell on main. Subtract the ~1.7 ns/cell of saved
streaming and you predict 2.9–3.6 ns/cell. Measured: 3.56 ns/cell. The stride model is
correct — it simply has almost nothing to work with in the phase that dominates the tick.

**Implication:** Tier 2a cannot beat ~−3% on a code-free grid no matter how well it is
implemented, because 88% of that grid's tick is arithmetic, not memory traffic. The
roadmap's premise — that the code-free sweeps are stride-bound — is true of the *minority*
of sweep time and false of the majority.

## Interpretation (b) — the dense delta, and the Pass 1 deref cost

On occupied grids the branch is a **net regression**: +3.6% (dense-additive), +6.4%
(dense-emit), +2.1% (frontend) serial. As the spec anticipated, this number is
sweep-gain *minus* pointer-chase cost, and Tier 2a proper (programs in a contiguous cold
array indexed by cell) would not pay that cost. So the two halves are reported separately.

Measured deref cost, serial:

| fixture | pass1 main | pass1 branch | deref cost |
|---|---:|---:|---:|
| dense-additive-128x128 | 351.9 | 403.0 | +51.1 µs (+14.5%) |
| dense-emit-64x64 | 144.3 | 174.3 | +30.0 µs (+20.8%) |
| frontend-64x64 | 63.7 | 74.2 | +10.5 µs (+16.5%) |

**Pass 1 pays 15–21% for one extra dereference per program.** That is the honest cost side
of this experiment, and it is large — the boxed programs are scattered across the heap, so
every occupied cell is a dependent load that the prefetcher cannot follow.

Ambient also regressed on dense grids (+2.2 to +2.8%) where it was flat on empty ones. Same
mechanism: `resolve_absorb` (`pass3.rs:262`) scans every cell reading
`program.tick.absorb_count`, and `resolve_collect_cell` (`pass3.rs:602`) reads
`program.tick.did_collect` — both are full derefs on the branch, one per occupied cell.

**Reconstructing Tier 2a proper.** Subtracting the pass1 and ambient regressions from the
branch totals — arithmetic reconstruction, *not* a measurement — gives what 2a might see if
it kept the sweep gain and paid no pointer chase:

| fixture | main | branch | branch − pass1 − ambient regressions | reconstructed delta |
|---|---:|---:|---:|---:|
| serial dense-additive-128x128 | 1727.0 | 1797.5 | 1720.7 | **−0.4%** |
| serial dense-emit-64x64 | 564.1 | 601.3 | 566.1 | **+0.4%** |
| serial frontend-64x64 | 489.4 | 498.7 | 487.8 | **−0.3%** |

Even with the deref cost hypothetically erased, dense grids land within ±0.5% of main. The
sweep gain that Tier 2a is built to capture is real but tiny, and on occupied grids it is
fully consumed by the fact that occupied-cell work dominates.

## Why this bounds 2a from above

Three reasons the real Tier 2a would land at or below these numbers:

1. **Empty grids give the branch a free pass.** Zero programs means zero deref, zero
   allocation. Real 2a has the same property here, so −2.3% / −3.5% transfers directly as a
   ceiling.
2. **The branch gets a bonus 2a would not have.** Moves in Pass 2 (`pass2.rs:488`
   `source_cell.program.take()` → `target_cell.program = program`) become an 8-byte pointer
   swap instead of a 112-byte struct move. An SoA split with programs in a cold array still
   moves the whole `Program`. Some of the branch's dense-grid `pass2` win (−6.4% on
   dense-additive) is this bonus, not stride.
3. **2a costs more to build.** This diagnostic is an 8-line production diff that preserves
   every digest. The real SoA split touches every `Cell` access site in the engine, with the
   draw-stream risk that entails — for a benefit bounded above by 3%.

## Recommendation

**NO-GO on Tier 2a.** Do not build the SoA hot-field split. It misses its own threshold by
~4×, and the reason is structural rather than a matter of implementation quality: the
code-free grid's cost is ~88% RNG arithmetic in `pass3_ambient`, which no layout change can
touch.

If empty/sparse-grid tick cost is worth attacking, the target is the ~70 ns/cell of
`cell_rng` + `binomial_pow2` + `sample_poisson` that every cell pays every tick — e.g.
skipping the sampler entirely for cells where `bg_radiation == 0` and the arrival draw is the
only possible effect, or a cheaper arrival sampler. Both are draw-stream-affecting changes
and would need the full sampled-literal reconciliation treatment, but they aim at the 88%
rather than the 12%.

This is the second roadmap tier whose premise did not survive measurement (see
`docs/analysis/2026-08-22_inline-program-storage-results.md` for Tier 2b). The cheap-diagnostic-first
order worked here: 8 lines and one bench window bought the answer instead of a full
implementation.

## Artifacts

- Branch: `diag/boxed-program` @ `b204e35` (kept for reference, not merged)
- Bench log: `<scratchpad>/diag2a-bench.log`
- Parity output: `<scratchpad>/parity-branch.txt` (diff-identical to `parity-main.txt`)
- Bench scripts: `<scratchpad>/diag2a-bench.sh`, `diag2a-run.sh`, `diag2a-parse.py`
- Quiet-window log: `<scratchpad>/diag2a-wait.log`
- Binaries: `<scratchpad>/diag2a-bin/`
