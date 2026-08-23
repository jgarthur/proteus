# Proteus Backlog

Durable parking lot for explicitly deferred follow-up work that should survive beyond a single handoff prompt or conversation.

This file is not authoritative for simulator semantics. `SPEC.md` and `SPEC-COMPANION.md` remain the source of truth for behavior. Backlog items here capture agreed future work, cleanup, and additive extensions.

Items are referenced by name from `STATUS.md` at the repo root. Use the exact item name when marking work done in either file.

## Items

### HEADLESS-RUNNER: Implement deterministic single-run and fixed-batch execution

Context: add `proteus-run` for one fully specified engine-direct run and `proteus-batch` for bounded subprocess supervision. Keep parameter search, scoring, and emergence optimization in an external outer loop. Share bootstrap and environmental preload semantics with the web controller rather than duplicating them.
References: `docs/RUNNER-SPEC.md`, `SEED-BOOTSTRAP`, `SEED-ENVIRONMENT`, `SNAPSHOT-BOUNDARY`, `rust/src/simulation.rs`, `rust/src/web/controller.rs`

### RAYON-BASELINE: Decide whether Rayon should replace the separate serial iteration paths

Decision (2026-08-12): keep the feature-gated direct serial paths. A one-thread Rayon pool was 3-15% slower than the direct serial build across the six final benchmark fixtures, while four threads provided substantial gains. This item is closed; retain the context here so the duplication remains an explicit measured choice.
References: `rust/src/pass1.rs`, `rust/src/pass3.rs`, `rust/src/simulation.rs`, `.github/workflows/rust.yml`, `docs/analysis/2026-08-12_rayon-optimization-results.md`

### DYADIC-SAMPLERS: Exact power-of-two probability samplers and Poisson inversion

Decision (2026-08-21): implemented exact `2^-k` Bernoulli/binomial samplers, Poisson inversion with per-loop precomputation and a large-rate fallback, dyadic config validation, and exponent-1 `powf` fast paths. The approved decay defaults are `2^-7`; mutation exponents above 63 are configuration errors. Serial and Rayon tests plus 1000-tick parity at 1/2/4/8 threads pass. Timed benchmarks were skipped because the shared machine was not quiet; rerun them before drawing performance conclusions.
References: `docs/analysis/2026-08-13_dyadic-sampler-plan.md`, `docs/analysis/2026-08-21_dyadic-sampler-results.md`, `rust/src/random.rs`, `rust/src/pass3.rs`, `rust/src/config.rs`

### EMERGENCE-CENSUS: Program-size histogram, opcode census, and per-program lineage

Delivered (2026-08-21): `census` is an optional point-in-time block inside `MetricsSnapshot` (size histograms, opcode census, size-1 population, aggregated lineage, stack depths), always present in runner rows and opt-in over the web API via `GET /v1/sim/metrics?census=1`. `Program` carries a 24-byte observation-only `Lineage` block.

Decisions worth keeping:
- **Uids are derived from the creation site**, `1 + ((tick * cell_count + cell_index) * 4 + origin)`, never allocated from a counter. Spawn births happen inside the Rayon per-cell closure, so any shared counter would make uids scheduling-dependent and break serial-vs-Rayon parity. The 4-way origin tag exists because one cell can be created twice in a tick (an `appendAdj` body destroyed by maintenance, then a spawn into the same cell). Uids are epoch-scoped: not comparable across simulations, resets, or grid sizes.
- **Seed programs and spontaneous spawns are lineage roots** (`parent` NONE, `generation` 0), matching SPEC.md's "primordial bootstrap - no parent required" and API-SPEC's rule that bootstrap programs are not births. That makes "did a spawn-rooted lineage reach generation >= 1?" (milestone M5) directly observable.
- **Parent is captured when the create commit is queued**, not when it is applied, because Pass 2 applies every move before every create.
- **Computed on sample, never incrementally**, to keep the tick path free of new branches, stores, and draws.
- **Schema placement**: API metrics schema 0.2.4 -> 0.2.5. That additive metrics change deliberately left runner schema 0.1.0 alone because it did not alter canonical input bytes. CONFIG-DYADIC-K later overrides that decision with runner schema 0.2.0: its field renames and null encoding change canonicalization and already invalidate every pre-v0.4.0 manifest and resume record.
- **Bucket widths were re-measured, not assumed.** Generation and stack-depth histograms were widened from the specified 32/64 to 512/512 after a grown `web-256x256` run showed 97.8% and 77.8% of the population in the overflow bins. Stack depth still overflows: on a real ecology it is p50 759 / p90 8315 / max 32767, spanning the entire stack cap, so no affordable linear width empties that bin. See CENSUS-LOG-BUCKETS.

