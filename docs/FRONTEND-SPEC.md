# Proteus Frontend Specification (Provisional)

**Status**: Provisional — subject to change as the backend API matures.

**Spec version**: 0.3.1

**Changed in 0.3.1**: the cell inspector gained a Lineage group showing the six lineage fields API-SPEC 0.2.5 added to cell inspection (§7).

**Changed in 0.3.0**: the config editor uses the API-SPEC 0.3.0 exponent fields. Each probability exponent is an integer in `0..=63`; the optional fields may be empty/`null` for never, and all exponent inputs show a live `2^-k` probability preview (§9).

**Targets**: Proteus v0.4.0, API-SPEC v0.3.1

---

## 1. Purpose and Status

This document defines the **user-facing frontend application** for the Proteus simulator. It specifies the technology stack, architecture, component structure, and behavior for a desktop-first web application that visualizes and controls a running Proteus simulation.

The frontend builds against the API contract defined in `API-SPEC.md`. It does not assume backend capabilities beyond what that spec exposes.

This is a design spec intended to be concrete enough for implementation. Features are separated into **MVP** (minimum viable product) and **Later** (deferred enhancements).

### Scope

- Grid visualization with zoom, pan, and click-to-inspect
- Simulation lifecycle controls
- Real-time metrics display and charting
- Cell inspection with program disassembly
- Snapshot management target design (deferred until backend support exists)
- Config editing for simulation creation

### Non-goals

- Mobile or responsive design (desktop-first, localhost)
- Authentication or multi-user features
- Deployment or hosting infrastructure
- Backend implementation details
- Simulation semantics beyond what's needed for display

---

## 2. Technology Choices

| Layer | Choice | Rationale |
|-------|--------|-----------|
| Framework | React 18 + TypeScript | Widely supported by AI tooling; legacy prototype uses React+Vite; large ecosystem for UI components |
| Build | Vite | Fast HMR, zero-config TypeScript support, proven with React |
| Grid rendering (MVP) | Canvas 2D | Simplest path to 1M cells; `putImageData` on offscreen canvas, `drawImage` with transform for zoom/pan |
| Grid rendering (Later) | WebGL 2 | Drop-in swap via `GridRenderer` interface; needed when Canvas 2D can't maintain frame budget at 1024×1024 |
| Charts | uPlot | ~35KB, handles 100k+ points, no framework dependency, fastest time-series library available |
| Zoom/pan | Custom canvas transform | ~80 lines of mouse/wheel handling; avoids library dependency for a well-bounded problem |
| State management | React Context + `useReducer` | Sufficient for a single-session app with one WebSocket connection; no external state library needed |
| HTTP client | Native `fetch` | All REST calls are simple JSON request/response; no library needed |
| WebSocket | Native `WebSocket` | Single connection, simple message protocol; no library needed |
| Styling | CSS Modules | Scoped styles, no runtime cost, works out of the box with Vite |

---

## 3. Architecture Overview

### Data flow

```
                          REST (fetch)
  ┌──────────┐     ───────────────────────►  ┌──────────┐
  │          │    POST /v1/sim, /v1/sim/start │          │
  │          │    GET  /v1/sim/cell/:index    │          │
  │ Frontend │    POST /v1/sim/snapshot*      │ Backend  │
  │  (React) │                                │  (Rust)  │
  │          │     ◄───────────────────────   │          │
  │          │        WebSocket /v1/ws        │          │
  └──────────┘     binary frames + JSON       └──────────┘
                        metrics
```

`/v1/sim/snapshot*` is a deferred target surface in the current backend, not an available MVP route.

### Component tree

```
App
├── WebSocketProvider          (context: connection state, subscribe/unsubscribe)
├── SimStateProvider           (context: sim status, metrics, latest frame)
│
├── GridCanvas                 (Canvas 2D renderer, zoom/pan, click handler)
│
├── Sidebar                    (360px, collapsible, tabbed)
│   ├── ControlsTab
│   │   ├── LifecycleControls  (create/start/pause/resume/step/reset/destroy)
│   │   ├── FrameRateControl   (max_fps slider for frame subscription)
│   │   ├── MetricsSampling    (every_n_ticks control)
│   │   ├── SnapshotPanel      (deferred; hidden or disabled until backend support lands)
│   │   └── ConfigEditor       (creation-time config form)
│   └── InspectorTab
│       └── CellInspector      (selected cell details + disassembly)
│
├── StatusBar                  (48px, always visible, key scalar metrics)
│   └── ColorMapSelector       (dropdown or button group for color mode)
│
└── MetricsDrawer              (expandable from status bar, uPlot charts)
    ├── PopulationChart
    ├── EnergyMassChart
    ├── BirthDeathChart
    ├── MutationRatesChart
    ├── ProgramSizeChart
    └── DiversityChart
```

