## Recent Changes

### 2026-03-13

- Added optional Cython helper acceleration for hot hash and loop-scan paths, with golden-archive parity tests to keep the Python reference engine authoritative.
- Added ecology analysis and sweep tooling, including milestone snapshots, anti-scavenger scoring, and a live time-series monitor for long runs.
- Added configurable inert-offspring lifecycle controls in the engine:
-  `inert_grace_ticks`
- Current defaults give inert offspring a 10-tick grace window with no maintenance while they are under active construction. If construction is abandoned, they remain inert and decay through ordinary maintenance rather than auto-booting.
- Added inspector support for inert wait state and archive persistence for the inert-write timer.
- Dense 50k evaluations showed that denser worlds support busier, richer ecologies than the original sparse regime, but stable replicator persistence still does not emerge.
- The inert no-maintenance plus auto-boot experiment improved cleanup of inert husks but did not improve long-run replicator persistence in the key dense worlds, and likely created accidental scavengers from abandoned fragments.
- Replaced that experiment with grace-period maintenance: active construction is protected, but abandoned fragments now die via the normal maintenance path.
- Next likely investigation areas:
  - event-level instrumentation for replication attempts, manual boots, abandonment, and pre-boot death
  - better understanding of whether abandoned fragments still dominate as passive debris
  - environmental non-stationarity to create stronger selection pressure for reproduction
