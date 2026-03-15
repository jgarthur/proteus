# Handoff: Frontend MVP Implementation

## Deliverable

A working React + TypeScript frontend application in `frontend/` at the repo root. It implements the **MVP** feature set from `docs/FRONTEND-SPEC.md` §12.

## Source Documents (priority order)

1. `docs/FRONTEND-SPEC.md` — authoritative for all frontend behavior, layout, component structure, state shape, and technology choices. Follow it closely.
2. `docs/API-SPEC.md` — authoritative for the backend contract. The frontend must not assume capabilities beyond what this spec exposes.
3. `docs/SPEC.md` — background reference for simulation semantics. Do not need to read in full; consult only if you need to understand what a metric or field means.

## Decisions already made

- **Stack**: React 18 + TypeScript + Vite + CSS Modules. No component library.
- **Grid rendering**: Canvas 2D (MVP). Follow the offscreen canvas + `putImageData` + transformed `drawImage` approach in FRONTEND-SPEC §4.
- **Charts**: uPlot for time-series metrics.
- **State**: React Context + `useReducer`. No external state library.
- **HTTP**: Native `fetch`. No axios or similar.
- **WebSocket**: Native `WebSocket`. No socket.io or similar.
- **CORS**: The backend already serves `Access-Control-Allow-Origin: *`. No Vite proxy needed. The frontend connects directly to the backend (default `localhost:3000`).
- **Snapshot panel**: Render as disabled / "unavailable" — backend does not expose snapshot routes yet.

## Scope

Build everything in the **MVP** column of FRONTEND-SPEC §12. Specifically:

- Project scaffolding (Vite + React + TypeScript)
- WebSocket connection with reconnection logic (FRONTEND-SPEC §3)
- Binary frame parsing (FRONTEND-SPEC §3)
- Grid canvas with Canvas 2D renderer, zoom/pan, click-to-select (FRONTEND-SPEC §4)
- All 8 color maps (FRONTEND-SPEC §4)
- Lifecycle controls (FRONTEND-SPEC §5)
- Config editor with defaults and validation (FRONTEND-SPEC §9)
- Cell inspector with disassembly view (FRONTEND-SPEC §7)
- Status bar with scalar metrics (FRONTEND-SPEC §6)
- Metrics drawer with 5 uPlot charts (FRONTEND-SPEC §6)
- Frame rate and metrics sampling controls (FRONTEND-SPEC §5)
- Sidebar with Controls/Inspector tabs, collapsible (FRONTEND-SPEC §10)
- Layout matching FRONTEND-SPEC §10 sketches

## Non-goals

- WebGL renderer (Later)
- Minimap, smooth zoom animation, keyboard shortcuts (Later)
- Assembly input for seed programs (Later)
- Any backend or spec changes
- Tests (we'll add those in a follow-up)

## Constraints

- Put all frontend code under `frontend/` at the repo root.
- Add a `frontend/README.md` explaining how to install and run (`npm install`, `npm run dev`).
- The backend URL should be configurable (environment variable or constant), defaulting to `http://localhost:3000`.
- Follow the component tree, state shape, and data flow from FRONTEND-SPEC §3 exactly. Don't invent a different architecture.
- Follow the layout and sizing rules from FRONTEND-SPEC §10.
- Follow the performance guidance — store frame data and metrics buffers in `useRef`, not React state (FRONTEND-SPEC §3, §6).

## Verification

When done:

1. `cd frontend && npm install && npm run dev` starts without errors.
2. `npm run build` produces a production build without errors.
3. With the backend running (`cargo run --bin proteus-server --features web` from `rust/`), the frontend connects, creates a simulation, and displays the grid.
4. All lifecycle controls work (create, start, pause, resume, step, reset, destroy).
5. Color map switching works without re-fetching data.
6. Clicking a cell opens the inspector with program disassembly.
7. Metrics charts update in the expandable drawer.

## Files to update after implementation

- `README.md` (repo root) — add `frontend/` to the Project Orientation list.
- `STATUS.md` — mark "Build the frontend" as done if the MVP is complete.