### State shape

```typescript
interface AppState {
  // Connection
  wsStatus: 'disconnected' | 'connecting' | 'connected';

  // Simulation
  simStatus: 'none' | 'created' | 'running' | 'paused';
  tick: number;
  gridWidth: number;
  gridHeight: number;

  // Subscriptions
  frameSubscribed: boolean;
  metricsSubscribed: boolean;
  maxFps: number;
  everyNTicks: number;

  // Latest frame (binary, not stored in React state — held in ref)
  // latestFrame: DataView  (via useRef, not in reducer)

  // Metrics history (rolling buffer, not in React state — held in ref)
  // metricsBuffer: MetricsBuffer  (via useRef)

  // UI
  selectedCell: { x: number; y: number } | null;
  colorMap: ColorMapMode;
  sidebarOpen: boolean;
  sidebarTab: 'controls' | 'inspector';
  metricsDrawerOpen: boolean;
}
```

The latest binary frame and metrics rolling buffer are stored in `useRef` (not reducer state) to avoid unnecessary re-renders on every frame. The grid canvas reads from the ref on each `requestAnimationFrame` paint.

### WebSocket lifecycle

1. **Connect**: Open `WebSocket` to `ws://{host}/v1/ws` when the app mounts.
2. **Subscribe frames**: Send `{"subscribe": "frames", "max_fps": 30}` after connection opens, if a simulation exists.
3. **Subscribe metrics**: Send `{"subscribe": "metrics", "every_n_ticks": 1}` after connection opens.
4. **Receive**: Binary messages are grid frames (parse header + CellView array). JSON messages with `"type": "metrics"` are metrics updates. JSON messages with `"type": "error"` are logged to console.
5. **Reconnect**: On close or error, attempt reconnection with exponential backoff (1s, 2s, 4s, 8s, max 30s). Re-subscribe on reconnect. Subscriptions are stateless per API-SPEC §16 Q4. The first received metrics snapshot establishes or updates the cumulative-event baseline. An older tick in the same epoch is discarded; a changed epoch or regressed total clears the old chart series before rebaselining.
6. **Teardown**: Close WebSocket on app unmount.

### Binary frame parsing

```typescript
function parseFrame(buffer: ArrayBuffer): GridFrame {
  const view = new DataView(buffer);
  const tick    = view.getBigUint64(0, true);   // u64 LE
  const width   = view.getUint32(8, true);      // u32 LE
  const height  = view.getUint32(12, true);     // u32 LE
  const cells   = new DataView(buffer, 16);     // N × 8-byte CellView
  return { tick, width, height, cells };
}
```

Each CellView is 8 bytes at offset `(y * width + x) * 8` within the cells DataView (per API-SPEC §11):

| Byte | Field | Range |
|------|-------|-------|
| 0 | flags (bit 0: has_program, bit 1: is_live, bit 2: is_open) | 0–7 |
| 1 | program_id | 0–255 |
| 2 | program_size, log2-scaled: `min(255, round(17 * log2(size)))`, 0 if empty | 0–255 |
| 3 | free_energy (clamped) | 0–255 |
| 4 | free_mass (clamped) | 0–255 |
| 5 | bg_radiation (clamped) | 0–255 |
| 6 | bg_mass (clamped) | 0–255 |
| 7 | reserved | 0 |

---

## 4. Grid Visualization

### Renderer abstraction

To allow a future WebGL upgrade without rewriting the grid component, rendering is behind an interface:

```typescript
interface GridRenderer {
  attach(canvas: HTMLCanvasElement): void;
  resize(width: number, height: number): void;
  render(
    cells: DataView,
    gridW: number,
    gridH: number,
    viewport: ViewportTransform,
    colorFn: ColorMapFn
  ): void;
  hitTest(canvasX: number, canvasY: number): { x: number; y: number } | null;
  destroy(): void;
}

interface ViewportTransform {
  offsetX: number;  // pan offset in canvas pixels
  offsetY: number;
  scale: number;    // zoom level (1.0 = 1 cell = 1 pixel)
}

type ColorMapFn = (cellView: DataView, byteOffset: number) => number; // returns 0xRRGGBB
```

### Canvas 2D implementation (MVP)

The MVP renderer uses an offscreen canvas + `putImageData` + transformed `drawImage`:

1. **Offscreen canvas**: Sized to `gridW × gridH` pixels. One pixel per cell.
2. **Color pass**: Iterate all cells, call `colorFn` for each, write RGBA into an `ImageData` buffer. This is a tight loop — the `ColorMapFn` reads raw bytes from the CellView DataView with zero allocation per cell.
3. **Put**: `offscreenCtx.putImageData(imageData, 0, 0)`.
4. **Draw**: On the visible canvas, apply the viewport transform (`translate` + `scale`), then `drawImage(offscreen, 0, 0)`. Set `imageSmoothingEnabled = false` so cells render as crisp squares when zoomed in.
5. **Highlight**: If a cell is selected, draw a 1px outline at the cell's grid position (transformed to canvas coordinates).

