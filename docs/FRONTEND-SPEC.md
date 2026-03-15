# Proteus Frontend Specification (Provisional)

**Status**: Provisional — subject to change as the backend API matures.

**Spec version**: 0.1.0

**Targets**: Proteus v0.2.0, API-SPEC v0.1.0

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
5. **Reconnect**: On close or error, attempt reconnection with exponential backoff (1s, 2s, 4s, 8s, max 30s). Re-subscribe on reconnect. Subscriptions are stateless per API-SPEC §16 Q4.
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
| 2 | program_size / 128 | 0–255 |
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
| Program Size | byte 2 | Linear blue→yellow ramp. Empty → black |
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

A numeric input sets `every_n_ticks` for the metrics subscription. Changing it sends `unsubscribe` + re-`subscribe` on the WebSocket. Default: 1. Higher values reduce WebSocket traffic during long observation runs.

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
| Births / Deaths / Mutations | `births`, `deaths`, `mutations` | Per-tick count; `births` aggregates both `boot` and spontaneous spawn |
| Program Size | `mean_program_size`, `max_program_size` | Instructions (dual axis) |
| Diversity | `unique_genomes` | Count |

### Rolling buffer

Metrics history is stored in typed arrays (one `Float64Array` per series) with a rolling window of 10,000 points. This gives uPlot a fixed-size data source and bounds memory usage. When the buffer is full, old points are evicted in FIFO order.

The x-axis is `tick` (not wall-clock time) so charts remain meaningful across pauses.

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
| `r_energy` | number | 0.25 | 0.0–1.0 |
| `r_mass` | number | 0.05 | 0.0–1.0 |
| `d_energy` | number | 0.01 | 0.0–1.0 |
| `d_mass` | number | 0.01 | 0.0–1.0 |
| `t_cap` | number | 4.0 | > 0 |

**Program dynamics**:

| Field | Type | Default | Validation |
|-------|------|---------|------------|
| `maintenance_rate` | number | 0.0078125 | 0.0–1.0 |
| `maintenance_exponent` | number | 1.0 | > 0 |
| `local_action_exponent` | number | 1.0 | > 0 |
| `n_synth` | number | 1 | ≥ 0, integer |
| `inert_grace_ticks` | number | 10 | ≥ 0, integer |
| `p_spawn` | number | 0.0 | 0.0–1.0 |

**Mutation**:

| Field | Type | Default | Validation |
|-------|------|---------|------------|
| `mutation_base_log2` | number | 16 | ≥ 0, integer |
| `mutation_background_log2` | number | 8 | ≥ 0, integer |

**Seed programs** (MVP):

A list editor where each entry has:
- `x`, `y`: grid coordinates (validated against width/height)
- `code`: raw byte array, entered as comma-separated decimal values (e.g. `81, 83, 64, 0, 74, 66`)
- `free_energy`, `free_mass`: initial resources

A "Add seed program" button appends a new empty entry. A "Remove" button on each entry deletes it.

**Later**: Assembly-language input for seed programs (enter mnemonics instead of raw bytes), with client-side assembly.

### Behavior

- Defaults are pre-populated from the values in API-SPEC §8.
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
| uPlot chart instances (5 charts) | ~5 MB |
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
| Expandable metrics charts | uPlot + metrics subscription | 5 chart definitions |
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

### 4. CellView program_size scaling

API-SPEC §11 specifies `program_size / 128` for the CellView byte. With the spec's size cap of 32,767, this gives ~128-instruction resolution. However, most programs in early simulation are small (< 128 instructions), which means the size byte is 0 for nearly all programs, making the Program Size color map useless in practice.

**Request**: Consider changing the scaling to `size / 4` (0–255 maps to 0–1020, with clamping above) or a nonlinear mapping like `min(255, floor(sqrt(size) * 8))`. This would give useful resolution for small programs while still distinguishing large ones.

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
