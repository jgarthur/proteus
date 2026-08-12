# RAYON-PERFORMANCE implementation plan

Date: 2026-08-12

This is the implementation plan for the `RAYON-PERFORMANCE` status item: remove Pass 2 grid-clone overhead and fuse compatible Rayon Pass 3 traversals. It builds on the measurements and recommended design in `docs/analysis/2026-08-11_rayon-performance-profile.md` and is written to be executed by someone without prior context.

Implementation completed on 2026-08-12. See `2026-08-12_rayon-optimization-results.md` for the final code shape, exact growing-web-scenario benchmarks, fixture and opcode audit, deep parity check, and the profile-guided deviations from the proposed allocation trims.

As with everything in `analysis/`, this document is supporting material, not authority. `SPEC.md` and `SPEC-COMPANION.md` remain the source of truth for behavior; every equivalence claim below was checked against the source on 2026-08-12 and should be re-verified against the tree you are editing. Read `docs/BACKEND-GUIDELINES.md` and `rust/AGENTS.md` before starting.

## Non-negotiable constraints

- Pass 2 and the shared Pass 3 helpers are compiled into both the serial and Rayon builds. Every change to shared code must be **bit-identical** in behavior: same grid state, same RNG draw sequence, same `TickReport`s, for all inputs the tests and Pass 1 can produce.
- Rayon-only changes go inside `#[cfg(feature = "rayon")]` blocks; keep the serial loop shapes unchanged (the 2026-08-11 profile found the fused shape slightly slowed the serial build).
- Public API: keep `pass2_nonlocal(&mut Grid, &[QueuedAction], u64, u64) -> Pass2Output` and the `Pass2Output` fields unchanged. Integration tests in `rust/tests/pass2_nonlocal.rs` and `rust/tests/property_invariants.rs` call it directly.
- After each step: `cargo fmt`, `cargo clippy --all-targets --all-features`, `cargo test`, `cargo test --features rayon`, and `rust/scripts/check-rayon-parity.sh` (CI runs it at 300 ticks, threads 1 2 4 8).

## Step 1 (done): benchmark harness and baseline

What landed:

- `rust/src/simulation.rs`: a `TickPhase` enum, a `TickObserver` trait, and `Simulation::run_tick_report_observed`. `run_tick_report` now delegates to the observed variant with a no-op `()` observer whose `ENABLED: bool = false` const keeps the unobserved path free of clock reads. The observer receives per-phase wall-clock durations (prepare, pass1, pass2, packets, ambient, tail, mutation, newborn-clear, total) and cannot influence simulation state.
- `rust/examples/tick_bench.rs`: fixture-based whole-tick and per-phase benchmark. Fixtures: `frontend-64x64` (the parity lithotroph replay), `empty-64x64`, `empty-256x256`, `dense-additive-128x128` (every cell queues one `GIVE_E` per tick, no exclusives, maintenance off so the workload is stationary), `dense-emit-64x64` (every cell emits one packet per tick). Usage:

  ```bash
  cargo run --release --example tick_bench -- 1000 5            # serial
  RAYON_NUM_THREADS=4 cargo run --release --features rayon --example tick_bench -- 1000 5
  ```

  Always launch through `cargo run` with explicit features: `target/release/examples/tick_bench` is whichever feature set was built last, and running the stale binary silently benchmarks the wrong configuration.

Baseline, serial build, Apple M2 (4P+4E), 1000 ticks x 5 reps, mean µs/tick:

| Phase | frontend-64x64 | empty-64x64 | empty-256x256 | dense-additive-128x128 | dense-emit-64x64 |
| --- | ---: | ---: | ---: | ---: | ---: |
| prepare | 9.6 | 10.4 | 284.1 | 97.6 | 17.8 |
| pass1 | 68.0 | 5.2 | 188.0 | 1622.7 | 576.9 |
| pass2 | 252.7 | 71.2 | 2417.1 | 4017.3 | 1347.5 |
| packets | 5.3 | 0.0 | 0.3 | 0.4 | 194.5 |
| ambient | 496.2 | 493.3 | 12865.5 | 5833.6 | 1095.3 |
| tail | 99.5 | 54.4 | 1589.5 | 688.6 | 200.6 |
| mutation | 6.2 | 3.1 | 118.1 | 81.3 | 22.3 |
| newborn-clear | 4.1 | 3.1 | 110.7 | 56.8 | 10.0 |
| total | 941.6 | 640.8 | 17573.5 | 12398.4 | 3464.9 |

**Caveat: the machine was heavily loaded during this baseline** (a game plus WindowServer consumed several cores; load average ~17). The frontend-64x64 serial reps were stable (934–953 µs), but empty-256x256 drifted 12–29 ms across reps, and the Rayon-build runs recorded alongside were meaningless (frontend ~3.6 ms/tick at 4 threads — worse than serial, with 2.4x rep spread). The Rayon baseline must be re-recorded on a quiet machine before implementation starts, and every before/after comparison in steps 2–7 must be same-session, quiet-machine. Raw phase numbers on this machine that ARE trustworthy directionally: pass2 is the largest or second-largest phase in every fixture with programs, which is exactly what step 2 attacks.

## Verified code facts the design relies on

Re-verify each against the current tree; line numbers are from 2026-08-12.

1. **One nonlocal action per cell per tick from Pass 1.** `Pass1CellOutput.action` is `Option<QueuedAction>`, and `canonicalize_pass1_output` sorts by `source_index` (`rust/src/pass1.rs:190`). However, integration tests call `pass2_nonlocal` with hand-built action lists in which one source issues **both** a `GiveE` and a `GiveM` (`rust/tests/property_invariants.rs:197-226`). Do not assume one-action-per-source in Pass 2; assume only what the tests exercise.
2. **Pass 2 never mutates `bg_radiation` or `bg_mass`.** Reads touch source stack/registers/flag; additive transfers touch `free_energy`/`free_mass` and source flags; exclusive winners touch program code/registers/flags and free resources. The only bg write in Pass 2 is `apply_move_commit` restoring `bg_*` from `exclusive_base` (`rust/src/pass2.rs:477-478`) — a self-assignment no-op, since nothing changed them. This is why the `exclusive_base` clone is removable.
3. **A cell can be both a source and a target in the same tick.** `Opcode::Listen` sets `is_open = true` on a *live* program (`rust/src/pass1.rs:549`), so a live listener can be a valid `WriteAdj`/`AppendAdj`/`DelAdj` target while also having queued its own action (including `Move`). Equivalence therefore comes from **replicating the current mutation order exactly**, not from any source/target disjointness argument.
4. **Validation state vs. apply state are different and both load-bearing.** `validate_exclusive` reads pre-Pass-2 state (program presence, openness, sizes, and `strength = min(source size, source free_energy)`), all *before* additive transfers change free resources. But `apply_winner`'s `AppendAdj` `free_mass == 0` check and `DelAdj` `free_energy < strength` check read the **post-read/additive working state** at apply time (`rust/src/pass2.rs:362,400`). Preserve this split.
5. **`resolve_exclusive` currently mutates one working grid sequentially**: invalid-candidate flags first, then per-target groups in ascending target order (loser flags, winner effects), then deferred move commits, then deferred append-create commits (`rust/src/pass2.rs:164-252`). Deferral matters: a moving program can receive a `WriteAdj` from a winner in another group before its move commits, and the move must carry the post-write program.
6. **Grouping and tie-breaking determinism**: groups iterate targets in ascending index order; `choose_winner` picks max strength, collects ties, sorts ties by source, then does a size-weighted roll from `cell_rng(seed ^ EXCLUSIVE_TIE_SALT, tick, target)`. Winner identity is independent of within-group candidate order.
7. **Boot-only groups**: when every candidate for a target is `Boot`, all sources get `flag = false`, the target boots once, and `booted_programs` increments **once per group** (`rust/src/pass2.rs:200-216`). Mixed groups fall through to normal winner selection.
8. **Self-targeting wraps exist.** On 1-wide/1-tall grids, `Grid::neighbor` can return the source cell itself. In-place reads stay correct because read resolution never mutates program code, only the reader's stack/registers — but keep this case in mind and in tests.
9. **Rayon collect order is deterministic**: `par_iter_mut().enumerate().filter_map(...).collect()` preserves iteration order, and `canonicalize_pass1_output` re-sorts anyway. Tuple-sum reductions over `u32` are associative/commutative, so `map(...).sum()`/`reduce` are thread-count-independent.