This approach separates the per-cell color computation (which touches every cell) from the viewport transform (which is a single GPU-accelerated `drawImage`).

### WebGL 2 upgrade path (Later)

A `WebGL2Renderer` implementing the same `GridRenderer` interface:

- Upload the raw CellView buffer as a texture (one texel per cell, or packed into RGBA channels).
- Color mapping is done in a fragment shader — eliminates the JavaScript color loop entirely.
- Zoom/pan is a uniform matrix, no CPU work.
- Expected to handle 1024×1024 grids at 60fps where Canvas 2D may struggle.

Swapping renderers is a one-line change in the `GridCanvas` component.

### Zoom and pan

| Interaction | Behavior |
|-------------|----------|
| Mouse wheel | Zoom in/out centered on cursor position. Scale range: 0.5× to 64×. |
| Click + drag | Pan the viewport. |
| Click (no drag) | Select cell for inspection (see §7). |
| Double-click | Zoom to fit grid in viewport. |

Implementation: track `ViewportTransform` in component state. On wheel events, adjust `scale` and recalculate `offsetX`/`offsetY` to keep the cursor-world-point fixed. On mousemove with button held, adjust offsets.

### Variable frame rate handling

The server drops frames to stay at or below the client's requested `max_fps` (API-SPEC §11). The frontend:

- Stores the latest frame in a `useRef` (not state) — receiving a new frame overwrites the previous one.
- Runs a `requestAnimationFrame` loop that reads the latest frame ref and calls `renderer.render()`. This decouples the WebSocket receive rate from the paint rate.
- If no new frame has arrived since the last paint, the RAF loop skips rendering (no wasted work).

### Color maps

Eight modes, all one click away via the `ColorMapSelector` in the status bar:

| Mode | Source byte(s) | Mapping |
|------|---------------|---------|
| Occupancy | flags (byte 0) | Empty → black, inert → dark gray, live → white |
| Program ID | byte 1 | Hash-based hue (program_id → HSL, s=0.7, l=0.5). Empty → black |
| Program Size | byte 2 | Blue→yellow ramp over `min(1, byte / 170)`. Byte 2 is log2-scaled, so the ramp spans sizes 1..1024 and saturates above. Empty → black |
| Free Energy | byte 3 | Linear black→green ramp |
| Free Mass | byte 4 | Linear black→blue ramp |
| Bg Radiation | byte 5 | Linear black→red ramp |
| Bg Mass | byte 6 | Linear black→cyan ramp |
| Combined | bytes 3,5,4 | RGB composite: R=bg_radiation, G=free_energy, B=free_mass. Programs outlined |

Each `ColorMapFn` reads exactly the bytes it needs from the CellView — no object allocation, no property access, just `DataView.getUint8()`.

---

## 5. Control Panel

The control panel lives in the **Controls** tab of the sidebar. It maps directly to API-SPEC §7 and §9 endpoints.

### Lifecycle controls

| Button | API call | Enabled when | Notes |
|--------|----------|-------------|-------|
| **Create** | `POST /v1/sim` with config body | `simStatus === 'none'` | Opens config editor if not already filled |
| **Start** | `POST /v1/sim/start` | `simStatus === 'created'` | Transitions to running |
| **Pause** | `POST /v1/sim/pause` | `simStatus === 'running'` | Idempotent |
| **Resume** | `POST /v1/sim/resume` | `simStatus === 'paused'` | |
| **Step** | `POST /v1/sim/step?count=N` | `simStatus === 'paused'` | Text input for count, default 1 |
| **Reset** | `POST /v1/sim/reset` | `simStatus !== 'none'` | Confirm dialog before executing |
| **Destroy** | `DELETE /v1/sim` | `simStatus !== 'none'` | Confirm dialog before executing |

### UI state machine

```
  [none] ──Create──► [created] ──Start──► [running]
    ▲                    ▲                  │  ▲
    │                    │              Pause│  │Resume
    │                    │                  ▼  │
  Destroy              Reset            [paused]
    │                    │                  │
    └────────────────────┴──────────────────┘
                                        Step (stays paused)
```

After each control operation, the frontend polls `GET /v1/sim` to confirm the new state and update `simStatus`.

### Frame rate control

A slider (1–60) sets `max_fps` for the frame subscription. Changing it sends an `unsubscribe` + re-`subscribe` message on the WebSocket with the new `max_fps` value. Default: 30.

### Metrics sampling control

