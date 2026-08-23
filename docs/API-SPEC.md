# Proteus API Specification (Provisional)

**Status**: Provisional — subject to change as the engine implementation matures.

**Spec version**: 0.3.0

**Changed in 0.3.0**: the simulation config contract replaces floating probability fields with optional integer exponent fields `d_energy_log2`, `d_mass_log2`, `maintenance_rate_log2`, and `p_spawn_log2`. Integer `k` means probability `2^-k`; explicit `null` means never. Absent decay and maintenance fields default to 7, while absent `p_spawn_log2` defaults to `null` (§8). This is a breaking field rename.

**Added in 0.2.5**: metrics gained an optional point-in-time `census` object (program-size histograms, opcode census, size-1 population, aggregated lineage, stack depths), omitted from the WebSocket stream and available over REST via `GET /v1/sim/metrics?census=1`; cell inspection gained six lineage fields (`uid`, `parent_uid`, `birth_tick`, `generation`, `origin`, `created_tick`). Both are additive, so existing clients are unaffected (§10, §12).

**Simulator version**: Targets Proteus v0.4.0

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

All responses include the header `X-Proteus-API-Version: 0.3.0`.

The API is provisional and pre-1.0, so the breaking 0.3.0 config-field migration deliberately retains the `/v1` URL prefix. Once the API is stable, breaking changes increment the major URL version (`/v2`); additive changes do not.

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
| `r_energy` | f64 | Mean bg-radiation arrivals per cell per tick (`Poisson(r_energy)`) | 0.25 |
| `r_mass` | f64 | Mean bg-mass arrivals per cell per tick (`Poisson(r_mass)`) | 0.05 |
| `d_energy_log2` | u32 or null | Decay probability is `2^-k`; null means no energy decay | 7 |
| `d_mass_log2` | u32 or null | Decay probability is `2^-k`; null means no mass decay | 7 |
| `t_cap` | f64 | Free resource decay threshold multiplier on program size | 4.0 |
| `maintenance_rate_log2` | u32 or null | Maintenance probability is `2^-k`; null means no maintenance charge | 7 |
| `maintenance_exponent` | f64 | Beta: maintenance quanta = size^beta | 1.0 |
| `local_action_exponent` | f64 | Alpha: local action budget = max(1, floor(size^alpha)) | 1.0 |
| `n_synth` | u32 | Additional energy cost for synthesize | 1 |
| `inert_grace_ticks` | u32 | Ticks before abandoned inert pays maintenance | 10 |
| `p_spawn_log2` | u32 or null | Spawn probability is `2^-k`; null means never | null |
| `mutation_base_log2` | u32 | Baseline mutation rate = 2^(-value) | 16 |
| `mutation_background_log2` | u32 | Per-consumed-quantum mutation-trigger rate = 2^(-value) | 8 |

Each optional exponent accepts `null` or an integer in `0..=63`. JSON explicit `null` always means never. If absent, `d_energy_log2`, `d_mass_log2`, and `maintenance_rate_log2` default to 7; `p_spawn_log2` defaults to `null`. Responses always serialize these fields, including `null`. Both mutation exponent fields must be integers in `0..=63`. Values outside these domains are rejected when the simulation is created.

Optional initial-state fields:

| Field | Type | Description |
|-------|------|-------------|
| `seed_programs` | array | Programs to place at simulation start (see below) |
| `seed_environment` | array | Exact cell-resource preloads to apply at simulation start (see below) |

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

### Seed environment entry

```json
{
  "x": 9,
  "y": 10,
  "free_energy": 20,
  "free_mass": 12,
  "bg_radiation": 0,
  "bg_mass": 0
}
```

| Field | Type | Description |
|-------|------|-------------|
| `x` | u32 | Cell x coordinate |
| `y` | u32 | Cell y coordinate |
| `free_energy` | u32 | Exact initial free-energy pool |
| `free_mass` | u32 | Exact initial free-mass pool |
| `bg_radiation` | u32 | Exact initial background-radiation pool |
| `bg_mass` | u32 | Exact initial background-mass pool |

Environment entries are applied before seed programs. When both arrays target
the same cell, the program entry replaces `free_energy` and `free_mass`; the
environment entry's `bg_radiation` and `bg_mass` remain. Duplicate coordinates
within either array are invalid. Array order has no semantic effect.

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

Advance exactly `count` ticks (default 1) while not running. This works from both `created` and `paused`. If called from `created`, the simulation transitions to `paused` after the requested ticks complete. Returns `200 OK` with status after stepping. The response is sent after all requested ticks have completed. Fails with `409` if the simulation is running.

### Reset

