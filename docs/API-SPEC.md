# Proteus API Specification (Provisional)

**Status**: Provisional — subject to change as the engine implementation matures.

**Spec version**: 0.1.0

**Simulator version**: Targets Proteus v0.2.0

---

## 1. Purpose and Status

This document defines the **external API contract** between the Proteus simulator backend and any frontend or external client. It specifies how a client creates, controls, and observes simulations, plus the deferred target design for persistence/snapshots.

This is a design document, not a generated API reference. It is provisional: fields and endpoints marked **stable** are unlikely to change; those marked **deferred** are placeholders that will be specified once the engine internals settle.

The API is independent of any specific frontend implementation.

---

## 2. Scope and Non-Goals

### In scope

- Simulation lifecycle (create, destroy)
- Configuration input
- Run controls (start, pause, resume, step, reset)
- Per-tick metrics output
- Grid frame streaming for visualization
- Cell and program inspection
- Snapshot save/load target design (deferred; not implemented in the current backend)
- Error model
- Versioning and compatibility

### Non-goals

- Internal Rust module boundaries
- Pass-internal debug APIs
- Raw queued-action or packet internals
- Exact binary snapshot encoding
- Mutation tracing or deep debug surfaces (deferred)
- Implementation-specific types not externally necessary

---

## 3. Design Principles

1. **Single session per server.** One simulation per process. No session multiplexing. Scale horizontally by running multiple processes.
2. **REST for control, WebSocket for streaming.** Request/response operations use HTTP. Continuous data (frames, metrics) uses a single WebSocket connection.
3. **Config is immutable after creation.** Simulation parameters are fixed at creation time. Changing parameters requires creating a new simulation. This preserves reproducibility: same seed + config always produces the same tick sequence.
4. **Latest-available frame delivery.** The server pushes the most recent frame at or below the client's requested max FPS. Frames are dropped if the sim outruns the client.
5. **Small, coherent surface.** Prefer fewer endpoints with clear semantics over a complete but sprawling API.
6. **Observation does not affect simulation.** No API call changes simulation state except explicit control operations.

---

## 4. API Versioning

All REST endpoints are prefixed with `/v1`.

All responses include the header `X-Proteus-API-Version: 0.1.0`.

Breaking changes increment the major URL version (`/v2`). Additive changes (new optional fields, new endpoints) do not.

WebSocket messages include an `api_version` field in the initial handshake.

---

## 5. Transport Model

### REST (HTTP/JSON)

Used for: simulation lifecycle, control, inspection, configuration, and the deferred snapshot-management design surface.

- Content-Type: `application/json` for request and response bodies.
- Standard HTTP status codes for errors.

### WebSocket

Used for: grid frame streaming, metrics streaming.

- Single endpoint: `GET /v1/ws` upgrades to WebSocket.
- Client sends JSON control messages to subscribe/unsubscribe.
- Server pushes binary frames (grid data) and JSON messages (metrics).
- The WebSocket connection is optional — a simulation runs without any connected client.

### Data flow

```
Client                          Server
  |                               |
  |-- POST /v1/sim (config) ----->|  create simulation
  |<---- 201 + status ------------|
  |                               |
  |-- POST /v1/sim/start -------->|  begin ticking
  |<---- 200 --------------------|
  |                               |
  |-- GET /v1/ws (upgrade) ------>|  open streaming
  |<==== WebSocket ===============|
  |---- subscribe frames -------->|
  |<--- binary frame -------------|  (repeating)
  |---- subscribe metrics ------->|
  |<--- JSON metrics -------------|  (repeating)
  |                               |
  |-- POST /v1/sim/pause -------->|  pause
  |-- GET /v1/sim/cell/42 ------->|  inspect while paused
  |-- POST /v1/sim/step?count=5 ->|  advance 5 ticks
  |                               |
```

---

## 6. Core Resources

The API exposes one primary resource: the **simulation**. Because there is one simulation per server, it is accessed at a fixed path rather than by ID.

| Resource | Description |
|----------|-------------|
| Simulation | The running (or idle) simulation instance |
| Cell | A single grid cell, addressed by index |
| Snapshot | Deferred target resource for saved simulation state; no snapshot routes are implemented in the current backend |