A numeric input sets `every_n_ticks` for the metrics subscription. Changing it sends `unsubscribe` + re-`subscribe` on the WebSocket. Default: 1. Higher values reduce WebSocket traffic during long observation runs. The birth/death and mutation-rate charts remain correct at coarser sampling because they derive rates from cumulative `event_totals`, not by summing the delivered per-tick fields.

---

## 6. Metrics Dashboard

### Status bar (always visible)

A thin bar (48px) at the bottom of the viewport showing key scalar metrics from the latest metrics message (API-SPEC §10):

| Field | Source | Format |
|-------|--------|--------|
| Tick | `tick` | Integer with comma separators |
| Population | `population` (`live_count` / `inert_count`) | e.g. "1,847 (1,623 / 224)" |
| Energy | `total_energy` | Integer with comma separators |
| Mass | `total_mass` | Integer with comma separators |
| TPS | `ticks_per_second` (from `GET /v1/sim`) | One decimal place, e.g. "412.7" |

The status bar also contains the **color map selector** (§4) and a toggle button to expand the metrics drawer.

TPS is polled via `GET /v1/sim` every 2 seconds (not available in the metrics WebSocket message).

### Metrics drawer (expandable)

Clicking the expand toggle on the status bar opens a drawer that slides up from the status bar to 50% viewport height. The drawer contains uPlot time-series charts.

### Chart definitions

Charts are defined as a `ChartDef[]` array. Adding a new chart requires only adding an entry — no component changes.

```typescript
interface ChartDef {
  id: string;
  title: string;
  series: Array<{
    key: keyof MetricsMessage;   // field from the metrics JSON
    label: string;
    color: string;
    axis?: 'left' | 'right';
  }>;
}
```

**MVP charts**:

| Chart | Series | Y-axis |
|-------|--------|--------|
| Population | `live_count`, `inert_count`, `population` | Count |
| Energy & Mass | `total_energy`, `total_mass` | Total (dual axis) |
| Birth / Death Rates | Differences of `event_totals.births`, `.deaths` | Average events per tick over each observation interval; births aggregate both `boot` and spontaneous spawn |
| Mutation Rates | Differences of `event_totals.base_mutations`, `.background_mutations`, `.mutations` | Average events per tick over each observation interval; mutations split into baseline (fired from the per-program mutation probability) and background-stressed (fired because the program consumed background radiation that tick), which sum to the total |
| Program Size | `mean_program_size`, `max_program_size` | Instructions (dual axis) |
| Diversity | `unique_genomes` | Count |

### Rolling buffer

Metrics history is stored in typed arrays (one `Float64Array` per series) with a rolling window of 10,000 points. This gives uPlot a fixed-size data source and bounds memory usage. When the buffer is full, old points are evicted in FIFO order.

The x-axis is `tick` (not wall-clock time) so charts remain meaningful across pauses.

For birth, death, and mutation series, the first metrics snapshot is a baseline and displays a zero rate. Each later snapshot in the same epoch stores `(new_total - old_total) / (new_tick - old_tick)`. Duplicate snapshots replace the point without changing its rate. Older ticks in the same epoch are discarded so a stale REST response cannot overwrite a newer WebSocket observation. An epoch change or a regression in totals clears the rolling buffer and establishes a new baseline. This handles reset, reconnect, and backend restart discontinuities without manufacturing an event spike.

The frontend uses JavaScript `number` values for API `u64` fields and therefore assumes values remain at or below `Number.MAX_SAFE_INTEGER`, as constrained by API-SPEC §10.

---

## 7. Cell Inspector

The cell inspector lives in the **Inspector** tab of the sidebar.

### Trigger

Clicking a cell on the grid (any zoom level) selects it. The `GridRenderer.hitTest()` method converts canvas coordinates to grid coordinates using the current viewport transform. The selected cell is stored in app state (`selectedCell: { x, y }`).

Selecting a cell automatically switches the sidebar to the Inspector tab and opens the sidebar if it was collapsed.

### Display layout

The inspector fetches cell data via `GET /v1/sim/cell?x={x}&y={y}` (API-SPEC §12) and displays:

**Cell header**:
- Coordinates: `(x, y)` and flat index
- Status: empty / live / inert

**Resources** (always shown):

| Field | Value |
|-------|-------|
| Free Energy | exact value |
| Free Mass | exact value |
| Bg Radiation | exact value |
| Bg Mass | exact value |

**Program** (shown when cell has a program):

| Field | Value |
|-------|-------|
| ID | program_id |
| Size | instruction count |
| Age | ticks alive |
| IP | instruction pointer |
| Registers | src, dst, dir, flag, msg, lc |
| Stack | array display |
| Abandonment timer | shown for inert programs |

The `dir` register arrives as the API integer encoding from API-SPEC §12: `0 = right`, `1 = up`, `2 = left`, `3 = down`. The UI should render a human-readable label alongside the numeric value.

