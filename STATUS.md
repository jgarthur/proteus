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
- [x] CONFIG-RATES: Rework ambient config fields to use Poisson arrival rates and steady-state background initialization while keeping decay fields as binomial per-quantum probabilities
- [x] MOVE-ELIGIBILITY: Make tick-start eligibility follow a moving program
- [x] METRIC-CUMULATIVE-EVENTS: Add per-simulation metrics epochs and cumulative birth, death, and mutation totals so sampled observation remains lossless
- [x] HEADLESS-RUNNER: Implement deterministic single-run and fixed-batch execution from `docs/RUNNER-SPEC.md`
- [x] SEED-ENVIRONMENT: Support seed-neighborhood resource preload
- [x] SEED-BOOTSTRAP: Extract seed-program bootstrap module

## Next

- [ ] Design 1–2 minimal self-replicating seed programs
- [ ] CONFIG-DYADIC-K: Specify every dyadic probability config field as an integer exponent (field value k, probability 2^-k; explicit null = never), replacing the f64 fields, `dyadic_exponent` derivation, and the NotDyadicProbability validation class; includes API-SPEC bump, SPEC.md parameter-domain edits, fixture/manifest/frontend migration, and runner input-digest churn. Decide absent-vs-null serde semantics explicitly. Batch with MUTATION-ANY-QUANTUM to pay the migration churn once. See `docs/BACKLOG.md`.
- [ ] MUTATION-ANY-QUANTUM: Change background-stressed mutation from `p = min(x / 2^k, 1)` to "any of x Bernoulli(2^-k) triggers" — i.e. `binomial_pow2(x, k) > 0` — removing the saturation cliff at x = 2^k; delete the then-dead `bernoulli_ratio_pow2`. Draw streams change (version bump + sampled-literal migration). Open spec question to resolve in-task: mutate once when any quantum fires (default), or apply count mutations (escalation, off by default). Batch with CONFIG-DYADIC-K. See `docs/BACKLOG.md`.
- [ ] MUTATION-CAUSE-SPLIT: Split the mutation metric into baseline and background-stressed counters. See `docs/analysis/2026-08-13_mutation-load-and-emergence-milestones.md`.
- [x] EMERGENCE-SWEEP: All three axes swept (62 runs). M2 onset collapses onto mutations × occupancy ≈ 10^4, not `mutation_base_log2` alone; `p_spawn` buys attempts, `r_mass` buys build size; spec default `r_mass = 0.05` yields dead soup. See `docs/analysis/2026-08-13_mutation-load-and-emergence-milestones.md`.
- [x] ATTRACTOR-CONVERGENCE: Confirmed at 200k ticks — mean genome size converges to 40.5 and mutations-per-lifetime to ≈1.8 by tick 130k; population density does not settle in that span. See `docs/analysis/2026-08-13_mutation-load-and-emergence-milestones.md`.
- [ ] SEED1-TRACE: Verify the seed-1 genome predictions (offspring receive the literal byte `39`; `boot` fires before the copy loop) by inspecting a live offspring at boot time. See `docs/analysis/2026-08-13_mutation-load-and-emergence-milestones.md`.
- [x] EMERGENCE-CENSUS: Program-size histogram, opcode census, size-1 population, stack-depth histogram, and per-program parent/birth-tick lineage, delivered as an optional point-in-time `census` block in `MetricsSnapshot` (always in runner rows, `?census=1` on the web API). See `docs/BACKLOG.md`.
- [x] CENSUS-LOG-BUCKETS: `BucketHistogram` gained a `linear`/`log2` scale; `stack_depths` now uses 33 log2 buckets spanning every `u32` with no overflow, because linear bucketing could not describe it (measured p50 759, max 32767 on a grown 256x256 run). See `docs/BACKLOG.md`.
- [ ] LINEAGE-DUMP: Emit a per-program `lineage.jsonl` artifact at coarse cadence for offline lineage reconstruction; needs a runner schema bump. See `docs/BACKLOG.md`.
- [ ] INSPECTOR-LINEAGE: Surface the new `uid`/`parent_uid`/`birth_tick`/`generation`/`origin`/`created_tick` inspection fields in the frontend inspector. See `docs/BACKLOG.md`.
- [ ] CENSUS-STREAM: Opt-in WebSocket census subscription with its own cadence. See `docs/BACKLOG.md`.
- [ ] CONTROLLER-LIFECYCLE: Clean up web-controller lifecycle state machine
- [ ] SNAPSHOT-BOUNDARY: Engine snapshot boundary and web-layer snapshot store
- [x] METRIC-PACKET-ENERGY: Add packet-energy metric
- [x] METRIC-BIRTH-TYPES: Add boot_births and spawn_births metrics
- [ ] SPEED-CONTROL: Replace the frontend-side target-TPS stepping shim with real backend speed control and re-align the frontend with the spec
- [ ] FRONTEND-DEFAULTS: Reconcile local testing defaults and seed-program bootstrap hacks with the spec-backed frontend defaults
- [ ] CONFIG-SCENARIO: Share one engine-owned scenario type between the web layer and the runner
- [ ] FRONTEND-CONFIG-TOOLS: Reconcile local config save/load debugging helpers with the frontend spec
- [ ] COORDINATE-CONVENTIONS: Standardize frontend coordinates as 0-indexed and display them in `(y, x)` order
- [ ] NO-SIM-STATUS: Replace the frontend startup `404 /v1/sim` probe with a cleaner no-simulation status path
- [ ] FRONTEND-STATIC-CHECKS: Add lightweight frontend static checks for stale vars and similar mistakes
- [ ] FRONTEND-ARCH-CLEANUP
- [ ] INSPECTOR-TRACK-PROGRAM: Let the inspector follow a selected program as it moves
- [ ] SEED-PROGRAM-QOL: Add seed-program library/import-export/disassembly tooling
- [ ] TRANSPORT-CONTROLS: Promote play-pause-speed controls and add hotkeys
- [x] RAYON-BASELINE: Keep the direct serial iteration paths; a one-thread Rayon pool remained 3-15% slower across the final fixture sweep. See `docs/analysis/2026-08-12_rayon-optimization-results.md`.
- [x] RAYON-PERFORMANCE: Remove Pass 2 grid-clone overhead, fuse compatible Rayon Pass 3 traversals, add dense-growth benchmarking, and strengthen parity/instruction audits. See `docs/analysis/2026-08-12_rayon-optimization-results.md`.
- [x] DYADIC-SAMPLERS: Implement exact power-of-two probability samplers and Poisson inversion per `docs/analysis/2026-08-13_dyadic-sampler-plan.md` (includes the approved dyadic spec adjustment for decay probabilities)
- [ ] RAYON-HOTSPOTS: Remaining performance tiers after the sampler work (SoA hot-field split, inline program storage behind data-justified caps, Pass 1 dispatch); roadmap in `docs/analysis/2026-08-13_performance-roadmap.md`
- [x] RAYON-PARITY: Automate the serial-vs-Rayon golden check (`rust/scripts/check-rayon-parity.sh`, wired into CI)