---

## 7. Session Lifecycle

### Create simulation

```
POST /v1/sim
```

Request body: a simulation config object (see §8).

Response `201 Created`:

```json
{
  "status": "created",
  "tick": 0,
  "grid_width": 256,
  "grid_height": 256,
  "config": { ... }
}
```

Fails with `409 Conflict` if a simulation already exists. The client must `DELETE /v1/sim` first.

### Get simulation status

```
GET /v1/sim
```

Response `200 OK`:

```json
{
  "status": "running" | "paused" | "created",
  "tick": 104832,
  "grid_width": 256,
  "grid_height": 256,
  "population": 1847,
  "total_energy": 294011,
  "total_mass": 183722,
  "ticks_per_second": 412.7
}
```

Returns `404` if no simulation exists.

### Destroy simulation

```
DELETE /v1/sim
```

Stops the simulation and releases all resources. Returns `204 No Content`. Any connected WebSocket clients receive a close frame.

Returns `404` if no simulation exists.

---

## 8. Simulation Config Schema

Provided as the request body to `POST /v1/sim`. All fields are required unless marked optional.

| Field | Type | Description | Default |
|-------|------|-------------|---------|
| `width` | u32 | Grid width in cells | *required* |
| `height` | u32 | Grid height in cells | *required* |
| `seed` | u64 | Master RNG seed | *required* |
| `r_energy` | f64 | P(cell receives 1 bg radiation per tick) | 0.25 |
| `r_mass` | f64 | P(cell receives 1 bg mass per tick) | 0.05 |
| `d_energy` | f64 | P(each bg radiation / excess free energy unit decays per tick) | 0.01 |
| `d_mass` | f64 | P(each bg mass / excess free mass unit decays per tick) | 0.01 |
| `t_cap` | f64 | Free resource decay threshold multiplier on program size | 4.0 |
| `maintenance_rate` | f64 | P(each maintenance quantum costs 1 per tick) | 0.0078125 |
| `maintenance_exponent` | f64 | Beta: maintenance quanta = size^beta | 1.0 |
| `local_action_exponent` | f64 | Alpha: local action budget = max(1, floor(size^alpha)) | 1.0 |
| `n_synth` | u32 | Additional energy cost for synthesize | 1 |
| `inert_grace_ticks` | u32 | Ticks before abandoned inert pays maintenance | 10 |
| `p_spawn` | f64 | P(spontaneous creation in eligible empty cell) | 0.0 |
| `mutation_base_log2` | u32 | Baseline mutation rate = 2^(-value) | 16 |
| `mutation_background_log2` | u32 | Bg-stressed mutation rate divisor | 8 |

Optional initial-state fields:

| Field | Type | Description |
|-------|------|-------------|
| `seed_programs` | array | Programs to place at simulation start (see below) |

### Seed program entry

```json
{
  "x": 10,
  "y": 10,
  "code": [81, 83, 64, 0, 74, 66, 48, 85, 95, 49, 100, 80],
  "free_energy": 20,
  "free_mass": 12
}
```

| Field | Type | Description |
|-------|------|-------------|
| `x` | u32 | Cell x coordinate |
| `y` | u32 | Cell y coordinate |
| `code` | u8[] | Program bytecode |
| `free_energy` | u32 | Initial free energy in the cell |
| `free_mass` | u32 | Initial free mass in the cell |

The program is placed as live with `IP = 0`, an empty stack, and default registers. As in the
master simulation spec, default `Dir` and `ID` initialization is randomized at program creation.

### Read config

```
GET /v1/sim/config
```

Returns the config the simulation was created with. `200 OK` with the same schema as the creation request. `404` if no simulation exists.

---

## 9. Control Operations

All control endpoints return `404` if no simulation exists.

### Start

```
POST /v1/sim/start
```

Begin ticking from the `created` state. Returns `200 OK` with the current status. Fails with `409` if already running.

### Pause

```
POST /v1/sim/pause
```

Pause the tick loop after the current tick completes. Returns `200 OK` with the current status. Idempotent if already paused.

### Resume

```
POST /v1/sim/resume
```