**Lineage** (shown when cell has a program):

| Field | Value |
|-------|-------|
| Origin | `Seed` / `Spawn` / `Append`, from the API `origin` tag |
| Generation | 0 for a lineage root, otherwise the creator's generation plus one |
| Birth tick | tick the program became live; `—` while an inert body has never booted |
| Created tick | tick the program first materialized; precedes the birth tick for an inert body |
| uid | monospace, exact digits |
| Parent uid | monospace; `—` for a lineage root (seed or spawn) |

The nullability follows API-SPEC §12: `parent_uid`, `birth_tick`, `origin`, and `created_tick` may be null, `uid` and `generation` are always present. A null field renders as `—`.

A uid encodes its creation site as `1 + ((tick × cell_count + cell_index) × 4 + origin)`, and `cell_count` is `gridWidth × gridHeight`, so the frontend can decode a parent uid without another request. The parent uid is therefore a click target that selects the cell the parent was **created** in — not necessarily where the parent is now, since a program can move or die, so the panel says so. The decode is skipped (and the uid rendered as plain text) when the uid exceeds `Number.MAX_SAFE_INTEGER`, where JSON parsing has already rounded the `u64`, when the origin code is the reserved fourth one, or when the decoded cell index falls outside the current grid.

Uids are epoch-scoped (API-SPEC §12), so fetched cell data is stamped with the simulation instance and grid dimensions it was fetched under; creating, resetting, or destroying a simulation drops retained inspection data along with any fetch in flight, holds the inspector's poll for the duration of the request, advances the epoch once that request lands, and then refetches the selected cell. Each inspection is stamped at fetch start, so a fetch that observed the pre-boundary simulation keeps the pre-boundary epoch. While a stamp does not match the current epoch and dimensions the parent uid renders as plain text rather than a link, because a uid from a previous epoch names a program that no longer exists and — after a resize — would decode to an in-range but wrong cell.

**Disassembly view**:

A scrollable list of instructions with:
- Index number
- Instruction mnemonic (from `disassembly` array)
- Raw opcode byte (from `code` array)
- Current IP highlighted

### Auto-refresh

- **While paused**: The inspector re-fetches cell data after each `Step` operation completes.
- **While running**: The inspector re-fetches on a 500ms interval (throttled to avoid overloading the server). A stale-data indicator shows when the displayed data is older than 1 second.
- **On cell change**: Immediate fetch when the user clicks a different cell.

---

## 8. Snapshot Management

Current backend status: deferred/unimplemented. The Rust backend does **not** currently expose snapshot routes, so the frontend must not assume save/load/list/delete is available.

For the current backend, the snapshot panel should either be hidden or rendered in a disabled "Unavailable in current backend" state. The operations below describe the intended UI/API behavior once snapshot support lands.

### Operations

| Action | UI element | Target API call | Notes |
|--------|-----------|----------|-------|
| Save | "Save Snapshot" button + optional label text input | `POST /v1/sim/snapshot` with `{"label": "..."}` | Deferred in current backend; when implemented, button disabled when `simStatus === 'none'` |
| List | Auto-populated list below the save button | `GET /v1/sim/snapshots` | Refreshed on save, load, delete, and on tab focus |
| Load | "Load" button on each snapshot row | `POST /v1/sim/snapshot/:id/load` | Confirm dialog. Sim is paused after load in the target API-SPEC §13 design |
| Delete | "Delete" button on each snapshot row | `DELETE /v1/sim/snapshot/:id` | Confirm dialog |

### Snapshot list display

Each row shows:
- Label (or "Untitled" if no label)
- Tick number
- Timestamp (`created_at`, formatted as relative time)
- Size (human-readable bytes)

Sorted by `created_at` descending (newest first).

---

## 9. Config Editor

The config editor appears in the **Controls** tab when creating a new simulation. It is a form that produces the JSON body for `POST /v1/sim` (API-SPEC §8).

`r_energy` and `r_mass` are Poisson arrival means per cell per tick, so they are valid for any finite non-negative number. The four optional probability exponents accept integers in `0..=63`; empty input serializes as `null` and means never. Both required mutation exponent fields accept integers in `0..=63`. The editor displays the decoded `2^-k` probability beside every exponent field.

### Field groups

**Grid**:

| Field | Type | Default | Validation |
|-------|------|---------|------------|
| `width` | number | 256 | 1–1024, integer |
| `height` | number | 256 | 1–1024, integer |
| `seed` | number | random | 0–2^64 |

**Resource rates**:

| Field | Type | Default | Validation |
|-------|------|---------|------------|
| `r_energy` | number | 0.25 | ≥ 0.0 |
| `r_mass` | number | 0.05 | ≥ 0.0 |
| `d_energy_log2` | number or null | 7 | empty/null or integer 0–63 |
| `d_mass_log2` | number or null | 7 | empty/null or integer 0–63 |
| `t_cap` | number | 4.0 | > 0 |

