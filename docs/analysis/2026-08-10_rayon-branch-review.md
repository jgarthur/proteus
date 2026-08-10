# Review: `proteus-para` (deterministic Rayon execution paths)

Date: 2026-08-10
Branch: `proteus-para` (5 commits, `ef11c0d..28c99c1`)
Reviewer: Claude Opus 5

## Scope

| Commit | Summary |
| --- | --- |
| `8301afd` | Add deterministic Rayon execution paths |
| `4f0daf0` | Tighten Rayon follow-ups and CI coverage |
| `588674a` | Add thread-count option to dev launcher |
| `bd61183` | Fix websocket subscribe replay on current payload |
| `28c99c1` | Enable Rayon feature when dev launcher sets threads |

954 insertions / 264 deletions across 12 files. Two unrelated threads of work: the
Rayon parallelization (`pass1.rs`, `pass3.rs`, `simulation.rs`, tests, CI, launcher)
and an independent WebSocket subscribe-replay fix (`web/ws.rs`).

## Verdict

The parallelization is **correct and semantics-preserving**, and I verified that
empirically rather than by inspection alone (method below). The design is sound: the
refactor makes cross-cell writes *impossible by construction* inside parallel regions,
which is a much stronger guarantee than "we checked the loops don't alias."

There is one issue I would fix before merging (CI no longer compiles or tests the
default build), one test-coverage gap against the project's own documented plan, and a
benchmark whose printed number overstates the benefit by ~40%.

---

## Verification performed

Because the serial and Rayon paths are mutually exclusive at compile time
(`#[cfg(feature = "rayon")]` / `#[cfg(not(...))]`), a single test binary *cannot*
compare them. I built an out-of-tree harness that links the crate twice — once with
default features, once with `--features rayon` — and hashed the full grid, the packet
list, and every per-tick `TickReport` after N ticks. I also built the same harness
against the merge base to check the refactor against pre-branch behavior.

Two fixtures: the branch's own 64×64 lithotroph seed, and a deliberately harsher
96×96 fixture (`p_spawn = 0.25`, `mutation_base_log2 = 4`, mixed live/inert programs,
`EMIT`/`LISTEN`/`ABSORB`/`COLLECT`/`GIVE_E`/`GIVE_M`, non-zero ambient everywhere) to
exercise the spontaneous-creation and mutation paths that the in-repo fixtures leave
mostly idle.

Result — all bit-identical:

```
merge base, serial      grid=2495bd5270d659bc reports=d5c58b99d9865a45 packets=6d14e2dbe4e1fc8e
branch,     serial      grid=2495bd5270d659bc reports=d5c58b99d9865a45 packets=6d14e2dbe4e1fc8e
branch,     rayon t=1   grid=2495bd5270d659bc reports=d5c58b99d9865a45 packets=6d14e2dbe4e1fc8e
branch,     rayon t=2   (identical)
branch,     rayon t=4   (identical)
branch,     rayon t=8   (identical)
branch,     rayon t=16  (identical)
```

So: the branch does not change simulation results, and the Rayon path agrees with the
serial path across thread counts on a fixture much harsher than anything in the test
suite. `cargo test` (default features) and `cargo test --all-features` both pass
locally; `cargo clippy --all-targets` is clean in both configurations.

### Why the design is safe

- **Aliasing is ruled out by types.** `execute_local_instruction` and the nine new
  `*_cell` helpers in `pass3.rs` take `&mut Cell`, not `&mut Grid`. A parallel closure
  physically cannot reach a neighbour cell. This is the single best decision in the
  branch.
- **RNG is order-independent.** Every parallelized step derives its stream from
  `cell_rng(seed ^ SALT, tick, cell_index)`, so scheduling cannot perturb it.
- **Reductions are associative.** The `deaths` / `births` / `mutations` counters use
  `u32` `.sum()`; integer addition reorders safely. (A `f64` reduction here would have
  been a real determinism bug.)
- **Pass 1 merge order is canonicalized.** `canonicalize_pass1_output` sorts by
  `source_index` before flattening, which addresses the risk called out by name in
  `docs/BACKEND-TESTING.md` §6 ("the merge order must not affect Pass 2 results").
- **The genuinely order-dependent work stayed sequential** — Pass 2, packet
  propagation, and absorb resolution. Correctly scoped first cut.