Resume from paused state. Returns `200 OK`. Fails with `409` if not paused.

### Step

```
POST /v1/sim/step?count=1
```

Advance exactly `count` ticks (default 1) while paused. Returns `200 OK` with status after stepping. The response is sent after all requested ticks have completed. Fails with `409` if the simulation is running (not paused).

### Reset

```
POST /v1/sim/reset
```

Destroy the current simulation and recreate it with the same config. Equivalent to DELETE + POST with the original config. Tick counter resets to 0. Returns `200 OK` with the new status.

---

## 10. Metrics Schema

Metrics are delivered via the WebSocket `metrics` subscription channel.

### Subscription

Client sends:

```json
{
  "subscribe": "metrics",
  "every_n_ticks": 1
}
```

`every_n_ticks` controls sampling. Default 1 (every tick). Set higher to reduce volume.

Client sends to stop:

```json
{
  "unsubscribe": "metrics"
}
```

### Metrics message

Server pushes JSON:

```json
{
  "type": "metrics",
  "tick": 104832,
  "population": 1847,
  "live_count": 1623,
  "inert_count": 224,
  "total_energy": 294011,
  "total_mass": 183722,
  "mean_program_size": 14.3,
  "max_program_size": 87,
  "unique_genomes": 412,
  "births": 12,
  "deaths": 8,
  "mutations": 3
}
```

| Field | Type | Purpose | Stability |
|-------|------|---------|-----------|
| `tick` | u64 | Current tick number | stable |
| `population` | u32 | Total programs (live + inert) | stable |
| `live_count` | u32 | Live programs | stable |
| `inert_count` | u32 | Inert programs | stable |
| `total_energy` | u64 | Sum of all free energy + bg radiation across grid | stable |
| `total_mass` | u64 | Sum of all free mass + bg mass + program instructions across grid | stable |
| `mean_program_size` | f64 | Mean instruction count of live programs | stable |
| `max_program_size` | u32 | Largest live program | stable |
| `unique_genomes` | u32 | Distinct program bytecodes (live only) | stable |
| `births` | u32 | Programs that became live this tick via `boot` or spontaneous spawn | stable |
| `deaths` | u32 | Programs destroyed (maintenance/decay) this tick | stable |
| `mutations` | u32 | Mutation events this tick | stable |

### REST fallback

```
GET /v1/sim/metrics
```

Returns the latest metrics object as `200 OK`. Useful for polling without a WebSocket connection.

---

## 11. Grid Frame / Streaming Schema

Grid frames are delivered via the WebSocket `frames` subscription channel as binary messages.

### Subscription

Client sends:

```json
{
  "subscribe": "frames",
  "max_fps": 30
}
```

The server delivers the latest available frame at or below `max_fps`. If the simulation is running faster than `max_fps`, intermediate frames are dropped. `max_fps` must be between 1 and 60.

Client sends to stop:

```json
{
  "unsubscribe": "frames"
}
```

### Frame binary format

Each binary WebSocket message is a grid frame. Layout:

| Offset | Size | Type | Description |
|--------|------|------|-------------|
| 0 | 8 | u64 LE | Tick number |
| 8 | 4 | u32 LE | Grid width |
| 12 | 4 | u32 LE | Grid height |
| 16 | N × `cell_size` | [CellView] | Row-major cell data |

Each `CellView` is 8 bytes:

| Offset | Size | Type | Description |
|--------|------|------|-------------|
| 0 | 1 | u8 | Flags: bit 0 = has_program, bit 1 = is_live, bit 2 = is_open |
| 1 | 1 | u8 | Program ID (0 if empty) |
| 2 | 1 | u8 | Program size / 128 (scaled, 0–255; 0 if empty) |
| 3 | 1 | u8 | Free energy (clamped to 255) |
| 4 | 1 | u8 | Free mass (clamped to 255) |
| 5 | 1 | u8 | Background radiation (clamped to 255) |
| 6 | 1 | u8 | Background mass (clamped to 255) |
| 7 | 1 | u8 | Reserved (0) |

Total frame size: 16 + (width × height × 8) bytes. A 256×256 grid produces ~512 KB frames.