**Program dynamics**:

| Field | Type | Default | Validation |
|-------|------|---------|------------|
| `maintenance_rate_log2` | number or null | 7 | empty/null or integer 0–63 |
| `maintenance_exponent` | number | 1.0 | > 0 |
| `local_action_exponent` | number | 1.0 | > 0 |
| `n_synth` | number | 1 | ≥ 0, integer |
| `inert_grace_ticks` | number | 10 | ≥ 0, integer |
| `p_spawn_log2` | number or null | null | empty/null or integer 0–63 |

**Mutation**:

| Field | Type | Default | Validation |
|-------|------|---------|------------|
| `mutation_base_log2` | number | 16 | 0–63, integer |
| `mutation_background_log2` | number | 8 | 0–63, integer |

**Seed programs** (MVP):

A list editor where each entry has:
- `x`, `y`: grid coordinates (validated against width/height)
- `code`: raw byte array, entered as comma-separated decimal values (e.g. `81, 83, 64, 0, 74, 66`)
- `free_energy`, `free_mass`: initial resources

A "Add seed program" button appends a new empty entry. A "Remove" button on each entry deletes it.

**Seed library**: ten built-in organisms in four structural families. *Classic* holds the three that predate the ladder — Turnchain (the spec Seed Replicator), Quadsorb (the 4×-absorb default), and Driftkin (the evolved 39-instruction glider) — none of which has a verified regime. The other seven are the verified ladder from `docs/analysis/2026-08-23_seed-organisms.md`: *Chain* (Radchain, Sunchain), *Synthesis* (Freesynth, Frontforge, Mixedforge), and *Mobile* (Squarestep, Wildstep). Each entry carries a description, provenance, an evidence note (its five-seed horizon result), and the parameter regime it was verified in; `free_energy`/`free_mass` are the root preloads those runs used. The entry cards show a derived library badge (matched on bytes) or "custom".

**Population composer**: a table of per-organism weights plus a total, grouped by family, with each row expanding in place to show the organism's description, evidence, regime, source, starting resources, and disassembly. The per-card library picker groups the same organisms into `<optgroup>`s. The total is split across the active organisms by largest-remainder apportionment, and the resulting counts are placed on distinct random free cells. Placement is live: every change to the mix, to `width`/`height`, to `seed`, or to the composer's own placement seed rewrites the entries the composer owns, so the population always matches what the table says and a grid shrink can never leave a stale out-of-range placement behind. Hand-placed entries are preserved and their cells excluded, and editing or removing a generated entry makes it hand-placed, so a later regeneration neither replaces nor resurrects it. Cells are drawn from the config `seed` folded to 32 bits, XOR the placement seed: the world seed moves the layout because the whole run moves with it, while "New" beside the placement seed moves only the layout. The same seeds, grid, counts, and hand-placed cells always give the same layout. If the request exceeds the free-cell count it is clamped proportionally with an inline warning. Composer state sits alongside the draft config in app state, never inside `SimConfig` — the submitted `seed_programs` list is unchanged in shape and remains the source of truth. Loading a saved config starts a fresh composer session, which stays out of the config until the user edits the mix; creating or resetting a simulation leaves what it had placed behind as plain hand-placed entries.

**Assembly input**: each entry has a mnemonic textarea alongside the raw-bytes input; editing either keeps both in sync. Assembly happens client-side and mirrors the engine's decode table, so inspector `disassembly` output pastes straight back in. Unknown tokens are reported inline with line, token, and near-miss suggestions, and the last valid bytes are retained. A collapsible opcode reference lists every mnemonic with a one-line summary.

**Later**: Import/export of seed sets, and user-saved organisms alongside the built-in library.

### Behavior

- Defaults are pre-populated from the values in API-SPEC §8.
- Exponent inputs are integer controls bounded to 0–63. The optional controls accept an empty value for never and show a live `2^-k` preview; required mutation controls cannot be empty.
- The form is only editable when `simStatus === 'none'` (config is immutable after creation, per API-SPEC §3).
- A "Create Simulation" button at the bottom submits the form. On success, `simStatus` transitions to `'created'`.
- Validation errors are shown inline per field. The API's error response (422 with `INVALID_CONFIG`) is displayed as a banner.
- A "Randomize Seed" button generates a new random seed value.

---

## 10. Layout and Navigation

### Layout sketch