## Step 2: clone-free Pass 2

Today `pass2_nonlocal` deep-clones the grid three times per tick: `pre_pass2` (`rust/src/pass2.rs:35`), `exclusive_base` (`:172`), and `working` (`:173`). A populated `Cell` holds a `Program` with heap-allocated code and stack vectors, so these are the dominant Pass 2 cost (54–90% depending on workload, per the 2026-08-11 profile). All three clones can be removed by pre-staging the few values that genuinely need pre-Pass-2 state, then mutating the live grid in the exact order the working grid is mutated today.

New shape:

```rust
pub fn pass2_nonlocal(grid, actions, tick, seed) -> Pass2Output {
    let mut output = Pass2Output::new(grid.len());
    if actions.is_empty() {
        return output;
    }
    let staged = stage_exclusive(grid, actions);   // A: immutable pre-resolution
    resolve_reads(grid, actions);                  // B: in place
    resolve_additive_transfers(grid, actions);     // C: two-phase, in place
    resolve_exclusive(grid, staged, tick, seed, &mut output); // D: in place
    output
}
```

**Phase A — stage exclusives against the unmutated grid.** Iterate actions; for each exclusive-class action run the existing `validate_exclusive` logic against `grid` (which still holds pre-Pass-2 state). Valid candidates are stored with everything `apply_winner` currently reads from `pre_pass2`:

- existing fields: `action`, `source`, `target`, `strength`, `size`;
- new: `target_strength: u32` — for `DelAdj`, `min(target size, target free_energy)` captured now (additive transfers may change the target's energy before apply time);
- new: `target_has_program: bool` — for `AppendAdj`, whether the target held a program pre-Pass-2 (decides append-to-program vs. deferred create).

Invalid exclusive actions go into an `invalid_sources: Vec<usize>` list instead of being flagged immediately, so phase A stays purely immutable. Flag timing is unobservable within Pass 2 (nothing in Pass 2 reads flags), so applying these flags at the start of phase D produces identical end-of-tick state.

Reads and additive transfers do not change anything validation reads (program presence, code length, openness, liveness) **except** free resources — which is exactly why staging must happen before phase C applies transfer deltas. This matches the current code, which validates against the `pre_pass2` snapshot.

**Phase B — reads in place.** Same body as today's `resolve_reads`, but read the target byte from `grid` instead of the snapshot: capture `Option<i16>` (the code byte, or None for empty target) immutably, then mutate the source cell. Equivalent because read resolution mutates only reader stacks/registers/flags and never program code, so later reads observe unchanged target code — including self-reads on wrapping grids.

**Phase C — additive transfers, two-phase and sparse.** Replace the four grid-sized `vec![0_u32; len]` in/out arrays with:

1. Scan pass (no resource mutation): for each `GiveE`/`GiveM` with `amount > 0`, compute `transferable = min(amount, source's current free resource)` — the grid resource values are still pre-Pass-2 at this point — push `(source, target, transferable)` onto a reused transfer list per resource kind, and `set_flag(source, false)` exactly where the current code does (flags are registers, not resources, so setting them during the scan cannot perturb later caps).
2. Apply pass: walk the transfer list applying `source -= t; target += t`.

Equivalence: each transfer's cap is computed against pre-Pass-2 values (identical to today's snapshot read), and final per-cell balances are `initial - sum(out) + sum(in)` either way; u32 addition order does not change the result. A source's outgoing amounts are capped by its pre-state balance, and incoming amounts only add, so incremental application cannot underflow in any input where the current dense version does not already overflow-panic under debug assertions. Keep the shuffled-action property test (`property_invariants.rs`) green — sparse application is order-independent in exactly the way the dense arrays are. Fast path: if the scan finds no positive transfers, skip the apply pass.