---

## Findings

### 1. CI no longer builds or tests the default configuration — fix before merge

`4f0daf0` changed both steps in `.github/workflows/rust.yml`:

```yaml
- run: cargo build --all-features --verbose
- run: cargo test  --all-features --verbose
```

Previously these ran with default features (`default = []`). Every step that resolves
features — clippy, build, test — now runs with `rayon` enabled, so the
**`#[cfg(not(feature = "rayon"))]` blocks are never compiled in CI.** (`cargo fmt` is
the one step still covering them, since rustfmt formats `cfg`'d-out code — but it
checks layout, not types or behavior.) This branch just created 13 such blocks:

| File | serial/rayon block pairs |
| --- | --- |
| `rust/src/pass1.rs` | 2 |
| `rust/src/pass3.rs` | 9 |
| `rust/src/simulation.rs` | 2 |

A type error, a stale variable, or a semantic drift in any of those 13 serial blocks
would sail through CI. And that path is what actually ships: `default = []`,
`rust/README.md` says "The default build remains single-threaded", and `start-dev.sh`
runs `--features web` unless `--threads` is passed.

This also contradicts the project's own documented pipeline in
`docs/BACKEND-TESTING.md` §9, which specifies *both*:

> 1. `cargo test` — all unit + integration tests
> 2. `cargo test --features rayon` — parallelism correctness (the golden determinism test)

Suggested fix — keep the all-features steps and restore the default-feature ones:

```yaml
- name: Build
  run: cargo build --verbose
- name: Build (all features)
  run: cargo build --all-features --verbose
- name: Run Tests
  run: cargo test --verbose
- name: Run Tests (all features)
  run: cargo test --all-features --verbose
```

Adding `cargo clippy --all-targets -- -D warnings` (no `--all-features`) would close
the lint half of the same gap.

### 2. The "golden test" the docs ask for is not implemented

`docs/BACKEND-TESTING.md` §6 defines the golden test as **single-threaded ==
multi-threaded**. All three new tests in `rust/tests/determinism.rs` compare
Rayon-against-Rayon at different thread counts. That is a useful test — it catches
scheduling-dependent bugs — but it cannot catch the failure mode where the
`#[cfg(feature = "rayon")]` branch and its `#[cfg(not(...))]` twin drift apart, which
is precisely the risk that 13 duplicated loop pairs introduce.

As noted above, the cfg-exclusivity means one binary can't do this comparison. The
practical form is a **golden digest test that runs in both configurations**: hash the
grid + packets + tick reports after a fixed replay and assert against a committed
constant. The same test then runs under `cargo test` and `cargo test --features rayon`,
and CI (once finding 1 is fixed) fails if either path moves. My harness confirms the
digests currently agree, so the constant can be captured today.

Two smaller coverage gaps against §6, which asks for sparse / moderate / dense configs:

- The 64×64 fixture sets `p_spawn = 0.0`, so `resolve_spontaneous_creation` — one of
  the parallelized reductions — is never meaningfully exercised at scale. The 5×2
  fixture sets `p_spawn = 0.5` but has only 10 cells.
- There is no dense fixture (§6 asks for "32×32, every cell has a program").

My 96×96 stress fixture covers both and passes, so this is a coverage gap rather than a
latent defect — but it is worth landing as a test.

### 3. The benchmark's printed speedup overstates the benefit by ~40%

`frontend_default_seed_ecology_is_deterministic_for_long_rayon_replay` prints:

```
frontend-seed determinism benchmark: ticks=1000 single_thread=958ms rayon_4=554ms speedup=1.73x
```

The variable named `single_thread` is a **Rayon pool with `num_threads(1)`**, not the
serial build. Rayon-with-one-thread is measurably slower than the non-Rayon path, so
using it as the baseline inflates the ratio. Measured on this machine (4 performance
cores / 8 logical), best-of-N, same 64×64 fixture and 1000 ticks:

| Configuration | Time | vs. true serial |
| --- | --- | --- |
| serial (no `rayon` feature) | 0.90s | 1.00x |
| rayon, 1 thread | 1.06s | **0.85x** (slower) |
| rayon, 4 threads | 0.72s | **1.25x** |