Deferred follow-ups: `LINEAGE-DUMP`, `INSPECTOR-LINEAGE`, `CENSUS-STREAM`, `CENSUS-LOG-BUCKETS`.
References: `docs/analysis/2026-08-13_mutation-load-and-emergence-milestones.md`, `docs/API-SPEC.md`, `docs/RUNNER-SPEC.md`, `rust/src/observe.rs`, `rust/src/model.rs`

### LINEAGE-DUMP: Per-program lineage artifact for offline reconstruction

Context: the census aggregates lineage; reconstructing actual family trees offline needs per-program rows (`lineage.jsonl`) at a coarse cadence. Needs a runner schema bump, which is exactly what EMERGENCE-CENSUS avoided, so it is its own item.
References: `docs/RUNNER-SPEC.md`, `rust/src/runner/single.rs`

### INSPECTOR-LINEAGE: Surface the lineage fields in the frontend inspector

Delivered (2026-08-22): the inspector shows a Lineage card (origin, generation, birth tick, created tick, uid, parent uid) between the Program and Disassembly cards. `CellProgram` carries the six fields with the API-SPEC §12 nullability, and `frontend/src/lib/lineage.ts` decodes a uid back into its creation site, which makes the parent uid a click target that selects the cell the parent was created in. The decode declines uids past `Number.MAX_SAFE_INTEGER`, where JSON has already rounded the `u64`.

Context that motivated it: `GET /v1/sim/cell/:index` returns `uid`, `parent_uid`, `birth_tick`, `generation`, `origin`, and `created_tick`, and the inspector did not show them.
References: `docs/FRONTEND-SPEC.md`, `frontend/src/types.ts`, `frontend/src/lib/lineage.ts`, `frontend/src/components/inspector/InspectorTab.tsx`

### CENSUS-STREAM: Opt-in WebSocket census subscription with its own cadence

Context: the census is excluded from the metrics WebSocket stream because the controller refreshes metrics every tick. A separate subscription with an independent, much coarser cadence would let live observers watch the distribution without paying for it per tick.
References: `docs/API-SPEC.md`, `rust/src/web/ws.rs`, `rust/src/web/controller.rs`

### CENSUS-LOG-BUCKETS: Give the census histograms a logarithmic option

Delivered (2026-08-21): `BucketHistogram` gained a `scale` field, `linear` or `log2`. On the log2 scale bucket 0 counts the value 0 and bucket `i` counts `[2^(i-1), 2^i)`, so 33 buckets span every `u32` and `overflow` is 0 by construction. `count`, `sum`, and `max` remain exact on both scales.

Context that motivated it: linear bucketing fits sizes and offspring well, and generation acceptably, but not stack depth. Measured on `web-256x256` at tick 8000 (65 062 programs, 99.3% occupancy) stack depth was p50 759 / p75 2893 / p90 8315 / p99 31 178 / max 32 767 — the full `PROGRAM_SIZE_CAP` stack range — which left 58.9% of the population in the overflow bin even after widening to 512 linear buckets. A near-empty linear overflow bin would have needed 32 768 buckets in every metrics row.

Only `stack_depths` switched to log2. Sizes, `generation`, and `offspring` stay linear because their measured distributions fit: live size p50 63 / max 286, generation max 319, offspring max 4. The mechanism is in place if any of them ever needs switching. `scale` is always serialized and defaults to linear when absent, so this rode inside the census's existing API 0.2.5 bump with no new version number.
References: `rust/src/observe.rs`, `docs/API-SPEC.md`, `docs/analysis/2026-08-13_performance-roadmap.md`

### RAYON-HOTSPOTS: Remaining performance tiers after the sampler work

Context: the recorded roadmap covers what is left once DYADIC-SAMPLERS lands: a digest-identical SoA hot-field split of the grid sweeps (2a), inline program storage (2b — tried 2026-08-22 and shelved as a stride-dominated regression at every cap; see `docs/analysis/2026-08-22_inline-program-storage-results.md`), and the open Pass 1 dispatch investigation. Re-profile after each step; parallel absorption, Pass 2 scratch reuse, and further traversal fusion remain profile-gated.

