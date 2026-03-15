# Rust Testing Handoff Prompt

Continue the single-threaded backend testing work for Proteus in `rust/`.

## Deliverable

Land the next testing tranche for the active Rust simulator, keeping the work scoped to engine correctness and regression coverage.

Target outcome:

- additional hand-authored tests for the remaining meaningful single-threaded gaps
- no semantic engine changes unless a test reveals a real bug that must be fixed
- updated implementation plan if the completed testing slice materially changes what remains

This is an execution task, not a design/spec-writing task.

## Source Hierarchy

Use these sources in this order:

1. `docs/SPEC.md`
2. `docs/SPEC-COMPANION.md`
3. `docs/BACKEND-TESTING.md`
4. `docs/BACKEND-GUIDELINES.md`
5. `.agents/plans/codex-rust-impl.md`
6. existing Rust code and tests under `rust/`

Authority rule:

- `SPEC.md` and `SPEC-COMPANION.md` define behavior.
- `BACKEND-TESTING.md` defines the testing strategy and high-value gaps.
- `BACKEND-GUIDELINES.md` is structural guidance.
- the plan is execution context, not authority over behavior.

If you find a conflict, do not silently resolve it in the docs. Note it in code comments or final notes and follow the behavioral spec.

## Current State You Should Assume

Already implemented and covered:

- active Rust engine in `rust/`; `legacy/rust/` is irrelevant
- single-threaded Pass 0 / 1 / 2 / 3 and `Simulation::run_tick()`
- helper layer: `WorldBuilder`, `ProgramBuilder`, `assert_cell!`, `assert_program!`, `run_ticks(...)`, `diff_grids(...)`
- deterministic replay test
- zero-rate conservation test
- snapshot-isolation and additive-transfer property-style tests
- Pass-2 truth-table and conflict coverage
- multi-tick coverage for absorb lag, inert grace expiry, repeated `delAdj`, extinction-to-respawn
- weighted exclusive-tie sanity test
- transfer-cap tests for large requested `giveE` / `giveM`
- dense packet-ring stress test

The suite was green at the handoff point with `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test`.

## Goal

Finish the remaining high-value hand-authored single-threaded tests before seed-gated work and before fuzz/property tooling becomes the focus.

Do not broaden scope into:

- API implementation
- frontend work
- parallelism/Rayon
- snapshot/observer/product features
- revisiting the final seed organism

## Remaining Testing Priorities

Focus on the next useful manual gaps from the current plan and testing guide:

1. One more deterministic nonzero-rate accounting case that includes decay, not just forced arrivals.
2. One or two meaningful concurrent-stress scenarios still worth hand-authoring:
   - many exclusive actions targeting one cell
   - a denser packet-collision lattice, if it reveals something new beyond the current ring case
3. Any remaining explicit control-flow corners that are still high-signal and cheap:
   - only if they are not already effectively covered
   - prefer tests that clarify invariants, not redundant opcode enumeration

If you finish those cleanly, the next tier is:

- stronger ecology-style multi-tick scenarios
- more stochastic-accounting cases with fixed seeds

Do not start:

- seed smoke tests unless the seed has been finalized externally
- proptest / cargo-fuzz infrastructure unless explicitly needed to finish this handoff

## Fixed Constraints

Treat these as settled:

- the work is still single-threaded
- pass boundaries should remain explicit
- test helpers already exist; reuse them instead of building new harness infrastructure
- the engine is a library first; do not distort it for tests
- failure explanations matter as much as the eventual fix

## Failure Analysis Requirement

This is now a repo-level instruction and must be preserved in behavior:

- when a test fails, explain the failure explicitly before or alongside the fix
- reason from first principles when possible
- describe the execution steps, invariants, or data-flow that produce the result
- do not just say “the assertion was wrong” or “ordering changed” without explaining why

## Working Style

Prefer:

- small, coherent test batches
- using existing helpers and public entry points
- tests that assert spec invariants, not internal implementation accidents

Avoid:

- speculative refactors
- broad code churn just to make a test easier to write
- duplicating coverage that already exists unless the existing test is too weak or ambiguous

## Task Boundary

You may edit:

- `rust/tests/**`
- `rust/src/**` only if a test exposes a real engine bug that requires a narrow fix
- `.agents/plans/codex-rust-impl.md` if the completed tranche changes the execution state materially

Do not edit:

- spec docs
- API/frontend docs
- legacy code

## Verification

Before stopping, run from `rust/`:

- `cargo fmt`
- `cargo clippy --all-targets --all-features`
- `cargo test`

Done means:

- the new tests are green
- any engine fix is narrow and justified by a failing test
- the plan is updated if the tranche materially changed what remains
- failure explanations were explicit and first-principles if any test failed during the work