**Phase D — exclusive resolution in place.** If there were no exclusive-class actions at all (no candidates and no invalid sources), return without touching anything — this is the profile doc's "skip exclusive setup" win, and on the dense-additive fixture it eliminates the whole phase. Otherwise:

1. Apply `flag = true` to `invalid_sources`.
2. Sort the candidate vec by `(target, source)` (`ExclusiveCandidate` is small and `Copy`). Iterate it as runs sharing a target: this replaces the `by_target: vec![Vec<usize>; grid.len()]` allocation entirely. Runs ascend by target — same group order as today's `0..len` scan — and within a run candidates ascend by source; winner identity is order-independent anyway (fact 6), and loser flagging/boot handling are order-insensitive within a group.
3. For each run: boot-only special case, else `choose_winner` (adapted to a slice run; keep the tie sort and weighted roll byte-for-byte) and `apply_winner` mutating `grid` directly. `apply_winner` keeps reading the *current* grid for the apply-time resource checks and target cursor math (fact 4; target code size cannot have changed since staging because only its own group's single winner may mutate it), and uses the staged `target_strength` / `target_has_program` where it used `pre_pass2`.
4. Deferred commits, unchanged in spirit: collect `MoveCommit`/`AppendCreateCommit` during winner application, then apply moves, then creates. `apply_move_commit` no longer takes `exclusive_base`: `take()` the source's program and free resources, add them to the target, and leave the source's `bg_*` untouched (fact 2 makes the old restore a no-op).

In-place mutation is observationally identical to the working-grid version because after `working` is created today, nothing reads `grid` again — every read during resolution comes from `working`, `pre_pass2` (now staged fields), or `exclusive_base` (now known no-ops) — and `*grid = final_grid` unconditionally replaces the grid at the end.

Scratch reuse (optional, measure before keeping): the candidate/transfer/move/create vectors are small but per-tick; if allocation shows up in re-profiling, thread a reusable scratch struct through `Simulation` without changing `pass2_nonlocal`'s public signature (e.g., an internal `pass2_nonlocal_with_scratch` that the public fn wraps with a fresh scratch). Do not add a permanent double-buffered grid; the profile doc explicitly rejects it.

## Step 3: focused Pass 2 tests

Add to `rust/tests/pass2_nonlocal.rs` (use the existing `WorldBuilder`/`ProgramBuilder` helpers), covering every new fast path and the equivalence edge cases found during design:

1. **No actions**: empty action slice leaves the grid bit-identical (compare whole `Grid` equality against a clone) and returns a default `Pass2Output`.
2. **Additive-only**: `GiveE`+`GiveM` mix, no exclusives — exercises the phase-D skip; assert transfers, flags, and that untouched cells (including `bg_*`) are unchanged.
3. **Invalid exclusives only**: e.g., `WriteAdj` at a closed live target and `Move` into an occupied cell — assert `flag == true` on sources and no other mutation.
4. **Successful moves**: move into empty cell carrying free resources; assert source keeps only its `bg_*`, target receives program + resources; include a 1-row grid and a listener-mover receiving a `WriteAdj` before its move commits (the deferred-commit ordering from fact 5: moved program must carry the written byte).
5. **Conflicting exclusives**: mixed-class group at one target (e.g., `Move` vs `AppendAdj` into the same empty cell) plus a strength tie broken by the salted RNG — assert winner/loser flags and that the statistical tie test at the bottom of the file still holds.
6. **DelAdj strength capture**: target receives a `GiveE` in the same tick that raises its energy above the pre-Pass-2 value; assert the winner's cost uses the *pre*-transfer target strength (this is the `target_strength` staging in action) while the source's affordability check uses its *post*-transfer energy.
7. **Boot-only group with multiple booters**: `booted_programs` increments once, all sources get `flag = false`.

Cross-cutting: a randomized old-vs-new differential is unnecessary once the rewrite lands (there is no old implementation to compare against in-tree), so these fixtures plus the existing 24 pass2 tests, the shuffled-order property test, the conservation suite, and the parity script are the safety net. Run the parity script at extra depth once during this step: `./scripts/check-rayon-parity.sh 1000 1 2 4 8`.

## Step 4: fuse Rayon Pass 3 traversals

All in `rust/src/pass3.rs`, inside the existing `#[cfg(feature = "rayon")]` blocks only; the per-cell helper functions (`resolve_background_radiation_cell`, `resolve_collect_cell`, `resolve_background_mass_cell`, `resolve_inert_lifecycle_cell`, `resolve_maintenance_cell`, `resolve_free_resource_decay_cell`, `resolve_age_update_cell`, `resolve_spontaneous_creation_cell`) stay exactly as they are and are shared by both shapes.

**4a. Ambient fusion.** In `pass3_ambient`'s Rayon path, replace the three separate launches (radiation, collect, background mass) with one `par_iter_mut().zip(spawn_candidates).enumerate()` traversal calling radiation → collect → mass per cell in that order. `resolve_absorb` stays serial and ahead of it, unchanged. Safety: all three stages are cell-local (no neighbor reads), use per-cell RNG streams with distinct salts, and the per-cell stage order matches the current phase order; cross-cell interleaving is unobservable. Note this goes one stage further than the profile doc's validated A/B (which fused only collect+mass); the safety argument is identical, and the parity script is the arbiter.

**4b. Tail fusion.** In `pass3_tail`'s Rayon path, one traversal per cell: inert lifecycle → maintenance → decay → age → spawn, exactly the current stage order, producing `(deaths, births)` tuples reduced with tuple sums (deterministic per fact 9). The serial path keeps its five separate loops.

**4c (optional, measure first). Fold `mutate_end_of_tick` + newborn-clear into the tail.** Both are cell-local and run immediately after the tail in `run_tick_report`; folding them into the fused Rayon traversal saves two more pool launches (~25 µs/tick on the 64x64 fixture per the old profile). Cost: `pass3_tail` grows a `mutations` output and the driver stops calling `mutate_end_of_tick`/`clear_newborn_flags` separately, which changes `Pass3TailOutput` and moves `clear_newborn_flags` out of `simulation.rs` — an API reshuffle affecting the exported `mutate_end_of_tick` (keep it exported for tests) and the `TickPhase::Mutation`/`NewbornClear` phases (they would read ~0). Per-cell order must be lifecycle → maintenance → decay → age → spawn → mutate → clear-newborn; mutation only checks `was_live_at_tick_start` and program presence, so per-cell sequencing is equivalent to the current global sequencing. Do 4a+4b first, re-profile, and only take 4c if launch overhead still dominates the tail.

**4d (optional). Fuse Pass 1's reset into its collect traversal** (`rust/src/pass1.rs`): the Rayon path currently launches `reset_cells_for_pass1` and then `collect_pass1_cell_outputs`. A single traversal that resets the cell's tick state and then executes it if live is equivalent (execution reads only the immutable snapshot plus its own cell) and saves one launch. Serial path unchanged.

## Step 5: shared-path allocation trims (both builds, bit-identical)

These touch code shared by serial and Rayon builds, so they demand the same rigor as step 2.

**5a. `pass3_packets` bucketing** (`rust/src/pass3.rs:52-88`): today every tick with packets allocates `vec![Vec<Packet>; grid.len()]` plus a survivors vec. Replace with: advance positions in place; **stable** sort packets by `position` (`sort_by_key` is stable, so within-cell order equals the old bucket push order, which was drain order); walk runs of equal position applying the existing listen/collision/survivor logic; compact survivors in place (e.g., `retain`-style write index) instead of a new vec. Equivalences to preserve: run iteration ascends by cell index exactly as the old `0..len` bucket scan; the listen-capture RNG draw and `packets[choice]` indexing must see the same within-run ordering; survivor order (position-ascending, then original order within a cell) is observable across ticks and in frame encodings, and the stable sort reproduces it. The `packets.is_empty()` early return stays.

**5b. `resolve_absorb`** (`rust/src/pass3.rs:185-242`): first scan for any program with `absorb_count > 0`; return early if none (saves two grid-sized allocations every tick on absorb-free workloads, including all four bench fixtures except frontend). Otherwise collect `(footprint_cell, source)` pairs into a small vec, stable-sort by footprint cell, and process runs — pair generation order is source-ascending, so within-cell order after a stable sort matches the old bucket order, and the share/remainder math is untouched. The `gains` array can stay a grid-sized `vec![0_u32]` (one cheap calloc) or become sparse pairs; either is fine, gains application order is commutative.

## Step 6: verification gates

- `cargo test` and `cargo test --features rayon` — all suites, both feature sets.
- `cargo clippy --all-targets --all-features` and `cargo fmt` clean.
- `./scripts/check-rayon-parity.sh` (default 300 ticks) green at threads 1 2 4 8, plus one deep run `./scripts/check-rayon-parity.sh 1000 1 2 4 8` after steps 2 and 5.
- The parity script's serial control run also guards cross-process reproducibility — if *it* fails, the regression is nondeterminism (hash order, address dependence), not Rayon.
- New step-3 tests green in both feature sets.

## Step 7: re-benchmark and close out

- Quiet machine (check `uptime` / no game running — this sank the 2026-08-12 Rayon baseline). Re-record serial and Rayon (threads 2, 4, 8) with `tick_bench 1000 5`, after each structural step and at the end; savings from separate experiments do not add linearly.
- Report absolute µs/tick and phase proportions for all five fixtures, serial vs. Rayon, in a new dated `docs/analysis/` results note; compare against the serial baseline table above and re-baseline serial too (same session).
- Success criteria (from the 2026-08-11 profile): Pass 2 drops by roughly its measured clone share (~28% on frontend-style, ~64% on dense additive were achieved by the *partial* A/B; full clone removal should beat both); fused Pass 3 tail roughly halves; frontend-64x64 Rayon-vs-serial advantage grows from ~1.2x toward the ~1.4x+ the A/B combination implied; no serial regression beyond noise on any fixture.
- Update `STATUS.md`: mark `RAYON-PERFORMANCE` done; revisit `RAYON-BASELINE` (whether the serial `cfg` paths are still worth keeping) with fresh numbers in hand and leave it or update it accordingly; add follow-ups to `docs/BACKLOG.md` (candidates: parallel packet bucketing, parallel absorb, Pass 2 scratch reuse if skipped, snapshot-based Pass 1 prepare fusion).

## Known pitfalls, restated

- Do not benchmark the stale example binary; feature sets share one output path.
- Debug-assertion builds (plain `cargo test`) panic on u32 overflow; keep arithmetic shapes equivalent so pathological hand-built action lists panic (or not) the same way before and after.
- `is_open` live listeners mean sources can be targets; never "optimize" based on source/target disjointness.
- `booted_programs` counts once per boot-only *group*, not per candidate.
- The tie-break RNG (`EXCLUSIVE_TIE_SALT`) and append-create RNG (`APPEND_CREATE_SALT`) draw sequences must be preserved exactly — no added or removed draws anywhere in Pass 2/Pass 3.
- Keep pass boundaries explicit in code structure (`rust/AGENTS.md`); fused Rayon traversals should be clearly-named functions that document the per-cell stage order they preserve.