Tier 2b prerequisite data (2026-08-21, EMERGENCE-CENSUS): measured on `web-256x256` at tick 8000, 65 062 programs at 99.3% occupancy. **Live program size** is p50 63 / p90 143 / p99 195 / max 286 - a code cap in the 256-320 range would cover essentially the whole population, so inline code storage looks viable. **Stack depth is the opposite story**: p50 759 / p75 2893 / p90 8315 / p99 31 178 / max 32 767, i.e. the distribution spans the entire `PROGRAM_SIZE_CAP` stack limit and 77.8% of programs sit above depth 64. Inline stack storage behind any small cap would spill for most of the population; the roadmap's note that "capping the stack alone buys nothing" is confirmed, and a stack cap low enough to inline would be a behavior change, not an optimization.
References: `docs/analysis/2026-08-13_performance-roadmap.md`, `docs/analysis/2026-08-12_rayon-optimization-results.md`, `rust/src/pass1.rs`, `rust/src/pass3.rs`, `rust/src/grid.rs`, `rust/src/model.rs`

### SEED-BOOTSTRAP: Extract seed-program bootstrap module

Context: keep seed placement outside `SimConfig`; this is code-organization follow-up, not a semantics change.
References: `docs/API-SPEC.md`, `rust/src/web/controller.rs`

### SEED-ENVIRONMENT: Support seed-neighborhood resource preload

Problem: the current API config can seed resources only on occupied program cells, so it cannot represent the spec seed's recommended resource-rich neighboring empty cells.

### CONTROLLER-LIFECYCLE: Clean up web-controller lifecycle state machine

Context: `created` / `running` / `paused` remain a web-layer concern; this follow-up is about controller structure, not moving lifecycle into the engine core.

Also define WebSocket destroy ordering for throttled frames. A frame queued in `FrameSubscription::pending` can race with the destroy notification when both the frame timer and `destroy_rx` are ready. Do not assume `tokio::select! { biased; ... }` alone establishes the externally visible guarantee; specify the boundary and clear or suppress pending delivery as needed.

Acceptance test: queue a frame behind the FPS throttle, destroy the simulation while that frame is pending, and assert that the socket closes without sending a binary frame after destroy is acknowledged.

References: `docs/API-SPEC.md`, `rust/src/web/types.rs`, `rust/src/web/controller.rs`, `rust/src/web/ws.rs`

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

Context: the backend now validates `r_energy` and `r_mass` as non-negative Poisson arrival rates, while `d_energy_log2` and `d_mass_log2` are optional integer decay exponents (`0..=63`, or null for no decay). This preserves the completed CONFIG-RATES distinction between arrival rates and per-quantum decay probabilities.
References: `docs/SPEC.md`, `docs/API-SPEC.md` §8, `rust/src/config.rs`, `rust/src/pass3.rs`, `frontend/src/constants.ts`

### CONFIG-SCENARIO: Share one engine-owned scenario type between the web layer and the runner

Context: the simulation input field list currently exists four times. `SimConfig` (`rust/src/config.rs`) is canonical, `CreateSimulationRequest` re-lists all sixteen fields as `Option`, the resolved `SimulationConfig` re-lists them again flat with the bootstrap arrays inlined, and `frontend/src/types.ts` mirrors that flat shape a fourth time. Adding one config field means touching all four. The runner already composes the canonical types directly (`RunManifest.simulation` + `RunManifest.bootstrap`); the web layer is the outlier.

Do not merge the run manifest and the web config, and do not make either a superset of the other. Their strictness policies are deliberately opposed: the manifest requires every field and denies unknown fields because `input_digest` is computed over a canonical projection (`docs/RUNNER-SPEC.md` §7), so implicit defaults would make a manifest's meaning depend on the binary that read it; the web request is optional-with-defaults because that is the ergonomics of `POST /v1/sim`. The manifest also carries run-only concerns (`run_id`, `limits`, `output_directory`) that are meaningless for an open-ended web session, which negotiates observation cadence per WebSocket connection instead.

Factor the intersection instead. Add an engine-owned `Scenario { simulation: SimConfig, bootstrap: BootstrapConfig }` in a new `rust/src/scenario.rs`. `Scenario` must be engine-owned, not runner-owned: placing it under `runner/` would force the web layer to depend on a runner module in order to construct a simulation, inverting the layering, and `docs/RUNNER-SPEC.md` §2 lists web integration as an explicit non-goal. Its natural portable serialization stays nested (`{"simulation": {...}, "bootstrap": {"programs": [], "environment": []}}`).