```
+----------------------------------------------------------------------+
|                                                        +-----------+ |
|                                                        |  SIDEBAR  | |
|                    GRID CANVAS                         | 360px     | |
|                  (fills remaining space)               |           | |
|                                                        | [Controls]| |
|                  zoom / pan / click                    | [Inspect] | |
|                                                        |           | |
|                                                        | tab body  | |
|                                                        |           | |
|                                                        +-----------+ |
|+--------------------------------------------------------------------+|
|| STATUS BAR (48px): Tick | Pop | Energy | Mass | TPS | [ColorMap ▼] ||
||                                                       [Charts ↑]  ||
|+--------------------------------------------------------------------+|
+----------------------------------------------------------------------+
```

With metrics drawer expanded:

```
+----------------------------------------------------------------------+
|                                                        +-----------+ |
|                    GRID CANVAS                         |  SIDEBAR  | |
|                  (reduced height)                      |           | |
|                                                        |           | |
|                                                        +-----------+ |
|+--------------------------------------------------------------------+|
||                     METRICS DRAWER (50vh)                           ||
||  [Population] [Energy&Mass] [Births/Deaths] [Size] [Diversity]     ||
||  ┌──────────────────────────────────────────────────────────────┐   ||
||  │                    uPlot chart area                         │   ||
||  └──────────────────────────────────────────────────────────────┘   ||
|+--------------------------------------------------------------------+|
|| STATUS BAR                                            [Charts ↓]  ||
|+--------------------------------------------------------------------+|
+----------------------------------------------------------------------+
```

### Sizing rules

| Element | Size | Behavior |
|---------|------|----------|
| Grid canvas | Fills remaining space | Resizes with window. Minimum 400×300. |
| Sidebar | 360px fixed width | Collapsible (toggle button at top). When collapsed, grid expands to fill. |
| Status bar | 48px fixed height | Always visible at bottom. |
| Metrics drawer | 50% viewport height | Expands upward from status bar, pushing grid canvas smaller. |

### Sidebar tabs

Two tabs at the top of the sidebar:

- **Controls**: Lifecycle buttons, frame rate, metrics sampling, deferred snapshot panel, config editor
- **Inspector**: Cell inspector (empty state shows "Click a cell to inspect")

The active tab is stored in app state. Clicking a cell on the grid auto-switches to the Inspector tab.

### Color map selector

Located in the status bar. A dropdown or segmented button group showing the 8 color map modes (§4). Changing the selection immediately re-renders the grid with the new color mapping — no re-fetch needed, since all 8 modes read from the same CellView data already in memory.

---

## 11. Performance Budget

### Frame render latency targets

| Grid size | Cells | Canvas 2D target | WebGL target |
|-----------|-------|------------------|--------------|
| 128×128 | 16K | < 4ms | < 1ms |
| 256×256 | 64K | < 8ms | < 2ms |
| 512×512 | 256K | < 16ms | < 4ms |
| 1024×1024 | 1M | < 50ms (may drop below 30fps) | < 8ms |

These are per-frame render times for the color pass + draw. The 16.6ms budget (60fps) is the target. If Canvas 2D cannot maintain 30fps at the user's grid size, that is the trigger to implement the WebGL renderer.

### Memory budget

| Component | Budget |
|-----------|--------|
| Grid frame buffer (1024×1024) | 8 MB (1M × 8 bytes) |
| Offscreen canvas ImageData (1024×1024) | 4 MB (1M × 4 bytes RGBA) |
| Metrics rolling buffer (10K points × 12 series) | ~1 MB |
| uPlot chart instances (6 charts) | ~5 MB |
| Application overhead | ~10 MB |
| **Total** | **< 30 MB** |

### Bundle size budget

| Target | Size |
|--------|------|
| Initial JS bundle (gzipped) | < 200 KB |
| uPlot | ~35 KB (gzipped) |
| React + ReactDOM | ~45 KB (gzipped) |
| Application code | < 100 KB (gzipped) |

### WebGL upgrade trigger

Switch from Canvas 2D to WebGL when any of these conditions are met:

- Frame render time consistently exceeds 16ms at the user's grid size
- Grid sizes above 512×512 are common in practice
- Color map switching causes visible stutter

---

## 12. Stable Now vs Deferred

### MVP (build first)

| Feature | Dependency | Notes |
|---------|-----------|-------|
| Grid canvas (Canvas 2D) | — | Core visualization |
| Zoom / pan / click-to-inspect | Grid canvas | Essential interaction |
| All 8 color maps | Grid canvas | All modes from day one |
| WebSocket connection + reconnection | — | Binary frame + JSON metrics parsing |
| Lifecycle controls | REST endpoints | Create, start, pause, resume, step, reset, destroy |
| Config editor with defaults | `POST /v1/sim` | All fields from API-SPEC §8 |
| Cell inspector with disassembly | `GET /v1/sim/cell` | Auto-refresh on step |
| Status bar with key metrics | Metrics subscription | Tick, population, energy, mass, TPS |
| Expandable metrics charts | uPlot + metrics subscription | 6 chart definitions |
| Snapshot save/load/list/delete | REST endpoints | Minimal UI in controls tab |
| Frame rate control (max_fps) | Frame subscription | Slider 1–60 |
| Metrics sampling control (every_n_ticks) | Metrics subscription | Numeric input |
| Sidebar collapse/expand | — | Toggle button |

