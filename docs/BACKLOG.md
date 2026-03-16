# Proteus Backlog

Durable parking lot for explicitly deferred follow-up work that should survive beyond a single handoff prompt or conversation.

This file is not authoritative for simulator semantics. `SPEC.md` and `SPEC-COMPANION.md` remain the source of truth for behavior. Backlog items here capture agreed future work, cleanup, and additive extensions.

Items are referenced by name from `STATUS.md` at the repo root. Use the exact item name when marking work done in either file.

## Items

### SEED-BOOTSTRAP: Extract seed-program bootstrap module

Context: keep seed placement outside `SimConfig`; this is code-organization follow-up, not a semantics change.
References: `docs/API-SPEC.md`, `rust/src/web/controller.rs`

### SEED-ENVIRONMENT: Support seed-neighborhood resource preload

Problem: the current API config can seed resources only on occupied program cells, so it cannot represent the spec seed's recommended resource-rich neighboring empty cells.

### CONTROLLER-LIFECYCLE: Clean up web-controller lifecycle state machine

Context: `created` / `running` / `paused` remain a web-layer concern; this follow-up is about controller structure, not moving lifecycle into the engine core.
References: `docs/API-SPEC.md`, `rust/src/web/types.rs`, `rust/src/web/controller.rs`

### SNAPSHOT-BOUNDARY: Engine snapshot boundary and web-layer snapshot store

Context: snapshot routes remain deferred until the backend has a settled, non-speculative snapshot boundary.
References: `docs/API-SPEC.md` §13, `docs/FRONTEND-SPEC.md` §8, `rust/src/web/mod.rs`

### METRIC-PACKET-ENERGY: Add packet-energy metric

Context: additive only. Do not redefine existing `total_energy`, which remains cell-local free energy plus background radiation only.
References: `docs/API-SPEC.md` §10, `docs/API-SPEC.md` §16, `rust/src/observe.rs`

### METRIC-BIRTH-TYPES: Add boot_births and spawn_births metrics

Context: additive only. Keep the existing aggregate `births` metric.
References: `docs/API-SPEC.md` §10, `docs/FRONTEND-SPEC.md` §6, `rust/src/observe.rs`, `rust/tests/tick_driver.rs`

### CONFIG-RATES: Rework backend ambient/decay config fields from probabilities to true rates

Context: the current backend validates `r_energy`, `r_mass`, `d_energy`, and `d_mass` as probabilities in `[0, 1]`, but the intended tuning model treats them as rates. Reconcile the engine, API docs, and frontend defaults around a single rate-based semantics.
References: `docs/SPEC.md`, `docs/API-SPEC.md` §8, `rust/src/config.rs`, `rust/src/pass3.rs`, `frontend/src/constants.ts`

### COORDINATE-CONVENTIONS: Standardize frontend coordinates as 0-indexed and display them in `(y, x)` order

Context: current frontend UI and config tooling still lean on `x, y` ordering from the API surface. Reconcile the display language, validation messaging, and editor fields so the UI is consistently 0-indexed and uses `(y, x)` ordering. This is intentionally marked tricky because it cuts across inspector display, seed-program editing, hit-testing labels, and API request mapping.
References: `docs/FRONTEND-SPEC.md`, `docs/API-SPEC.md` §12, `frontend/src/components/controls/ConfigEditor.tsx`, `frontend/src/components/inspector/InspectorTab.tsx`, `frontend/src/components/GridCanvas.tsx`

### NO-SIM-STATUS: Replace the frontend startup `404 /v1/sim` probe with a cleaner no-simulation status path

Context: the frontend currently probes `GET /v1/sim` on load and treats `404` as the expected "no sim exists" case. This is functionally fine, but it produces a noisy red network entry in browser devtools. Low priority cleanup: add a cleaner backend/API path for empty-state status or otherwise remove the expected-404 startup probe.
References: `docs/API-SPEC.md` §7, `rust/src/web/mod.rs`, `frontend/src/context/SimContext.tsx`, `frontend/src/lib/api.ts`