In scope:

- Add `Scenario` and have `ManagedSimulation` store it directly, removing the per-construction `to_engine_config()` / `bootstrap_config()` conversions at `rust/src/web/controller.rs:273`, `:456`, and `:552`.
- Delete the resolved `SimulationConfig` mirror in `rust/src/web/types.rs`.
- Preserve the existing flat v1 response shape through a web-owned, serialization-only view that flattens `SimConfig` and renames the bootstrap arrays to `seed_programs` / `seed_environment`. Do not give `Scenario` itself custom flat serialization.
- Keep `CreateSimulationRequest` unchanged and typed.
- Keep `RunManifest` and its digest bytes byte-identical.

Explicit non-goals for this item:

- Do not replace the typed request DTO with generic `serde_json::Map` overlay merging. It would silently collapse duplicate keys that derived `Deserialize` currently rejects, weaken error messages by dropping line/column, require lifting `seed_programs` / `seed_environment` out of the map before deserializing the remainder as a `deny_unknown_fields` `SimConfig`, and would newly reject unknown fields that `CreateSimulationRequest` accepts today.
- Web-to-runner export (a tuned session emitted as a run manifest, `Scenario` plus `run_id` / `limits` / `output_directory`) is a separate follow-up.
- A canonical nested request body plus a defaults endpoint is a versioned API decision, not part of this refactor. Note that a defaults endpoint removes duplicated default *values* only; eliminating the TypeScript field list additionally requires generated types (OpenAPI or JSON Schema), since the frontend accesses fields such as `config.width` statically.

Acceptance: `rust/tests/web_api.rs` and `rust/tests/runner_cli.rs` pass unchanged, and the §7 canonical digest projection for the `docs/RUNNER-SPEC.md` §6 example manifest is unchanged.
References: `docs/RUNNER-SPEC.md` §2, §6, §7, `docs/API-SPEC.md` §8, `rust/src/config.rs`, `rust/src/bootstrap.rs`, `rust/src/runner/mod.rs`, `rust/src/web/types.rs`, `rust/src/web/controller.rs`, `frontend/src/types.ts`, `FRONTEND-CONFIG-TOOLS`, `FRONTEND-DEFAULTS`

### COORDINATE-CONVENTIONS: Standardize frontend coordinates as 0-indexed and display them in `(y, x)` order

Context: current frontend UI and config tooling still lean on `x, y` ordering from the API surface. Reconcile the display language, validation messaging, and editor fields so the UI is consistently 0-indexed and uses `(y, x)` ordering. This is intentionally marked tricky because it cuts across inspector display, seed-program editing, hit-testing labels, and API request mapping.
References: `docs/FRONTEND-SPEC.md`, `docs/API-SPEC.md` §12, `frontend/src/components/controls/ConfigEditor.tsx`, `frontend/src/components/inspector/InspectorTab.tsx`, `frontend/src/components/GridCanvas.tsx`

### NO-SIM-STATUS: Replace the frontend startup `404 /v1/sim` probe with a cleaner no-simulation status path

Context: the frontend currently probes `GET /v1/sim` on load and treats `404` as the expected "no sim exists" case. This is functionally fine, but it produces a noisy red network entry in browser devtools. Low priority cleanup: add a cleaner backend/API path for empty-state status or otherwise remove the expected-404 startup probe.
References: `docs/API-SPEC.md` §7, `rust/src/web/mod.rs`, `frontend/src/context/SimContext.tsx`, `frontend/src/lib/api.ts`

### DOCS-RESOLVED-QUESTIONS: Establish a convention for resolved open questions in spec docs

Context: API-SPEC.md §16 "Open Questions" now contains a resolved entry (16.3). This pattern will recur as more open questions get resolved across spec documents. Need a consistent convention: either remove resolved items, move them to a "Resolved" subsection, or reference the resolving PR/commit. Applies to all docs with open-question sections (API-SPEC.md, SPEC.md, SPEC-COMPANION.md, FRONTEND-SPEC.md, etc.).
References: `docs/API-SPEC.md` §16, `docs/SPEC.md`, `docs/SPEC-COMPANION.md`, `docs/FRONTEND-SPEC.md`

### FRONTEND-STATIC-CHECKS: Add lightweight frontend static checks for stale vars and similar mistakes

