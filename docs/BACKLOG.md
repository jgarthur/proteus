# Proteus Backlog

Durable parking lot for explicitly deferred follow-up work that should survive beyond a single handoff prompt or conversation.

This file is not authoritative for simulator semantics. `SPEC.md` and `SPEC-COMPANION.md` remain the source of truth for behavior. Backlog items here capture agreed future work, cleanup, and additive extensions.

## Current Items

### API Caveat Reconciliation Follow-Ons

Carried forward from the completed API caveat reconciliation handoff on 2026-03-15.

1. Extract seed-program bootstrap into a dedicated module.
Context: keep seed placement outside `SimConfig`; this is code-organization follow-up, not a semantics change.
References: `docs/API-SPEC.md`, `rust/src/web/controller.rs`

2. Clean up the web-controller lifecycle state machine.
Context: `created` / `running` / `paused` remain a web-layer concern; this follow-up is about controller structure, not moving lifecycle into the engine core.
References: `docs/API-SPEC.md`, `rust/src/web/types.rs`, `rust/src/web/controller.rs`

3. Introduce an engine snapshot boundary and a web-layer snapshot store boundary.
Context: snapshot routes remain deferred until the backend has a settled, non-speculative snapshot boundary.
References: `docs/API-SPEC.md` §13, `docs/FRONTEND-SPEC.md` §8, `rust/src/web/mod.rs`

4. Add a packet-energy metric.
Context: additive only. Do not redefine existing `total_energy`, which remains cell-local free energy plus background radiation only.
References: `docs/API-SPEC.md` §10, `docs/API-SPEC.md` §16, `rust/src/observe.rs`

5. Add `boot_births` and `spawn_births` alongside aggregate `births`.
Context: additive only. Keep the existing aggregate `births` metric.
References: `docs/API-SPEC.md` §10, `docs/FRONTEND-SPEC.md` §6, `rust/src/observe.rs`, `rust/tests/tick_driver.rs`
