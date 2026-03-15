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

## Next

- [ ] Boot the simulator end-to-end and observe it (even via `curl` / WebSocket script)
- [ ] Design 1–2 minimal self-replicating seed programs
- [ ] SEED-BOOTSTRAP: Extract seed-program bootstrap module
- [ ] CONTROLLER-LIFECYCLE: Clean up web-controller lifecycle state machine
- [ ] Build the frontend (implement `FRONTEND-SPEC.md`)
- [ ] SNAPSHOT-BOUNDARY: Engine snapshot boundary and web-layer snapshot store
- [ ] METRIC-PACKET-ENERGY: Add packet-energy metric
- [ ] METRIC-BIRTH-TYPES: Add boot_births and spawn_births metrics