Context: add a fast frontend-only static-analysis pass so mistakes like stale variable names, unused locals after refactors, and similar TypeScript-level issues are caught without relying on a full production build. Start with the lightest-weight option that fits the current stack.
References: `frontend/package.json`, `frontend/tsconfig.app.json`, `frontend/tsconfig.node.json`

### FRONTEND-ARCH-CLEANUP

Do a focused cleanup pass on frontend lifecycle and ownership boundaries. Do not rewrite the app.

Requirements:
- Use Effects only to synchronize with external systems (WebSocket, ResizeObserver, canvas/chart adapters).
- Do not use Effects to derive state from other state/props.
- Isolate imperative integrations behind small adapter hooks/components with explicit setup/update/cleanup.
- Keep layout ownership explicit: one component measures, children consume.
- Split context by responsibility and update cadence; avoid broad rerenders from high-frequency simulation/metrics updates.
- Keep refs for mutable non-render values only.

Targets:
- `frontend/src/context/SimContext.tsx`
- `frontend/src/context/WebSocketContext.tsx`
- `frontend/src/components/GridCanvas.tsx`
- `frontend/src/components/MetricsDrawer.tsx`
- `frontend/src/App.tsx`

Validation:
- `npm run check:frontend` passes
- no new lint suppressions without explanation
- chart/canvas mount-cleanup works under remount
- resizing and drawer open/close do not produce stale measurements

### INSPECTOR-TRACK-PROGRAM: Let the inspector follow a selected program as it moves

Context: the current inspector is cell-centric. Add a mode that keeps the inspector locked onto the same program identity as it moves between cells, rather than dropping focus when the originally selected cell changes. This likely needs backend support for stable per-program identity and/or a direct "find current location for program id" path.
References: `docs/API-SPEC.md`, `docs/FRONTEND-SPEC.md`, `frontend/src/components/inspector/InspectorTab.tsx`, `frontend/src/context/SimContext.tsx`, `rust/src/observe.rs`, `rust/src/web/mod.rs`

### SEED-PROGRAM-QOL: Add seed-program library/import-export/disassembly tooling

Context: improve seed-program authoring without changing simulator semantics. Candidate scope includes a small reusable library of saved seed programs, JSON save/load for seed-program sets, and opcode/disassembly helpers so raw byte arrays are easier to inspect and edit.
References: `docs/API-SPEC.md` §8, `docs/FRONTEND-SPEC.md` §9, `frontend/src/components/controls/ConfigEditor.tsx`, `frontend/src/lib/config.ts`

### TRANSPORT-CONTROLS: Promote play-pause-speed controls and add hotkeys

Context: move the main transport actions (play, pause, step, speed selection) into a top-level location that stays accessible while the inspector is open, and add game-style keyboard shortcuts. Initial shortcut ideas: `Space` for play/pause and number keys for speed presets. Define clear focus/typing guards so shortcuts do not interfere with text input fields.
References: `docs/FRONTEND-SPEC.md`, `frontend/src/App.tsx`, `frontend/src/components/StatusBar.tsx`, `frontend/src/components/controls/ControlsTab.tsx`, `frontend/src/context/SimContext.tsx`

### CONFIG-DYADIC-K

Delivered (2026-08-21): `SimConfig` stores `d_energy_log2`, `d_mass_log2`,
`maintenance_rate_log2`, and `p_spawn_log2` as `Option<u32>`. `Some(k)`
means `2^-k` for `k` in `0..=63`; `None`/JSON `null` means never.
Absent decay and maintenance fields default to `Some(7)`; absent spawn defaults
to `None`. Serialization always emits explicit nulls. The float dyadic
derivation and validation classes were deleted, the frontend uses bounded
integer inputs with live previews, API-SPEC is 0.3.0, and the runner golden
digest is `sha256:6eec9c637d8967472f2b9712ee52014968086285e5fc14ad32cba13d38fe9292`.
The fractional maintenance term still converts `2^-k` exactly to `f64`.

### MUTATION-ANY-QUANTUM

Delivered (2026-08-21): each consumed background quantum independently triggers
with probability `2^-mutation_background_log2`, sampled exactly as
`binomial_pow2(rng, x, k) > 0`. If any trigger fires, the program mutates
once; trigger counts do not cause multiple mutations. `k = 0` is deterministic
and consumes no trigger draw, so the following instruction-index and bit-index
draws keep their order. The dead `bernoulli_ratio_pow2` helper, re-export, and
tests were removed and replaced with probability-law and draw-accounting tests.
### MUTATION-DOSE-TUNING: Decide whether `mutation_background_log2` should move off 8

