# Proteus Status

Living document. Mark items done (`[x]`) as they are completed. Backlog items reference `docs/BACKLOG.md` by name for full context.

## Done

- [x] v0.2.0 spec (SPEC.md + SPEC-COMPANION.md)
- [x] Single-threaded engine (passes 0–3)
- [x] Observation layer and binary/frame encoding (`observe.rs`)
- [x] Feature-gated web API (REST + WebSocket)
- [x] Server binary (`proteus-server`)
- [x] Integration test suite (tick driver + web API)
- [x] API caveat reconciliation (docs aligned with backend)
- [x] Frontend spec (`FRONTEND-SPEC.md`)
- [x] Build the frontend (implement `FRONTEND-SPEC.md`)
- [x] Boot the simulator end-to-end and observe it (via `/debug/smoke`)

## Next

- [ ] Design 1–2 minimal self-replicating seed programs
- [ ] SEED-ENVIRONMENT: Support seed-neighborhood resource preload
- [ ] SEED-BOOTSTRAP: Extract seed-program bootstrap module
- [ ] CONTROLLER-LIFECYCLE: Clean up web-controller lifecycle state machine
- [ ] SNAPSHOT-BOUNDARY: Engine snapshot boundary and web-layer snapshot store
- [ ] METRIC-PACKET-ENERGY: Add packet-energy metric
- [ ] METRIC-BIRTH-TYPES: Add boot_births and spawn_births metrics
- [ ] SPEED-CONTROL: Replace the frontend-side target-TPS stepping shim with real backend speed control and re-align the frontend with the spec
- [ ] FRONTEND-DEFAULTS: Reconcile local testing defaults and seed-program bootstrap hacks with the spec-backed frontend defaults
- [ ] CONFIG-RATES: Rework backend ambient/decay config fields from probabilities to true rates
- [ ] FRONTEND-CONFIG-TOOLS: Reconcile local config save/load debugging helpers with the frontend spec
- [ ] COORDINATE-CONVENTIONS: Standardize frontend coordinates as 0-indexed and display them in `(y, x)` order
- [ ] NO-SIM-STATUS: Replace the frontend startup `404 /v1/sim` probe with a cleaner no-simulation status path
- [ ] FRONTEND-STATIC-CHECKS: Add lightweight frontend static checks for stale vars and similar mistakes
- [ ] FRONTEND-ARCH-CLEANUP
- [ ] INSPECTOR-TRACK-PROGRAM: Let the inspector follow a selected program as it moves
- [ ] SEED-PROGRAM-QOL: Add seed-program library/import-export/disassembly tooling
- [ ] TRANSPORT-CONTROLS: Promote play-pause-speed controls and add hotkeys
