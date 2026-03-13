## Recent Changes

### 2026-03-13

- Added optional Cython helper acceleration for hot hash and loop-scan paths, with golden-archive parity tests to keep the Python reference engine authoritative.
- Added ecology analysis and sweep tooling, including milestone snapshots, anti-scavenger scoring, and a live time-series monitor for long runs.
- Added configurable inert-offspring lifecycle controls in the engine:
  - `inert_maintenance_scale`
  - `inert_auto_boot_ticks`
- Current defaults set inert offspring to pay no maintenance while inert and to auto-boot after 10 ticks with no incoming write.
- Added inspector support for inert wait state and archive persistence for the inert-write timer.
- Dense 50k evaluations showed that denser worlds support busier, richer ecologies than the original sparse regime, but stable replicator persistence still does not emerge.
- The inert no-maintenance plus auto-boot experiment improved cleanup of inert husks but did not improve long-run replicator persistence in the key dense worlds.
- Next likely investigation areas:
  - event-level instrumentation for replication attempts, manual boots, and auto-boots
  - shorter abandonment timeout or auto-dissolve instead of auto-boot
  - environmental non-stationarity to create stronger selection pressure for reproduction