**Design note**: `CellView` is a lossy rendering summary, not a precise simulation readout. Resource values are clamped to u8 and program size is scaled. Use the inspection endpoint for exact values.

---

## 12. Inspection Schema

### Inspect a cell

```
GET /v1/sim/cell/:index
```

Where `:index` is the flat cell index (row-major: `y * width + x`). Also accepts query parameters: `GET /v1/sim/cell?x=10&y=15`.

Response `200 OK`:

```json
{
  "index": 2570,
  "x": 10,
  "y": 15,
  "free_energy": 42,
  "free_mass": 7,
  "bg_radiation": 3,
  "bg_mass": 1,
  "program": {
    "code": [81, 83, 64, 0, 74, 66, 48, 85, 95, 49, 100, 80],
    "disassembly": [
      "absorb", "collect", "cw", "push 0", "setSrc", "getSize",
      "for", "read", "appendAdj", "next", "boot", "nop"
    ],
    "size": 12,
    "live": true,
    "age": 847,
    "ip": 7,
    "src": 8,
    "dst": 0,
    "dir": 1,
    "flag": false,
    "msg": 0,
    "id": 3,
    "lc": 4,
    "stack": [12, 0, 7],
    "abandonment_timer": null
  }
}
```

If the cell has no program, the `program` field is `null`.

`abandonment_timer` is present only for inert programs (null for live programs, null when no program).

`dir` uses the master `Dir` encoding from `docs/SPEC.md`: `0 = right`, `1 = up`, `2 = left`, `3 = down`.

`flag` is serialized as a JSON boolean.

Returns `400` if the requested index or coordinates are out of bounds. Returns `404` if no simulation exists.

### Inspect a region (batch)

```
GET /v1/sim/cells?x=10&y=10&w=5&h=5
```

Returns an array of cell objects for the rectangular region. Same schema as single-cell inspection. Useful for inspector panels that show a neighborhood.

Maximum region size: 100 cells (w × h ≤ 100). Returns `400` if exceeded or if the requested rectangle extends outside the grid. Returns `404` if no simulation exists.

---

## 13. Snapshot Operations (Deferred)

Current backend status: deferred/unimplemented. The Rust backend does **not** currently expose any `/v1/sim/snapshot*` routes. The routes below describe the intended product surface once the backend has a settled, non-speculative snapshot boundary.

In that target design, snapshots are product-level save points — opaque to the client. The server manages storage. A snapshot captures the full simulation state: grid, config, tick number, and all internal state needed for deterministic resumption.

### Save snapshot (target design)

```
POST /v1/sim/snapshot
```

Optional request body:

```json
{
  "label": "interesting divergence at tick 10k"
}
```

When implemented, response `201 Created`:

```json
{
  "id": "snap_01J5X...",
  "tick": 104832,
  "label": "interesting divergence at tick 10k",
  "created_at": "2026-03-15T14:30:00Z",
  "size_bytes": 2097152
}
```

### List snapshots (target design)

```
GET /v1/sim/snapshots
```

When implemented, response `200 OK`:

```json
{
  "snapshots": [
    {
      "id": "snap_01J5X...",
      "tick": 104832,
      "label": "interesting divergence at tick 10k",
      "created_at": "2026-03-15T14:30:00Z",
      "size_bytes": 2097152
    }
  ]
}
```

### Load snapshot (target design)

```
POST /v1/sim/snapshot/:id/load
```

When implemented, this replaces the current simulation state with the snapshot. The simulation is paused after loading. The route returns `200 OK` with the simulation status at the restored tick. Any active WebSocket subscribers receive an updated frame/metrics.

When implemented, returns `404` if the snapshot ID is not found.

### Delete snapshot (target design)

```
DELETE /v1/sim/snapshot/:id
```

When implemented, returns `204 No Content`. Returns `404` if not found.

---

## 14. Error Model

### HTTP errors

All error responses use a consistent JSON body:

```json
{
  "error": {
    "code": "SIM_ALREADY_EXISTS",
    "message": "A simulation already exists. DELETE /v1/sim first.",
    "status": 409
  }
}
```