```
POST /v1/sim/reset
```

Destroy the current simulation and recreate it with the same config. Equivalent to DELETE + POST with the original config. Tick counter resets to 0. Returns `200 OK` with the new status.

Reset also advances the metrics `epoch` and clears `event_totals`; see §10. This lets observers distinguish a reset from an ordinary sample at tick 0.

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

`every_n_ticks` controls sampling. Default 1 (every tick). Set higher to reduce volume. The stream is latest-value delivery: an observer can miss intermediate snapshots if the simulation advances faster than the connection can consume them. Current-state fields therefore describe the delivered tick, while cumulative `event_totals` preserve event counts across skipped or coalesced ticks.

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
  "epoch": 0,
  "tick": 104832,
  "population": 1847,
  "live_count": 1623,
  "inert_count": 224,
  "total_energy": 294011,
  "packet_energy": 237,
  "total_mass": 183722,
  "mean_program_size": 14.3,
  "max_program_size": 87,
  "unique_genomes": 412,
  "births": 12,
  "boot_births": 9,
  "spawn_births": 3,
  "deaths": 8,
  "mutations": 3,
  "event_totals": {
    "births": 89231,
    "boot_births": 64010,
    "spawn_births": 25221,
    "deaths": 87384,
    "mutations": 1439
  }
}
```

| Field | Type | Purpose | Stability |
|-------|------|---------|-----------|
| `epoch` | u64 | Metrics history generation; increments for each successful reset or new simulation in this server process | stable |
| `tick` | u64 | Current tick number | stable |
| `population` | u32 | Total programs (live + inert) | stable |
| `live_count` | u32 | Live programs | stable |
| `inert_count` | u32 | Inert programs | stable |
| `total_energy` | u64 | Sum of all free energy + bg radiation across grid | stable |
| `packet_energy` | u64 | Energy held in in-flight directed radiation packets (1 energy per packet) | stable |
| `total_mass` | u64 | Sum of all free mass + bg mass + program instructions across grid | stable |
| `mean_program_size` | f64 | Mean instruction count of live programs | stable |
| `max_program_size` | u32 | Largest live program | stable |
| `unique_genomes` | u32 | Distinct program bytecodes (live only) | stable |
| `births` | u32 | Programs that became live this tick via `boot` or spontaneous spawn | stable |
| `boot_births` | u32 | Programs that became live this tick via `boot` | stable |
| `spawn_births` | u32 | Programs that became live this tick via spontaneous spawn | stable |
| `deaths` | u32 | Programs destroyed (maintenance/decay) this tick | stable |
| `mutations` | u32 | Mutation events this tick | stable |
| `event_totals.births` | u64 | Cumulative live births in this epoch | stable |
| `event_totals.boot_births` | u64 | Cumulative births caused by `boot` in this epoch | stable |
| `event_totals.spawn_births` | u64 | Cumulative spontaneous spawn births in this epoch | stable |
| `event_totals.deaths` | u64 | Cumulative program deaths in this epoch | stable |
| `event_totals.mutations` | u64 | Cumulative mutation events in this epoch | stable |

The top-level `births`, `boot_births`, `spawn_births`, `deaths`, and `mutations` fields remain counts for the single delivered tick. `event_totals` count every completed tick since the start of the current epoch, independent of observation cadence. Programs placed by `seed_programs` during creation or reset are bootstrap state and are not births. The invariant `births = boot_births + spawn_births` holds for both the per-tick and cumulative fields.

To derive an event rate between two snapshots in the same epoch, clients divide the difference between cumulative totals by the difference in ticks. The first snapshot establishes a baseline. Clients discard out-of-order snapshots whose tick is older than the current baseline. They discard the baseline and begin a new series if `epoch` changes or any cumulative total regresses.

Epochs begin at 0 and increase monotonically for the lifetime of the server process. Both reset and destroy followed by create allocate a new epoch. Restarting the server process may restart the sequence at 0, so clients still treat any epoch change as a history boundary rather than comparing its absolute value across connections.

All `u64` values are encoded as JSON numbers. JavaScript clients can represent them exactly only through `2^53 - 1` (`Number.MAX_SAFE_INTEGER`); clients requiring longer exact histories must reject values above that limit until the API adopts a string or binary integer representation.

### Program census (optional)

An optional `census` object carries a point-in-time breakdown of the program population: size histograms, an opcode census, the size-1 population, aggregated lineage, and stack depths.

The census is **omitted from the WebSocket stream entirely**. The server recomputes metrics every tick, and the census costs far more than the rest of the snapshot on a dense grid, so it is opt-in and delivered only over REST:

```
GET /v1/sim/metrics?census=1
```

The flag accepts `1`/`0`, `true`/`false`, `yes`/`no`, and `on`/`off`. Without it the response has no `census` key at all. When present, the census describes the same tick as the snapshot it rides on.

```json
"census": {
  "live_sizes":  { "scale": "linear", "first_value": 1, "counts": [812, 40, "…256 entries…"],
                   "overflow": 0, "count": 15836, "sum": 1004002, "max": 154 },
  "inert_sizes": { "scale": "linear", "first_value": 1, "counts": ["…256 entries…"],
                   "overflow": 0, "count": 224, "sum": 903, "max": 12 },
  "opcode_counts": [4021, 0, "…256 entries, indexed by instruction byte…"],
  "size1": {
    "live_count": 812, "inert_count": 190,
    "live_age_sum": 913204, "live_max_age": 28417,
    "live_opcode_counts": ["…256 entries…"]
  },
  "lineage": {
    "roots": 41, "orphans": 15012,
    "generation": { "scale": "linear", "first_value": 0, "counts": ["…512 entries…"], "overflow": 3,
                    "count": 15836, "sum": 190032, "max": 37 },
    "birth_tick_sum": 152340000, "birth_tick_count": 15836,
    "offspring": { "scale": "linear", "first_value": 0, "counts": ["…16 entries…"], "overflow": 0,
                   "count": 15836, "sum": 15795, "max": 9 },
    "origin_seed": 1, "origin_spawn": 40, "origin_append": 15795
  },
  "stack_depths": { "scale": "log2", "first_value": 0, "counts": ["…33 entries…"],
                    "overflow": 0, "count": 16060, "sum": 4120, "max": 9 }
}
```

Every histogram uses the same shape and declares its bucketing in `scale`. `count`, `sum`, and `max` are always exact over all observations, whatever the scale and including any that overflow, so a mean or a maximum never depends on bucket layout.

**`"linear"`** — `counts[i]` counts the single value `first_value + i`, and `overflow` counts every observation above the last bucket. Used for `live_sizes` and `inert_sizes` (256 buckets from 1), `generation` (512 buckets from 0), and `offspring` (16 buckets from 0). A linear distribution can outrun its buckets: generation grows with run length, so treat `overflow` as a real population rather than an error.

**`"log2"`** — `counts[0]` counts the value 0, and `counts[i]` for `i >= 1` counts the half-open range `[2^(i-1), 2^i)`. The 33 buckets span every `u32`, so `overflow` is always 0; the field is retained so both scales share one wire shape, and `first_value` is unused and stays 0. Used for `stack_depths`, whose real distribution spans the entire stack cap (measured p50 759, p90 8315, max 32767 on a dense 256×256 run) and which no affordable linear width could describe.

Clients should switch on `scale` rather than assuming a layout. A histogram written before `scale` existed has no such key and is `"linear"`.

| Field | Type | Purpose | Stability |
|-------|------|---------|-----------|
| `census.live_sizes` | histogram | Instruction count per live program, `first_value` 1 | unstable |
| `census.inert_sizes` | histogram | Instruction count per inert program, `first_value` 1 | unstable |
| `census.opcode_counts` | u64[256] | Instruction bytes equal to each byte value, live programs only | unstable |
| `census.size1.live_count` | u32 | Live programs of exactly one instruction | unstable |
| `census.size1.inert_count` | u32 | Inert bodies of exactly one instruction | unstable |
| `census.size1.live_age_sum` | u64 | Summed age of live size-1 programs | unstable |
| `census.size1.live_max_age` | u32 | Oldest live size-1 program | unstable |
| `census.size1.live_opcode_counts` | u32[256] | Live size-1 programs by their single instruction | unstable |
| `census.lineage.roots` | u32 | Live programs with no parent (seed programs and spontaneous spawns) | unstable |
| `census.lineage.orphans` | u32 | Live programs whose parent is no longer live | unstable |
| `census.lineage.generation` | histogram | Generation over live programs, `first_value` 0 | unstable |
| `census.lineage.birth_tick_sum` | u64 | Summed birth tick over live programs that have one | unstable |
| `census.lineage.birth_tick_count` | u32 | Live programs contributing to `birth_tick_sum` | unstable |
| `census.lineage.offspring` | histogram | Live programs bucketed by how many live programs name them as parent | unstable |
| `census.lineage.origin_seed` | u32 | Live programs first created by bootstrap seeding | unstable |
| `census.lineage.origin_spawn` | u32 | Live programs first created by spontaneous spawn | unstable |
| `census.lineage.origin_append` | u32 | Live programs first created by `appendAdj` into an empty cell | unstable |
| `census.stack_depths` | histogram | Stack depth over live **and** inert programs, `log2` scale | unstable |

The census is strictly **point-in-time**, a gauge like `population` and `mean_program_size`. It has no cumulative counterpart and must never be diffed across samples the way `event_totals` are diffed: a census difference is not an event count. The losslessness guarantee that lets clients derive rates from `event_totals` applies to `event_totals` only.

### REST fallback

```
GET /v1/sim/metrics
```

Returns the latest metrics object as `200 OK`. It has the same fields and semantics as the WebSocket payload, except that it has no `"type"` discriminator, and that it accepts the optional `?census=1` flag described above. Useful for polling without a WebSocket connection.

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
| 2 | 1 | u8 | Program size, log2-scaled: `0` if empty, else `min(255, round(17 * log2(size)))` (17 steps per doubling; 0–255) |
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
    "abandonment_timer": null,
    "uid": 4295098371,
    "parent_uid": 4294983683,
    "birth_tick": 26214,
    "generation": 37,
    "origin": "append",
    "created_tick": 26212
  }
}
```

