# API Implementation Handoff Prompt

Implement the first real external API layer for Proteus in Rust.

## Deliverable

Build the backend API/server inside `rust/` around the existing simulator core.

Target outcome:

- a compilable web/API surface behind a Cargo feature such as `web`
- REST endpoints for the stable, unblocked control/inspection surface
- a WebSocket endpoint for frame and metrics streaming
- tests for handler behavior and wire-format encoding

This is an implementation task, not a spec-writing task.

## Source Hierarchy

Use these sources in this order:

1. `docs/API-SPEC.md`
   - primary contract for external behavior
   - especially sections 3-15
2. `docs/FRONTEND-SPEC.md`
   - consumer expectations and practical constraints
   - especially sections 3-7 and 12-13
3. `docs/BACKEND-GUIDELINES.md`
   - implementation guidance
   - especially sections 344-375
4. `docs/SPEC.md` and `docs/SPEC-COMPANION.md`
   - authoritative only for the meaning of simulation state and values exposed through the API
   - do not reinterpret engine semantics from the API side
5. existing Rust code in `rust/src/`
   - this is the actual engine surface you are wrapping

If sources conflict, `API-SPEC.md` wins for the external contract. Do not silently "fix" the docs in this task.

## Current Engine State

The Rust engine already exists as a single-threaded library in `rust/src/`:

- `Simulation::new(...)`
- `Simulation::run_tick()`
- pass-structured core logic in `pass1`, `pass2`, `pass3`
- deterministic RNG/config/grid/model infrastructure
- integration tests covering engine semantics

Treat the engine as the thing to wrap, not to redesign.

## Goal

Create the API layer that can run in parallel with ongoing engine-testing work.

That means:

- add transport, serialization, and controller code
- add read-only projection helpers if needed
- avoid changing pass semantics or low-level engine behavior unless absolutely required

## Scope

Build now:

- API versioning header and `/v1/...` routing
- single-simulation lifecycle
- config create/read
- control operations: start, pause, resume, step, reset, destroy
- simulation status response
- metrics schema and production
- binary grid-frame encoding for WebSocket streaming
- cell inspection with exact values and disassembly
- consistent JSON error model
- permissive localhost-friendly CORS behavior

Build only if it stays narrow and non-speculative:

- snapshot save/load/list/delete

If snapshot support would require inventing a serialization format or making broad engine changes, stop short of full snapshot implementation. Leave a clear blocker note in code/tests instead of guessing.

Do not build now:

- pass-level debug APIs
- runtime config mutation
- lineage/mutation tracing
- replay/deterministic playback API
- multi-session or auth
- compression or binary metrics
- frontend code

## Fixed Constraints

These are already decided. Treat them as fixed:

- one simulation per server process
- REST for control/inspection, WebSocket for streaming
- config is immutable after creation
- latest-available frame delivery is acceptable; do not queue unbounded frame backlogs
- subscriptions are stateless across reconnect unless the spec explicitly says otherwise
- `legacy/rust/` is irrelevant
- keep the engine core usable as a library

## Architectural Direction

Prefer a shape consistent with `docs/BACKEND-GUIDELINES.md`:

- keep the simulator core separate from the web layer
- gate web/API dependencies behind a Cargo feature such as `web`
- expose a small server/bootstrap entrypoint instead of entangling transport code with pass logic
- use a single owner/controller for mutable `Simulation` state
- expose rendering/metrics snapshots through lightweight projection types rather than the raw internal structs

Reasonable structure examples:

- `rust/src/web/`
- `rust/src/api/`
- `rust/src/observe/`
- `rust/src/bin/...` for a server entrypoint

You do not need to match those names exactly, but keep the separation obvious.

## Boundaries With Ongoing Engine Work

Assume engine semantics may continue to gain tests while you work. To avoid collisions:

- do not refactor pass modules just to suit the API layer
- keep API-facing additions additive where possible
- if you need new read-only helpers from the engine, make them small and explicit
- if an API need seems to require semantic engine changes, note it as a blocker/open question instead of quietly changing behavior

## Important Ambiguities

The docs still contain open questions. Do not convert them into externally claimed guarantees unless the spec already settles them.

If implementation forces a choice:

- prefer the simplest behavior compatible with the current frontend spec
- keep the choice local
- note it in code/tests as provisional
- do not edit spec docs in this task

Examples:

- whether a frame subscription immediately pushes the current frame
- whether snapshot persistence is file-backed or otherwise
- whether in-flight packet energy is included in total-energy metrics

## Required Output Shape

Aim to land:

1. web/API module(s) under `rust/src/`
2. any needed feature-gated dependencies in `rust/Cargo.toml`
3. a small server/bootstrap surface
4. projection/serialization types for:
   - sim status
   - metrics
   - grid frame header/cell view
   - cell inspection
   - errors
5. tests covering:
   - happy-path REST control flow
   - 404/409/400 class errors
   - WebSocket subscribe/encode behavior
   - binary frame layout sanity

## Task Boundary

Only edit:

- `rust/Cargo.toml`
- `rust/src/**`
- `rust/tests/**`
- `rust/README.md` if the crate surface changes materially

Do not edit:

- `docs/API-SPEC.md`
- `docs/FRONTEND-SPEC.md`
- `docs/SPEC.md`
- `docs/SPEC-COMPANION.md`
- frontend code outside `rust/`

If you discover a real spec gap, leave it as a clearly named open question in code/tests or your final notes.

## Verification

Before stopping, run from `rust/`:

- `cargo fmt`
- `cargo clippy --all-targets --all-features`
- `cargo test`

Done means:

- the API/server code compiles behind the chosen feature flag
- the implemented surface matches the stable, unblocked parts of `docs/API-SPEC.md`
- tests cover status/error behavior and wire-format basics
- no engine semantics were changed just to make the API convenient
- any deferred snapshot or open-question areas are called out explicitly rather than guessed