The honest answer to "what do I get for enabling this feature" is **1.25x**, not 1.73x.
On the harsher 96×96 fixture (500 ticks):

| Configuration | Time |
| --- | --- |
| merge base, serial | 1.60s |
| branch, serial | 1.61s |
| branch, rayon t=1 | 1.74s |
| branch, rayon t=2 | 1.31s |
| branch, rayon t=4 | **1.18s** |
| branch, rayon t=8 | 1.30s |

Worth noting the good news in that table: **the branch costs the serial path nothing**
(1.60s → 1.61s), despite routing Pass 1 through per-cell output structs and a sort. I
expected a regression there and did not find one — hoisting `&mut Cell` out of the
instruction loop appears to pay for the extra marshalling.

Suggested change: rename the variable, and either compare against a committed baseline
or drop the ratio from the assertion-free `eprintln!`. A timing benchmark inside a
correctness test is also a mild smell — it makes the test ~1.5s and its output is
unactionable in CI.

**This directly informs the open `RAYON-BASELINE` decision.** The branch defers the
question of whether Rayon should replace the serial paths entirely (relying on a
1-thread pool when serial). The data says **no**: rayon t=1 is 8–18% *slower* than the
serial build on both fixtures. Collapsing the cfg split would mean paying that tax on
the default build. Either keep the split, or accept the regression deliberately.

### 4. `rust/README.md` — new section inserted mid-procedure

The "Optional Parallelism" section was spliced into the middle of "Running The Smoke
Test", orphaning its closing line:

