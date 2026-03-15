# Codex Rust Implementation Plan

## Purpose

This plan is for the active Rust backend in `rust/`. It assumes:

- `legacy/rust/` is fully deprecated and not a migration target.
- [docs/SPEC.md](../../docs/SPEC.md) and [docs/SPEC-COMPANION.md](../../docs/SPEC-COMPANION.md) are the behavioral source of truth.
- [docs/BACKEND-GUIDELINES.md](../../docs/BACKEND-GUIDELINES.md) is the implementation-structure guide.
- [docs/BACKEND-TESTING.md](../../docs/BACKEND-TESTING.md) is part of the execution plan, not an afterthought.

## Current Status

Completed in the new `rust/` crate:

- config validation and spec-aligned defaults
- core data model (`Cell`, `Program`, `Registers`, `TickState`, `Packet`, `QueuedAction`)
- flat toroidal `Grid`
- deterministic RNG helpers and basic binomial sampling
- `Simulation` shell with Pass-0 snapshot / `live_at_tick_start`
- opcode decoding and metadata for all 71 spec opcodes
- reusable test helpers (`WorldBuilder`, `ProgramBuilder`, `assert_cell!`, `assert_program!`, `diff_grids(...)`)
- reusable multi-tick helper (`run_ticks(...)`)
- single-threaded Pass 1 local VM with nonlocal queue attempts
- single-threaded Pass 2 resolution in class order
- hardened Pass-2 truth-table, pass-boundary, and edge-case integration coverage
- Pass-3 packet phase (propagation, listening, collision)
- Pass-3 ambient-pool phase (`absorb`, background radiation decay/arrival, `collect`, background mass decay/arrival, spawn-candidate marking)
- Pass-3 lifecycle/economy tail (inert lifecycle, maintenance, free-resource decay, age update, spontaneous creation)
- end-of-tick mutation for `live_at_tick_start`
- real single-threaded tick driver in `Simulation::run_tick()`
- deterministic `run_tick()` replay coverage
- zero-rate conservation coverage for full-tick internal transfers
- multi-tick regression coverage for packet wrapping and absorb/collect arrival ordering
- randomized invariant coverage for toroidal wrapping, snapshot-isolated sensing, and additive-transfer order independence
- direct Pass-1 stack-boundary coverage for overflow/underflow behavior
- deterministic multi-tick scenarios for absorb accumulation and inert-grace expiry
- weighted-tie sanity coverage for exclusive Pass-2 conflicts
- deterministic multi-tick scenarios for repeated `delAdj` pressure and extinction-to-respawn flow

Validation baseline currently expected after each implementation batch:

- `cargo fmt`
- `cargo clippy --all-targets --all-features`
- `cargo test`

## Planning Principles

