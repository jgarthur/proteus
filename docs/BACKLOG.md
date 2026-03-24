# Proteus Backlog

Durable parking lot for explicitly deferred follow-up work that should survive beyond a single handoff prompt or conversation.

This file is not authoritative for simulator semantics. `SPEC.md` and `SPEC-COMPANION.md` remain the source of truth for behavior. Backlog items here capture agreed future work, cleanup, and additive extensions.

Items are referenced by name from `STATUS.md` at the repo root. Use the exact item name when marking work done in either file.

## Items

### RAYON-BASELINE: Decide whether Rayon should replace the separate serial iteration paths

Context: the current backend keeps a feature-gated non-Rayon path alongside the Rayon path for the newly parallelized per-cell loops. Revisit whether that duplication is worth keeping, or whether the crate should standardize on the Rayon iterator path and rely on a 1-thread pool when effectively running serially.
References: `rust/src/pass1.rs`, `rust/src/pass3.rs`, `rust/src/simulation.rs`, `.github/workflows/rust.yml`

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
