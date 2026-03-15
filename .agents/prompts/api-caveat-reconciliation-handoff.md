# API Caveat Reconciliation Handoff Prompt

Reconcile the remaining post-merge API caveats for the Rust backend using the decisions already settled below.

This is a targeted alignment task, not a fresh design task. Do not re-open the design questions that this prompt fixes as constraints.

## Goal

Bring the docs and the small remaining API-facing observation code into alignment with the main simulation spec and the agreed backend behavior.

Expected outcome:

- API-facing `dir` values match the master `Dir` encoding from `docs/SPEC.md`
- API and frontend docs describe the current implemented behavior accurately
- the known `400` vs `404` out-of-bounds inconsistency is resolved in docs
- snapshot support is described as deferred/unimplemented for the current backend state
- the current aggregate `births` metric remains documented correctly
- code and tests still pass after the small observation-surface change

## Settled Decisions

These decisions are fixed for this handoff:

1. Direction encoding
   - Align the API inspection `dir` field to the master spec now.
   - Use the same numeric encoding as `Dir` in `docs/SPEC.md`: `0=right`, `1=up`, `2=left`, `3=down`.
   - Remove the local remapping in `rust/src/observe.rs`.
   - No API version bump is required for this repo-internal change because nothing external is known to consume the current `API-SPEC.md` contract yet.

2. Lifecycle state
   - Keep `created` / `running` / `paused` in the web controller layer, not in the engine core.
   - Do not move lifecycle state into the engine in this handoff.

3. Seed-program bootstrap
   - Seed program placement should remain outside `SimConfig`.
   - Do not move `seed_programs` into the engine config surface in this handoff.
   - The existing behavior where default `Dir` and `ID` are randomized is correct and should be documented as such.

4. Birth metrics
   - Keep `births` as the aggregate count of programs that became live this tick via either `boot` or spontaneous spawn.
   - Do not rename `births`.
   - Split metrics such as `boot_births` and `spawn_births` are a later additive follow-up, not part of this handoff.

5. Total energy
   - Keep `total_energy` defined as the sum of cell-local free energy plus background radiation only.
   - Do not add in-flight packet energy to `total_energy` in this handoff.
   - Any packet-energy metric should be additive later, not a redefinition of the existing field.

6. Snapshots
   - Snapshot routes remain deferred.
   - Do not invent a snapshot serialization or storage boundary in this handoff.
   - Update docs so the current backend state is described honestly.

7. Out-of-bounds inspection status
   - Normalize on `400`, not `404`, for invalid cell coordinates / indices.
   - Treat this as invalid request input, not a missing resource.

## Source Hierarchy

This prompt encodes the settled decisions and overrides older prose where necessary.

Then use these sources in order:

1. `docs/SPEC.md`
   - authoritative for `Dir` numeric encoding and default register semantics
   - especially `### CPU and Registers`
2. `docs/SPEC-COMPANION.md`
   - implementation-facing clarifications when needed
3. `docs/API-SPEC.md`
   - external contract to update so it matches the settled decisions above
4. `docs/FRONTEND-SPEC.md`
   - consumer expectations that must match the updated API contract
5. `docs/BACKEND-GUIDELINES.md`
   - advisory only; use for layering boundaries, not to override the master spec
6. current Rust code under `rust/`
   - especially `rust/src/observe.rs` and affected tests

If a source conflicts with the settled decisions in this prompt, update that source rather than preserving the conflict.

## In Scope

- update `docs/API-SPEC.md`
- update `docs/FRONTEND-SPEC.md`
- update `rust/src/observe.rs`
- update affected tests under `rust/` as needed
- optionally make a very small wording update in `docs/BACKEND-GUIDELINES.md` only if necessary to prevent a direct contradiction

## Explicit Non-Goals

- no snapshot route implementation
- no engine snapshot type or storage work
- no controller lifecycle refactor
- no bootstrap-module extraction
- no new `packet_energy` metric
- no new `boot_births` / `spawn_births` fields yet
- no broad web-layer redesign

## Concrete Work

1. Align API direction encoding with the master spec
   - In `docs/API-SPEC.md §12`, change the `dir` encoding description to match `docs/SPEC.md`.
   - In `rust/src/observe.rs`, remove the API-only direction remap and expose the engine `Dir` numeric value directly.
   - Update or add tests so the inspection surface now asserts `0=right`, `1=up`, `2=left`, `3=down`.

2. Bring docs in line with current implemented behavior
   - In `docs/API-SPEC.md`, keep the randomized default `Dir` / `ID` wording for seeded programs.
   - In `docs/API-SPEC.md`, keep `births` documented as aggregate live creation via boot or spontaneous spawn.
   - In `docs/FRONTEND-SPEC.md`, update any wording that assumes a different meaning for `births` or leaves `dir` encoding implicit where a clarification would help.

3. Resolve the out-of-bounds inconsistency
   - In `docs/API-SPEC.md`, make inspection out-of-bounds behavior consistently `400`.
   - Check for any corresponding frontend wording that should match.

4. Clarify snapshot status without implementing snapshots
   - Update `docs/API-SPEC.md` and `docs/FRONTEND-SPEC.md` so snapshot operations are clearly described as deferred/unimplemented for the current backend state.
   - Do not remove the target-design concept entirely; just make current backend status explicit.

5. Preserve currently agreed metrics boundaries
   - Do not change `total_energy` behavior in code.
   - If you touch docs around metrics, keep the wording consistent with current implementation and note packet-energy expansion only as future work.

## Follow-On Work After This Handoff

These are intentionally not part of the execution scope here, but keep them visible:

1. extracted bootstrap module for seed-program application
2. controller lifecycle state-machine cleanup
3. engine snapshot boundary and web-layer snapshot store boundary
4. additive packet-energy metric
5. additive `boot_births` / `spawn_births` metrics alongside aggregate `births`

## Files In Scope

Primary:

- `docs/API-SPEC.md`
- `docs/FRONTEND-SPEC.md`
- `rust/src/observe.rs`
- `rust/tests/web_api.rs`
- `rust/tests/tick_driver.rs`

Secondary, only if needed:

- `docs/BACKEND-GUIDELINES.md`
- `rust/src/web/types.rs`

Do not broaden scope beyond these files unless a small supporting test/helper change is clearly required.

## Verification

From `rust/`, run:

- `cargo fmt --check`
- `cargo clippy --all-targets --all-features`
- `cargo test`
- `cargo test --all-features`

Then verify docs/search results:

- `rg -n "0 = North|1 = East|2 = South|3 = West" docs/API-SPEC.md docs/FRONTEND-SPEC.md rust/src/observe.rs rust/tests`
- `rg -n "Programs booted this tick" docs/API-SPEC.md docs/FRONTEND-SPEC.md`
- `rg -n "snapshot" docs/API-SPEC.md docs/FRONTEND-SPEC.md`

## Output Shape

Produce:

1. review findings first, if any
2. brief summary of the applied alignment changes
3. verification results
4. any explicitly deferred follow-up items still left for later work

## Done Means

- API `dir` encoding matches `docs/SPEC.md`
- no local API direction remap remains in `rust/src/observe.rs`
- API and frontend docs no longer contradict the agreed behavior on `dir`, `births`, snapshots, or out-of-bounds status
- Rust verification passes
- no deferred features were implemented speculatively