1. Follow the build order in [docs/BACKEND-GUIDELINES.md:381](../../docs/BACKEND-GUIDELINES.md#L381) through [docs/BACKEND-GUIDELINES.md:396](../../docs/BACKEND-GUIDELINES.md#L396).
2. Treat the companion test list in [docs/SPEC-COMPANION.md:543](../../docs/SPEC-COMPANION.md#L543) through [docs/SPEC-COMPANION.md:589](../../docs/SPEC-COMPANION.md#L589) as the minimum correctness checklist.
3. Build test helpers early, as recommended in [docs/BACKEND-TESTING.md:7](../../docs/BACKEND-TESTING.md#L7) through [docs/BACKEND-TESTING.md:40](../../docs/BACKEND-TESTING.md#L40).
4. Keep the simulator single-threaded until Pass 3 correctness is stable, per [docs/BACKEND-GUIDELINES.md:396](../../docs/BACKEND-GUIDELINES.md#L396).
5. Preserve explicit pass boundaries in code and tests; snapshot isolation and pass ordering are the hardest correctness constraints.

## Milestones

### M0. Foundation

Status: completed

Scope:

- crate scaffolding
- core types
- config
- deterministic RNG
- grid math
- Pass-0 preparation
- opcode metadata

Exit criteria:

- baseline tests remain green
- crate shape supports Pass 1 without structural refactor

### M1. Test Harness First

Status: completed

Build the reusable testing infrastructure before deeper VM work, following [docs/BACKEND-TESTING.md:9](../../docs/BACKEND-TESTING.md#L9) through [docs/BACKEND-TESTING.md:40](../../docs/BACKEND-TESTING.md#L40).

Deliverables:

- `WorldBuilder`
- `ProgramBuilder`
- `assert_cell!` and `assert_program!`
- `run_ticks(...)`
- `diff_grids(...)`
- initial test folder shape aligned with [docs/BACKEND-TESTING.md:42](../../docs/BACKEND-TESTING.md#L42) through [docs/BACKEND-TESTING.md:65](../../docs/BACKEND-TESTING.md#L65)

Reason:

- raw struct setup will become too noisy once Pass 1 and Pass 2 tests start
- determinism and regression debugging need `diff_grids` from the start

Exit criteria:

- one snapshot test and one grid diff test use the helpers instead of raw structs
- `run_ticks(...)` lands before the first real multi-tick integration scenarios in M4

### M2. Pass 1 Local VM

Status: completed

Implement the single-threaded Pass 1 interpreter, matching the spec and companion exactly.

Scope:

- local action budget
- base-cost payment
- stack helpers and bounds behavior
- register operations
- local opcodes
- local sensing from Pass-0 snapshot
- open-cell flags for `nop`, `listen`, failed energy payment, and inert state
- packet emission
- nonlocal queue attempts

Important semantic rule to preserve:

- after successful base-cost payment, the first nonlocal instruction always ends Pass 1 for that program, even if operand capture fails

Primary tests:

- single-instruction micro-worlds per [docs/BACKEND-GUIDELINES.md:377](../../docs/BACKEND-GUIDELINES.md#L377)
- `Flag` and payment checklist in [docs/SPEC-COMPANION.md:545](../../docs/SPEC-COMPANION.md#L545) through [docs/SPEC-COMPANION.md:564](../../docs/SPEC-COMPANION.md#L564)
- snapshot isolation property from [docs/BACKEND-TESTING.md:91](../../docs/BACKEND-TESTING.md#L91) through [docs/BACKEND-TESTING.md:98](../../docs/BACKEND-TESTING.md#L98)
- stack invariants from [docs/BACKEND-TESTING.md:100](../../docs/BACKEND-TESTING.md#L100) through [docs/BACKEND-TESTING.md:107](../../docs/BACKEND-TESTING.md#L107)
- idempotency tests for `absorb`, `collect`, `listen` from [docs/BACKEND-TESTING.md:117](../../docs/BACKEND-TESTING.md#L117) through [docs/BACKEND-TESTING.md:123](../../docs/BACKEND-TESTING.md#L123)

Exit criteria:

- all local opcode categories have focused unit tests
- the failed-nonlocal-capture edge case is covered directly
- Pass 1 behavior is stable enough to support Pass 2 without redesign

### M3. Pass 2 Nonlocal Resolution

Status: completed for the current core slice

Implement Pass 2 in class order:

1. read-only
2. additive transfer
3. exclusive

Scope:

- pre-Pass-2 validation view
- queued-action resolution
- additive transfer summation
- exclusive conflict resolution
- boot multi-success special case
- no-fallback winner rule
- success-only cursor side effects

Primary tests:

- companion Pass-2 checklist in [docs/SPEC-COMPANION.md:566](../../docs/SPEC-COMPANION.md#L566) through [docs/SPEC-COMPANION.md:574](../../docs/SPEC-COMPANION.md#L574)
- pass-boundary tests from [docs/BACKEND-TESTING.md:190](../../docs/BACKEND-TESTING.md#L190) through [docs/BACKEND-TESTING.md:210](../../docs/BACKEND-TESTING.md#L210)
- Pass-2 commutativity properties from [docs/BACKEND-TESTING.md:125](../../docs/BACKEND-TESTING.md#L125) through [docs/BACKEND-TESTING.md:130](../../docs/BACKEND-TESTING.md#L130)

Deferred but related:

- fuzz the conflict resolver per [docs/BACKEND-TESTING.md:150](../../docs/BACKEND-TESTING.md#L150) through [docs/BACKEND-TESTING.md:155](../../docs/BACKEND-TESTING.md#L155) once the API settles

Exit criteria:

- Pass 2 tests cover all conflict classes and pre-Pass-2 visibility rules
- no ambiguity remains around queue payloads or winner selection

Optional hardening before or during M4:

- completed:
  - nonpositive `giveE` / `giveM` tested as `Flag`-neutral no-ops
  - `delAdj` stored-`IP` adjustment covered directly per [docs/SPEC-COMPANION.md:588](../../docs/SPEC-COMPANION.md#L588)
  - size-1 `delAdj` failure covered directly per [docs/SPEC-COMPANION.md:591](../../docs/SPEC-COMPANION.md#L591)
  - protected-target rejection for `writeAdj` / occupied-target `appendAdj` covered directly
  - size-cap `appendAdj` failure covered directly
  - deterministic winner selection under reordered action lists covered for equal-strength conflicts
  - weighted-tie selection sanity-covered over many trials for unequal-size equal-strength conflicts
- still worth adding later if needed:
  - explicit `giveE` / `giveM` cap-at-available tests at extreme requested values

User note:

- keep the weighted-tie frequency sanity pass on the backlog, but do not block Pass 3 on it

### M4. Pass 3 Physics, Lifecycle, and Mutation

Status: mechanically complete, regression coverage materially expanded; fuzz/property-style work still open

Implement Pass 3 in strict order, then end-of-tick mutation.

Scope:

- directed radiation propagation / listening / collision
- `absorb` distribution
- background radiation decay then arrival
- `collect`
- background mass decay then arrival
- inert abandonment timer update
- maintenance
- free-resource decay
- age update
- spontaneous creation
- mutation for programs in `live_at_tick_start`

Primary tests:

- lifecycle and deletion checklist in [docs/SPEC-COMPANION.md:576](../../docs/SPEC-COMPANION.md#L576) through [docs/SPEC-COMPANION.md:589](../../docs/SPEC-COMPANION.md#L589)
- conservation properties from [docs/BACKEND-TESTING.md:73](../../docs/BACKEND-TESTING.md#L73) through [docs/BACKEND-TESTING.md:89](../../docs/BACKEND-TESTING.md#L89)
- multi-tick integration scenarios from [docs/BACKEND-TESTING.md:166](../../docs/BACKEND-TESTING.md#L166) through [docs/BACKEND-TESTING.md:186](../../docs/BACKEND-TESTING.md#L186)
- edge-case coverage from [docs/BACKEND-TESTING.md:242](../../docs/BACKEND-TESTING.md#L242) onward as each subsystem lands

Deferred but related:

- maintenance fuzz target per [docs/BACKEND-TESTING.md:157](../../docs/BACKEND-TESTING.md#L157) through [docs/BACKEND-TESTING.md:162](../../docs/BACKEND-TESTING.md#L162)

Exit criteria:

- one full tick executes end-to-end
- conservation holds in zero-rate worlds
- lifecycle/newborn semantics are regression-tested
- longer-horizon deterministic scenarios cover resource accumulation and grace-window maintenance timing
- repeated `delAdj` pressure and extinction-to-respawn flows are regression-tested

Implementation note:

- the tick driver now freezes both `live_at_tick_start` and `program_existed_at_tick_start`; this follows the lifecycle/Pass-3 maintenance text where execution/aging/mutation are live-only, but maintenance can still apply to inert programs that existed at tick start once abandoned

### M5. Determinism and Replicator Gates

After the whole single-threaded tick exists, add the higher-level gates from [docs/BACKEND-GUIDELINES.md:373](../../docs/BACKEND-GUIDELINES.md#L373) through [docs/BACKEND-GUIDELINES.md:379](../../docs/BACKEND-GUIDELINES.md#L379).

Scope:

- same-seed determinism tests
- `diff_grids`-backed diagnostics
- seed replicator smoke test
- multi-tick ecology sanity checks
- additional stochastic-accounting and fuzz-style stress coverage once the seed is settled

Primary tests:

- determinism test from [docs/BACKEND-GUIDELINES.md:373](../../docs/BACKEND-GUIDELINES.md#L373) through [docs/BACKEND-GUIDELINES.md:375](../../docs/BACKEND-GUIDELINES.md#L375)
- seed smoke test from [docs/BACKEND-GUIDELINES.md:379](../../docs/BACKEND-GUIDELINES.md#L379)
- population / extinction / resource-accumulation scenarios in [docs/BACKEND-TESTING.md:170](../../docs/BACKEND-TESTING.md#L170) through [docs/BACKEND-TESTING.md:186](../../docs/BACKEND-TESTING.md#L186)

Exit criteria:

- deterministic replay is stable
- the seed organism can run in its intended preloaded environment

### M6. Parallelism

Only after single-threaded correctness is stable.

Scope:

- add Rayon to Pass 0, Pass 1, and suitable Pass 3 steps
- keep Pass 2 sequential unless profiling says otherwise

Primary tests:

- the single-threaded vs multi-threaded golden test from [docs/BACKEND-TESTING.md:214](../../docs/BACKEND-TESTING.md#L214) through [docs/BACKEND-TESTING.md:238](../../docs/BACKEND-TESTING.md#L238)
- thread-count variation using `RAYON_NUM_THREADS`

Exit criteria:

- same seed/config yields identical results across single-threaded and Rayon modes

### M7. Hardening and Tooling

Scope:

- property-based tests with `proptest`
- fuzz targets with `cargo-fuzz`
- metrics output
- snapshots

Priority order:

1. conservation properties
2. VM fuzz target
3. Pass-2 conflict fuzz target
4. maintenance fuzz target
5. observer/output modules

References:

- property testing: [docs/BACKEND-TESTING.md:69](../../docs/BACKEND-TESTING.md#L69) through [docs/BACKEND-TESTING.md:130](../../docs/BACKEND-TESTING.md#L130)
- fuzzing: [docs/BACKEND-TESTING.md:135](../../docs/BACKEND-TESTING.md#L135) through [docs/BACKEND-TESTING.md:162](../../docs/BACKEND-TESTING.md#L162)
- CI / execution order: [docs/BACKEND-TESTING.md:278](../../docs/BACKEND-TESTING.md#L278) through [docs/BACKEND-TESTING.md:295](../../docs/BACKEND-TESTING.md#L295)

## Immediate Next Slice

1. Continue expanding the M4 regression suite where it still has visible gaps:
   - longer-horizon accumulation / steady-state scenarios
   - extinction and respawn behavior over many ticks
   - stronger conservation checks across richer worlds
2. Move into M5 with a broader determinism harness and then the seed smoke tests.
3. Add any remaining edge-case coverage revealed by the new tick driver, especially around spontaneous creation interacting with crowded or resource-rich worlds.

## Things Not To Do

- Do not pull patterns from `legacy/rust/`.
- Do not parallelize early.
- Do not blur pass boundaries in code structure.
- Do not mix observer/output concerns into tick execution before correctness gates are in place.
- Do not defer test-helper work; the testing guide explicitly argues for building it early.