### Later (deferred)

| Feature | Dependency | Notes |
|---------|-----------|-------|
| WebGL 2 renderer | `GridRenderer` interface | Drop-in swap; implement when Canvas 2D hits performance limits |
| Minimap | Grid canvas | Small overview showing full grid with viewport rectangle |
| Smooth zoom animation | Zoom/pan | Animated transitions instead of instant zoom |
| 5×5 neighborhood inspector | `GET /v1/sim/cells` batch endpoint | Show neighbors around selected cell |
| Assembly input for seed programs | Config editor | Client-side assembler for mnemonics → bytecodes |
| Config presets | Config editor | Save/load named parameter sets |
| Keyboard shortcuts | — | Space=pause/resume, arrow keys=step, +/-=zoom |
| Derived viability metrics | Metrics dashboard | Computed ratios like energy-per-program |
| Spatial statistics charts | Metrics dashboard | Depends on API additions (API-SPEC §15 deferred) |
| Lineage / phylogeny visualization | — | Depends on API additions (API-SPEC §15 deferred) |
| Grid frame compression | WebSocket | Delta compression; depends on API additions |
| Export metrics as CSV | Metrics buffer | Download rolling buffer contents |

---

## 13. Open Questions / API Requests

These are items where the frontend needs clarification or additions from the backend/API:

### 1. Tick rate control

The current API has no server-side speed limiter. The simulation runs as fast as the CPU allows. For observation runs, the user may want to slow the simulation to a target TPS (e.g. 10 ticks/second) to watch behavior in real time.

**Request**: Consider adding `POST /v1/sim/speed` with a `target_tps` field (0 = unlimited). Alternatively, this could be a field on the `start`/`resume` response or a query parameter.

### 2. Initial frame on subscribe

When a client subscribes to frames (API-SPEC §11), does the server immediately push the current grid state, or only after the next tick completes? This matters when subscribing to a paused simulation — without an immediate push, the grid canvas would be blank until the user steps.

**Request**: Clarify behavior. If the server does not push an initial frame, the frontend will need to poll `GET /v1/sim/metrics` and render a blank grid until the first frame arrives. Immediate push on subscribe is preferred.

### 3. Config defaults endpoint

The frontend needs default values for config fields (API-SPEC §8) to pre-populate the config editor. Currently these are hardcoded from the spec.

**Request**: Consider adding `GET /v1/defaults` that returns the default config values. This would keep the frontend in sync if defaults change. Low priority — hardcoding from the spec is acceptable for MVP.

### 4. ~~CellView program_size scaling~~

**Resolved.** The linear `program_size / 128` byte was 0 for every program under 128 instructions, so the Program Size color map had no dynamic range in practice. The CellView byte is now log2-scaled:

```
byte = size == 0 ? 0 : min(255, round(17 * log2(size)))
```

17 steps per doubling: size 1 → 0, 2 → 17, 3 → 27, 4 → 34, 128 → 119, 1024 → 170, 32767 (the size cap) → 255. The frontend color map normalises by 170 (clamped to 1), so sizes 1..1024 span the blue→yellow ramp and larger programs saturate at the yellow end. Empty cells stay black.

### 5. program_id collision at scale

`program_id` is a u8 (0–255) in the CellView. For species-level coloring, collisions will occur as soon as more than 256 distinct lineages exist. This is acceptable for MVP since the Program ID color map is approximate, but it limits the usefulness of species visualization.

**Acknowledged**: This is a known limitation. No immediate API change needed, but worth revisiting if species tracking becomes important.

### 6. CORS headers

**Resolved.** The backend already applies `CorsLayer::permissive()` (Axum/Tower), which sets `Access-Control-Allow-Origin: *` and allows all methods and headers. No Vite proxy is needed.

### 7. TPS in metrics WebSocket message

The `ticks_per_second` field is available on `GET /v1/sim` (API-SPEC §7) but not in the WebSocket metrics message (API-SPEC §10). The status bar currently polls TPS via REST every 2 seconds.

**Request**: Consider adding `ticks_per_second` to the metrics WebSocket message to eliminate the polling need.

### 8. Simulation existence on connect

When the frontend loads, it needs to know whether a simulation already exists (e.g. if the page was refreshed). It calls `GET /v1/sim` — a 404 means no simulation exists, 200 means one does.

**Acknowledged**: This is already supported by the API. No change needed. Listed here for completeness of the frontend's startup sequence.

---

*This document is provisional and will be updated as the backend API and simulation engine mature.*