Context (2026-08-21, ambient rebalance sweep): the ambient background level is simultaneously the energy a program absorbs and the numerator of its mutation probability — `mutate_end_of_tick_cell` uses `min(bg_radiation_consumed / 2^mutation_background_log2, 1)` — so `r_energy` moves food and mutation load together and cannot separate them. `mutation_background_log2` is the only knob that addresses the mutation channel alone, and it has never been tuned. Measured on the `web-256x256-single` fixture at `r_energy = 0.25`: raising the exponent from 8 to 9 (halving the dose, income unchanged) took grid fill from 13/16 to **8/8 with no run below 57,175 programs**, eliminating the bimodal low mode rather than shifting its odds.

This is deliberately *not* folded into a rate rebalance. Halving the background-stressed mutation rate halves what `2026-08-13_mutation-load-and-emergence-milestones.md` §3 identifies as the substrate's operative evolutionary clock, so a world that reliably fills the grid may be a less interesting world. Wants its own investigation against the emergence milestones, not an occupancy metric.

Related open question that the sweep could not answer: whether the old `d_energy = 0.01` defaults were genuinely better in ensemble or whether the single recorded old-default checkpoint was also just a favourable seed. `d = 0.01` is now rejected by dyadic validation, so answering it needs a throwaway build with relaxed validation.
References: `docs/analysis/2026-08-21_ambient-rebalance-sweep.md`, `docs/analysis/2026-08-13_mutation-load-and-emergence-milestones.md`, `rust/src/pass3.rs`, `rust/src/config.rs`

### AMBIENT-SAMPLER-COST: Cut the per-cell ambient RNG work that dominates empty and sparse ticks

Context (2026-08-22, Tier 2a stride diagnostic): on an empty 256x256 grid `ambient` is 88% of the tick and costs ~70 ns/cell regardless of cell stride (a 24-byte-stride build moved it −0.9%). The cost is `pass3_ambient` seeding `cell_rng` twice per cell and running `binomial_pow2` + `sample_poisson` on every cell, occupied or not. Candidates: skip the decay draw when `bg_radiation == 0` / `bg_mass == 0` (probability-0 branches consume no draws, so this is stream-preserving only if the sampler already short-circuits — verify), derive one seed per cell and split it, or batch the Poisson inversion. Any change that moves draw positions needs the full sampled-literal reconciliation from CLAUDE.md and a parity proof for the cases that should not move. Bench on empty-256x256 (the anchor for this phase) and the dense fixtures.
References: `docs/analysis/2026-08-22_tier-2a-stride-diagnostic.md`, `rust/src/pass3.rs`, `rust/src/random.rs`

### BENCH-WEB-FIXTURE-ENSEMBLE: Stop using a single-seed grown-web checkpoint as a comparison basis

Delivered (2026-08-22): `tick_bench` now defaults to `web-256x256-ens`, a fixed
four-seed set spanning two takeoff and two stall outcomes from the ambient
rebalance sweep. Each seed retains the parser-stable repetition and activity
lines, and the fixture reports the median and min-max of per-seed repetition
medians. Each seed builds its own checkpoint; use the established
`start_tick = 6500` convention because the takeoff/stall modes separate only
after the roughly 500-3000-tick bifurcation. The bimodal
`web-256x256-single` fixture remains available by an explicit substring filter
but no longer runs by default or when the filter also matches the ensemble.

Context (2026-08-21, ambient rebalance sweep): `web-256x256-single` is bimodal. Across 88 runs its final population splits into two clusters with an 18,336-program gap and nothing in between, and the fixture seed `6846702536457205` sits in the minority mode under the current defaults — it finishes at 8,110 programs where the median seed finishes at 56,312. Worse, its response to `r_energy` is non-monotone (fills the grid at backgrounds 25, 28 and 36; stalls at 32), so a single-seed grown-web checkpoint can swing by an order of magnitude on a parameter change that barely moves the ensemble.

That is what produced the misleading tick-6500 A/B in `2026-08-21_dyadic-sampler-results.md`. Dense-regime benchmarking should use a synthetic dense fixture (`dense-additive-128x128`, already the honest measurement there) or report a seed ensemble with a stated spread.
References: `docs/analysis/2026-08-21_ambient-rebalance-sweep.md`, `docs/analysis/2026-08-21_dyadic-sampler-results.md`, `rust/examples/tick_bench.rs`