If the cell has no program, the `program` field is `null`.

`abandonment_timer` is present only for inert programs (null for live programs, null when no program).

The six lineage fields describe where the program came from. They are observation metadata: no simulation rule reads them and no instruction can observe them.

| Field | Type | Meaning |
|-------|------|---------|
| `uid` | u64 | Identifies this program uniquely within the current simulation instance |
| `parent_uid` | u64 or null | The program that created this one; `null` for a lineage root |
| `birth_tick` | u32 or null | Tick at which the program became live; `null` while an inert body has never booted |
| `generation` | u32 | 0 for a lineage root, otherwise the creator's generation plus one |
| `origin` | string or null | How the program first came into existence: `"seed"`, `"spawn"`, or `"append"` |
| `created_tick` | u64 or null | Tick at which the program first materialized, which for an inert body precedes `birth_tick` |

A program is a **lineage root** when it has no parent: either a bootstrap seed program (`origin` `"seed"`) or a spontaneous spawn (`origin` `"spawn"`). Both are the primordial bootstrap that `docs/SPEC.md` describes, and both carry `generation` 0 and `parent_uid` `null`. Their birth accounting differs: bootstrap seed programs are not births, while a spontaneous spawn increments `spawn_births` (and therefore `births`) exactly as the metrics section above describes. A program created by `appendAdj` into an empty cell (`origin` `"append"`) starts inert with `birth_tick` `null`, and keeps its `uid`, `parent_uid`, and `generation` unchanged when `boot` later makes it live.