| HTTP Status | When |
|-------------|------|
| 400 | Out-of-bounds inspection coordinates/index, malformed request, or other invalid request input |
| 404 | No simulation exists |
| 409 | Conflict (sim already exists, wrong state for operation) |
| 422 | Config validation failure (e.g. width = 0, negative probability) |
| 500 | Internal server error |

### Error codes

| Code | Meaning |
|------|---------|
| `NO_SIM` | No simulation exists |
| `SIM_ALREADY_EXISTS` | Simulation already exists |
| `SIM_NOT_RUNNING` | Operation requires running state |
| `SIM_NOT_PAUSED` | Operation requires paused state |
| `SIM_NOT_CREATED` | Operation requires created state |
| `INVALID_CONFIG` | Config validation failure |
| `CELL_OUT_OF_BOUNDS` | Cell index or coordinates exceed grid size |
| `REGION_TOO_LARGE` | Batch inspection region exceeds limit |
| `BAD_REQUEST` | Malformed or semantically invalid request input |
| `INTERNAL_ERROR` | Unexpected server error |

Deferred snapshot routes are expected to add:

| Code | Meaning |
|------|---------|
| `SNAPSHOT_NOT_FOUND` | Snapshot ID not found |

### WebSocket errors

Errors on the WebSocket are sent as JSON messages:

```json
{
  "type": "error",
  "code": "INVALID_SUBSCRIPTION",
  "message": "Unknown channel: 'foo'"
}
```

Fatal errors close the connection with an appropriate WebSocket close code.

---

## 15. Stable Now vs Deferred

### Stable

These are specified and unlikely to change:

- Simulation lifecycle (create, destroy, status)
- Config schema (mirrors spec system parameters)
- Control operations (start, pause, resume, step, reset)
- Core metrics fields (population, energy, mass, births, deaths)
- Cell inspection (full program state + disassembly)
- Error model structure
- Transport split (REST + WS)

### Deferred

These are acknowledged but not yet specified:

| Surface | Reason |
|---------|--------|
| Runtime config mutation | Deferred until reproducibility implications are understood |
| Every-N-ticks frame delivery mode | Additive; will be specified when analysis use cases require it |
| Mutation tracing / lineage tracking | Depends on engine internal tracing infrastructure |
| Spatial statistics in metrics | Depends on what proves useful in practice |
| Top-N genotype ranking in metrics | Depends on genome hashing/comparison implementation |
| Binary metrics format | JSON is sufficient until bandwidth becomes an issue |
| Authentication / authorization | Not needed for localhost; will be specified if the server is exposed |
| Multi-client coordination | Single-session model makes this mostly moot; deferred |
| Replay / deterministic playback API | Requires tick-level recording; deferred |
| Grid frame compression | The 8-byte CellView is already compact; delta compression deferred |
| Debug/pass-level inspection | Internal to engine; not part of the external API |
| Snapshot save/load/list/delete | Deferred until the backend exposes a settled snapshot boundary |

---

## 16. Open Questions

1. **Frame CellView program_size scaling.** The current spec scales program size to fit u8 (size / 128, capped at 255). With the spec's size cap of 32767, this gives ~128-instruction resolution. Is this sufficient for visualization, or should the scaling factor be configurable?

2. **Snapshot storage.** When snapshot routes are implemented, the spec does not prescribe where snapshots are stored. File-based storage (one file per snapshot in a configured directory) is the likely implementation, but the API intentionally hides this. Should snapshots support export/import (download/upload raw snapshot data) for portability between server instances?

3. **Future packet-energy visibility.** `total_energy` intentionally means free energy + background radiation across the grid; it does **not** include in-flight directed-radiation packets. If packet visibility is needed later, it should be added as a separate metric rather than by redefining `total_energy`.

4. **WebSocket reconnection semantics.** If a client's WebSocket connection drops and reconnects, should subscriptions be stateless (client must re-subscribe) or should the server remember subscription state by some client identifier? Stateless is simpler and recommended.

5. **Batch cell inspection vs. grid frames.** The `/v1/sim/cells` endpoint overlaps somewhat with grid frames — both provide spatial cell data. The distinction is precision (exact values vs. clamped rendering summary). Is this overlap acceptable, or should one be removed?