```markdown
## Optional Parallelism
...
The default build remains single-threaded.

Then open `http://127.0.0.1:3000/debug/smoke`.     <-- belongs to the section above
```

Move "Optional Parallelism" below the smoke-test instructions.

---

## Minor / cleanup

- **`web/controller.rs:92,97`** — `current_frame()` and `current_metrics()` now have
  zero callers repo-wide; `ws.rs` was their only consumer. They are `pub`, so no
  dead-code warning fires. Remove, or keep deliberately as controller API.
- **`simulation.rs:17-19`** — `use rayon::prelude::*;` was placed *after* the
  `INITIAL_BG_*_SALT` consts, and the blank line separating imports from consts was
  removed. Cosmetic, but it breaks the import block.
- **`simulation.rs`** — `crate::model::Cell` is spelled out inline in two signatures
  while `CellSnapshot` is imported from the same module. Import `Cell`.
- **`pass1.rs:190`** — `canonicalize_pass1_output`'s `sort_unstable_by_key` is
  redundant: Rayon's `collect()` into `Vec` already preserves sequential order. It is
  cheap and documents intent, so keeping it is defensible — just worth knowing it is
  belt-and-braces rather than load-bearing.
- **`start-dev.sh`** — there is no way to enable the Rayon feature without pinning a
  thread count; `--threads N` is the only trigger. A bare `--rayon` flag that lets
  Rayon choose its own default would be the natural complement. Also note that
  toggling `--threads` flips the cargo feature set and forces a full rebuild each time.
- **`start-dev.sh:89`** — an invalid inherited `RAYON_NUM_THREADS` now aborts the
  launcher rather than being ignored. Probably intended; flagging as a behavior change.

---

## WebSocket fix (`bd61183`) — separate from the Rayon work

This one is correct and the reasoning in the commit message holds up. The change swaps
`controller.current_frame()` for `frame_rx.borrow_and_update()` on the subscribe path.
Since `current_frame()` was itself just `self.frame_rx.borrow().clone()` on the
controller's own receiver clone, the *value* read is unchanged — what is new is that
`borrow_and_update()` marks the payload seen on the session's receiver, so the
subsequent `changed()` no longer replays the frame that was already sent inline. The
`Ref` guard is correctly scoped to a block and dropped before the `await`, which
matters — holding it across the send would have been a `Send` problem.

One residual, pre-existing race worth noting rather than fixing here: a frame that has
been queued into `subscription.pending` behind the FPS throttle can still be delivered
after a destroy, because `tokio::select!` picks randomly among ready branches and the
`frame_timer` branch may win over `destroy_rx`. This fix narrows the window
substantially (the duplicate replay was the reliable trigger) but does not close it. If
tests still flake on "frame after destroy", that timer branch is where to look.

Unrelated to correctness: this commit is bundled into a branch otherwise about Rayon.
Worth splitting if the branch is not squash-merged.

---

## Recommendation

The core parallelization work is good and I would not hold it up. Findings 1, 2 and 4
have since been addressed (see addendum); finding 3 and the cleanup list remain.

Note that the measurements above answer the `RAYON-BASELINE` question in the negative:
a 1-thread Rayon pool is slower than the serial path, so the cfg split is currently
earning its keep.

---

## Addendum (same day): parity check automated

Findings 1, 2 and 4 are now closed:

- **`rust/examples/parity_digest.rs`** — replays three fixtures matching the configs
  `docs/BACKEND-TESTING.md` §6 asks for (sparse 8×8, moderate 64×64, dense 32×32) and
  prints FNV-1a digests of the final grid, the packet list, and every `TickReport`.
  Digests go to stdout, build configuration to stderr, so runs diff cleanly.
- **`rust/scripts/check-rayon-parity.sh`** — builds that example with default features
  and again with `--features rayon`, then diffs the digests at
  `RAYON_NUM_THREADS` 1/2/4/8. Exits non-zero with a per-fixture diff on divergence.
- **`.github/workflows/rust.yml`** — default-feature clippy/build/test steps restored
  alongside the all-features ones, plus a `Check Rayon Parity` step.
- **`rust/README.md`** — "Optional Parallelism" moved out of the middle of the
  smoke-test procedure and extended with a parity-check section.

### The harness was validated by fault injection

An automated check that cannot fail is worth nothing, so both were tested against
deliberately injected bugs before being trusted:

| Injected fault | Detected? |
| --- | --- |
| Off-by-one cell index into the RNG stream in the Rayon-only `resolve_free_resource_decay` block | **Yes** — all three fixtures diverged, at both 1 and 4 threads |
| `existed_set[cell_index]` → `true` in the Rayon-only `resolve_maintenance` block | **No** — no divergence at all |

The first is the classic parallel-refactor bug and the harness catches it cleanly.

### Incidental finding, now confirmed: moved programs escape end-of-tick bookkeeping

Forcing `existed_at_tick_start = true` in `resolve_maintenance` changed nothing across
three fixtures × 200 ticks. That was not a harness weakness — it meant no fixture in
the suite ever executes a successful `move`. Nothing under `rust/tests/` referenced
`op::MOVE` at all.

Following that thread surfaced a **pre-existing correctness bug**, since confirmed by
`rust/tests/move_eligibility.rs`. `SPEC.md:252` scopes end-of-tick eligibility to
*programs*:

> Also record the set of programs that are **live at tick start**. Only those programs
> are eligible to execute in Pass 1, pay maintenance this tick, age at end of tick, and
> mutate at end of tick.

The engine stores that set as position-indexed `Vec<bool>` masks over *cells*
(`live_set` / `existed_set`) — equivalent only while programs stay put. On a successful
move, `apply_move_commit` relocates the program to a cell whose masks describe that
cell's previous occupant (empty), while the source cell keeps `true` masks and no
program. Measured over one tick, with an identical stationary program as control:

| | maintenance charged | age | mutations |
| --- | --- | --- | --- |
| stationary | 2 quanta | 7 → 8 | 1 |
| moved | **0** | **7 → 7** | **0** |

The maintenance figure needs isolating from the `move` instruction's own cost: energy
spent is 1 at both `maintenance_rate = 0.0` and `1.0`, so the charge really is zero.

This predates the branch — the masks were position-indexed before the Rayon work — and
the parallelization neither caused nor worsened it.

**A minimal fix is not simply dropping `existed_set`.**
`apply_append_create_commit` creates an inert program in a previously empty cell
*without* setting `is_newborn`, so `existed_set` is currently what stops that program
being charged maintenance on the tick it was appended. Eligibility has to travel with
the program: either applied alongside `MoveCommit`, or — cleaner — stored on the
program's `TickState`, so relocation carries it naturally and the tick-end helpers stop
consulting position-indexed masks at all.

Tracked as `MOVE-ELIGIBILITY` in `STATUS.md` and `docs/BACKLOG.md`. The two failing
tests are committed as `#[ignore]` with the spec citation in the ignore reason, so the
suite stays green and the fix has a ready-made gate.