`uid` values are **epoch-scoped**. They are never reused within one simulation instance below the encoding ceiling — uniqueness holds while `(tick × cell_count + cell_index) × 4` stays below 2^64 (≈ 2.8 × 10^14 ticks on a 128×128 grid); beyond it the engine panics rather than wrapping to a colliding uid. Independent of that ceiling, they are not comparable across simulations, across `POST /v1/sim/reset`, or across grids of different size, because a uid encodes the grid position and tick at which the program was created. `origin` and `created_tick` are decoded from the uid, so they carry the same scope.

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
| 422 | Config validation failure (e.g. width = 0, negative arrival rate, decay probability > 1) |
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

1. ~~**Frame CellView program_size scaling.**~~ Resolved: the byte is now log2-scaled, `min(255, round(17 * log2(size)))` with `0` for an empty cell. The former linear `size / 128` encoding was 0 for every program under 128 instructions, which covers nearly all early-simulation programs. At 17 steps per doubling, size 1 → 0, 2 → 17, 4 → 34, 128 → 119, 1024 → 170, and the 32767 size cap → 255.

2. **Snapshot storage.** When snapshot routes are implemented, the spec does not prescribe where snapshots are stored. File-based storage (one file per snapshot in a configured directory) is the likely implementation, but the API intentionally hides this. Should snapshots support export/import (download/upload raw snapshot data) for portability between server instances?

3. ~~**Future packet-energy visibility.**~~ Resolved: `packet_energy` is now a separate metric in §10. `total_energy` remains cell-local free energy + background radiation only.

4. **WebSocket reconnection semantics.** If a client's WebSocket connection drops and reconnects, should subscriptions be stateless (client must re-subscribe) or should the server remember subscription state by some client identifier? Stateless is simpler and recommended.

5. **Batch cell inspection vs. grid frames.** The `/v1/sim/cells` endpoint overlaps somewhat with grid frames — both provide spatial cell data. The distinction is precision (exact values vs. clamped rendering summary). Is this overlap acceptable, or should one be removed?
