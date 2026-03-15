# Proteus Backend Implementation Guidelines

Practical guidelines for implementing the Proteus simulator in Rust. Not a design doc — a reference for keeping implementation decisions consistent as the codebase grows.

## Stack

- **Language**: Rust (stable)
- **Parallelism**: Rayon for data parallelism across cells
- **RNG**: `fastrand` (Wyrand) for per-cell draws, with splitmix64 seed derivation for reproducibility
- **Serialization**: `serde` for configs and snapshots
- **Output formats**: CSV or JSONL for metrics, bincode or MessagePack for grid snapshots
- **Web frontend** (when ready): `axum` + `tokio-tungstenite` for WebSocket streaming

No ECS, no game engine, no framework. Plain data structures and functions.

## Grid layout

Flat `Vec<Cell>` with index arithmetic. Row-major: `index = y * width + x`. Toroidal wrapping handled by a small `Grid` struct that resolves neighbor indices.

```
grid.neighbor(index, dir) -> usize   // wraps at boundaries
grid.x(index) -> u32
grid.y(index) -> u32
grid.index(x, y) -> usize
```

Keep the `Grid` struct thin — it owns the `Vec<Cell>` and handles coordinate math. All pass logic lives in free functions or a separate `Tick` module that borrows the grid.

Do not use a 2D array crate. You want explicit control over memory layout and iteration order, and you want Rayon's `par_chunks_mut` over a flat slice.

## Cell struct

Start simple. One struct per cell. Split into hot/cold only if profiling shows cache pressure.

```rust
struct Cell {
    // Program state (hot in Pass 1)
    program: Option<Program>,

    // Resource pools (hot in Pass 3)
    free_energy: u32,
    free_mass: u32,
    bg_radiation: u32,
    bg_mass: u32,
}

struct Program {
    code: Vec<u8>,
    registers: Registers,
    stack: Vec<i16>,

    // Lifecycle
    live: bool,
    age: u32,
    abandonment_timer: u32,

    // Transient per-tick state (reset each tick)
    tick: TickState,
}
```

Keep `TickState` as a flat struct inside `Program`, not a separate allocation. It's reset every tick and accessed alongside the program during Pass 1.

```rust
struct TickState {
    absorb_count: u8,
    absorb_dir: Option<Dir>,
    did_listen: bool,
    did_collect: bool,
    did_nop: bool,
    is_open: bool,
    bg_radiation_consumed: u32,  // total bg radiation used for base costs (drives mutation rate)
    is_newborn: bool,
}
```

## Program storage

Start with `Vec<u8>` per program. This is simple, correct, and fast enough.

If profiling later shows that memory is tight at scale or that cache performance suffers, consider a deduplicated program pool (programs stored once, cells hold an index + diff). This optimization mirrors the low-diversity property of evolved populations but adds complexity — don't do it preemptively.

The program size cap (2^15 - 1) means a `u16` is sufficient for IP, Src, Dst, and stack values. The code itself is `u8` per instruction.

## Directed radiation packets

Directed radiation packets live in a global `Vec<Packet>`, not per-cell. Each packet carries a position, direction, and message value; energy is implicit (1 per packet).

```rust
struct Packet {
    position: usize,   // cell index
    direction: Dir,
    message: i16,
}
```

The simulation state owns the vec and reuses it across ticks with `clear()` + refill. `emit` (local, Pass 1) appends new packets to a thread-local vec that Rayon merges, same pattern as `QueuedAction`. Pass 3 steps 1–3 (propagation, listening, collision) consume and update this vec.

## Snapshot strategy

Pass 0 snapshots only the fields that Pass 1 sensing instructions read: free energy, free mass, bg radiation, bg mass, program size, program ID. This is a small struct per cell.

```rust
struct CellSnapshot {
    free_energy: u32,
    free_mass: u32,
    bg_radiation: u32,
    bg_mass: u32,
    program_size: u16,
    program_id: u8,
    has_program: bool,
}
```

Allocate the snapshot vec once and rewrite it each tick. Do not reallocate.

Lazy snapshotting (only snapshot cells adjacent to occupied cells) is a valid optimization but adds bookkeeping. Start with full-grid snapshot; it's a single `par_iter` memcpy-like pass and likely not a bottleneck.

## Pass architecture

Each pass is a function that borrows the grid (and snapshot where needed). Keep them visually and structurally separate — this is the most important architectural invariant.

### Pass 0

```rust
fn pass0_snapshot(grid: &Grid) -> Vec<CellSnapshot>
```

Embarrassingly parallel. `par_iter` over cells, write snapshot. Also record `live_at_tick_start` as a `BitVec` or a `Vec<bool>` indexed by cell index.

### Pass 1

```rust
fn pass1_local(grid: &mut Grid, snapshot: &[CellSnapshot], live_set: &[bool]) -> Vec<QueuedAction>
```

Embarrassingly parallel over cells. Each cell reads only its own mutable state and the immutable snapshot for neighbor sensing. Returns a collected vec of queued nonlocal actions.

Use `par_chunks_mut` on the cell slice. Each chunk processes its cells independently, collecting `QueuedAction`s into a thread-local vec. Rayon merges them afterward.

The inner loop is opcode dispatch. A `match` on the instruction byte is fine — the compiler will generate a jump table. Do not try to optimize this with function pointer tables or computed gotos; the `match` is idiomatic, readable, and the compiler handles it.

### Pass 2

```rust
fn pass2_nonlocal(grid: &mut Grid, actions: Vec<QueuedAction>)
```

Group actions by target cell index. Sort the action vec by target index (or use a `HashMap<usize, SmallVec<[QueuedAction; 2]>>`). Process each target group independently.

Resolve in class order: read-only, then additive transfers, then exclusive. Within each class, the spec guarantees order-independence, so no further sorting needed.

Exclusive-class special cases to handle:

- **Boot multi-success**: if the only exclusive-class instructions targeting a cell are `boot`s and the target is inert, all of them succeed — no conflict resolution needed.
- **`appendAdj` into empty cell**: creates a new inert program with default registers (Dir and ID randomized), empty stack, age 0, abandonment timer 0. Protection check is skipped (empty cells are always open).
- **`delAdj` additional cost**: the additional energy cost equals the target's **strength** (`min(target_size, target_free_energy)` in pre-Pass-2 state), paid by the source only on success. This can be expensive.
- **Additional costs generally**: base costs were paid in Pass 1 and are never refunded. Additional costs (mass for `appendAdj`, energy for `delAdj` and `synthesize`) are paid only on success from working pools, never from background pools. If the tentative winner can't pay, the instruction fails with no fallback winner.

This pass is parallel across target cells but the number of target cells with actions is typically small. Parallelism may not help here; profile before reaching for Rayon. A simple sequential loop over target groups is probably fine for a long time.

### Pass 3

```rust
fn pass3_physics(grid: &mut Grid, live_set: &[bool])
```

Follow the spec's sub-step ordering (radiation propagation, listening, collision, absorb resolution, bg radiation decay/arrival, collect, bg mass decay/arrival, inert lifecycle, maintenance, free-resource decay, age update, spontaneous creation).

Most sub-steps are per-cell and parallel. Two exceptions need care:

- **Directed radiation propagation** (step 1): packets move between cells. Use the global `Vec<Packet>` — update each packet's position, then process listening/collision per-cell.
- **Absorb resolution** (step 4): this is a many-to-many distribution. Each cell's background radiation is split (floor division, remainder stays) among all programs whose absorb footprint includes that cell. A program's footprint can cover up to 5 cells, and multiple programs can overlap on the same cell. Build a mapping of cell → list of absorbing programs, then distribute.

For stochastic draws, derive each cell's Wyrand RNG from `(master_seed, tick, cell_index)` — see the RNG section below. Do not store RNG state in cells or share RNG state across threads.

### Mutation (end of tick)

After Pass 3's 12 sub-steps, run mutation for every program that was live at tick start. This is logically step 13 of the tick.

Each eligible program mutates with probability based on whether it consumed background radiation for base-cost payment this tick:

- **Normal**: `2^(-mutation_base_log2)` per tick.
- **Background-stressed**: `min(x / 2^(mutation_background_log2), 1)` where `x` is `bg_radiation_consumed` from `TickState`.

If triggered, pick one instruction uniformly at random, flip one random bit in its 8-bit opcode. Mutation does not affect the current tick.

This is embarrassingly parallel (per-program, no inter-cell communication).

## RNG and reproducibility

Reproducibility is non-negotiable. Same seed, same config, same tick sequence — always.

### Per-tick derivation, not persistent per-cell state

Do not store RNG state in cells. Persistent per-cell RNG creates problems: `move` makes ownership ambiguous (does the RNG travel with the program or stay with the cell?), cell death orphans RNG state, cell creation requires order-dependent seeding that breaks parallel reproducibility, and checkpoint/restore has to serialize every cell's RNG.

Instead, derive each cell's RNG as a pure function of `(master_seed, tick, cell_index)` each tick. No state to move, serialize, or worry about.

### Seed derivation

Use splitmix64 to mix the three inputs before seeding a Wyrand RNG. Raw XOR of seed, tick, and index has weak avalanche — nearby cells in nearby ticks can get correlated streams.

```rust
fn splitmix64(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9e3779b97f4a7c15);
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d049bb133111eb);
    x ^ (x >> 31)
}

fn cell_rng(master_seed: u64, tick: u64, cell_index: u64) -> fastrand::Rng {
    let mixed = splitmix64(
        master_seed
            .wrapping_add(tick.wrapping_mul(0x517cc1b727220a95))
            .wrapping_add(cell_index.wrapping_mul(0x6c62272e07bb0142))
    );
    fastrand::Rng::with_seed(mixed)
}
```

Cost is ~3-4ns per cell — noise compared to Pass 1 VM execution.

### Wyrand via fastrand

Use `fastrand` (Wyrand) for all per-cell draws. It's ~1ns per call, the state is a single `u64`, and statistical quality is more than sufficient for simulation stochastics.

If you need `rand_distr::Binomial` or other distributions from the `rand` ecosystem, write a thin trait wrapper around `fastrand::Rng`. Alternatively, for the parameter ranges typical in this sim (n up to a few hundred, p typically small), a direct loop of Bernoulli trials is competitive with BTPE and avoids the dependency. Benchmark both.

### Binomial sampling

Binomial draws come up constantly: decay, maintenance, background input — per cell per tick. For small n (most cases), a direct Bernoulli trial loop is fine:

```rust
fn binomial(rng: &mut fastrand::Rng, n: u32, p: f64) -> u32 {
    // For small n, direct trials. For large n, consider BTPE.
    (0..n).filter(|_| rng.f64() < p).count() as u32
}
```

For large n (energy/mass pools in the hundreds), this gets slow. Add a BTPE or normal-approximation fast path if profiling shows binomial sampling as a bottleneck.

## Queued actions

```rust
enum QueuedAction {
    ReadAdj { source: usize, target: usize, src_cursor: u16 },
    WriteAdj { source: usize, target: usize, value: u8, dst_cursor: u16 },
    AppendAdj { source: usize, target: usize, value: u8 },
    DelAdj { source: usize, target: usize, dst_cursor: u16 },
    GiveE { source: usize, target: usize, amount: i16 },
    GiveM { source: usize, target: usize, amount: i16 },
    Move { source: usize, target: usize },
    Boot { source: usize, target: usize },
}
```

Capture everything the action needs to resolve in Pass 2. Source and target are cell indices. Operands (popped values, cursor positions) are captured at queue time in Pass 1 so that Pass 2 doesn't need to touch the source program's stack.

### Deletion and IP adjustment

When an instruction is deleted, all later instructions shift down by 1. `Src` and `Dst` are not adjusted (they're interpreted modulo size at point of use). IP needs careful handling:

- **Local `del`** (Pass 1): if the deleted index is at or before the currently executing IP, decrement IP by 1 before the normal post-instruction increment. Effect: execution continues with the next surviving instruction.
- **`delAdj`** (Pass 2): if the deleted index is strictly less than the target program's stored next-tick IP, decrement that stored IP by 1.

Neither `del` nor `delAdj` may delete the last instruction of a size-1 program — fail and set `Flag`.

## Config

All system parameters from the spec in one struct, loaded from a file (TOML or JSON).

```rust
struct SimConfig {
    width: u32,
    height: u32,
    seed: u64,

    r_energy: f64,
    r_mass: f64,
    d_energy: f64,
    d_mass: f64,
    t_cap: f64,
    maintenance_rate: f64,
    maintenance_exponent: f64,  // beta
    local_action_exponent: f64, // alpha
    n_synth: u32,
    inert_grace_ticks: u32,
    p_spawn: f64,
    mutation_base_log2: u32,
    mutation_background_log2: u32,
    program_size_cap: u16,
}
```

No parameter should be a compile-time constant. Everything comes from config so experiments can vary parameters without recompilation.

## Output and observation

Start with two output channels:

**Per-tick metrics** (written every tick or every N ticks): total population, total energy, total mass, mean program size, program diversity (unique hashes), birth/death counts, mutation counts. Cheap to compute, essential for understanding what's happening. CSV is the path of least resistance to pandas/plotting for flat scalar metrics. If you need richer per-tick data (top genotypes, spatial statistics), use JSONL (one JSON object per line) — it gives structure with append-friendliness.

**Grid snapshots** (binary, written every M ticks): full grid state sufficient to restart the sim from that point. Use bincode or MessagePack via serde. Include the tick number and config so snapshots are self-contained. RNG state does not need to be saved since it is derived per-tick from (master_seed, tick, cell_index).

Keep the output logic in its own module. The tick loop should call `observer.record_tick(grid, tick)` and the observer decides what to write based on config. This keeps observation concerns out of the simulation logic.

## WebSocket frontend interface

Add behind a feature flag (`--features web`). The sim runs in its own thread; the web layer is a separate tokio runtime that reads from a shared state.

```
sim thread: tick loop -> writes latest grid state to Arc<RwLock<GridView>>
web thread: axum server -> on client connect, streams GridView snapshots at requested FPS
```

`GridView` is a lightweight rendering-friendly representation — not the full `Cell` struct. Just what the frontend needs to draw: cell color/type, program size, resource levels, maybe a selected cell's full state for an inspector panel.

Protocol: JSON for control messages (pause, resume, step, set parameter, inspect cell). Binary for grid frames (a flat array of per-cell render data). Keep the per-cell render payload small — 4-8 bytes per cell means a 1K×1K grid frame is 4-8MB, fine for WebSocket at 10-30 FPS on localhost.

The frontend should be a separate directory (e.g., `web/`) with its own build. The Rust side doesn't know or care what the frontend looks like — it just serves the WebSocket and static files.

## Testing strategy

Follow the companion doc's section 13 suggested tests as the primary checklist. In addition:

**Determinism test**: run the same config+seed twice, assert identical state after N ticks. This catches any accidental order-dependence or unseeded randomness.

**Conservation tests**: after each tick, verify that total energy and total mass are consistent with the expected external inputs and removals. Small rounding tolerance for stochastic draws, but the books should balance.

**Single-instruction micro-worlds**: hand-authored tiny programs that exercise one instruction in isolation. Assert register/stack/flag state after one tick. The companion's truth tables map directly to these.

**Seed replicator smoke test**: place the seed replicator from the spec in a pre-loaded environment, run for 100 ticks, assert population > 1.

## What to build first

1. Cell/Grid structs and coordinate math
2. Pass 0 (snapshot)
3. Pass 1 (VM interpreter, local instructions only, single-threaded)
4. Single-instruction tests for every opcode
5. Pass 2 (nonlocal resolution)
6. Pass 3 (physics, maintenance, decay, spawning)
7. Determinism and conservation tests
8. Seed replicator smoke test
9. Per-tick CSV metrics output
10. Rayon parallelism (swap `iter` for `par_iter` — should be nearly mechanical if the pass architecture is clean)
11. Binary snapshots
12. WebSocket frontend

Do not parallelize until single-threaded correctness is solid and tested. The pass structure makes adding Rayon almost trivial later — it's a find-and-replace from `iter` to `par_iter` if ownership is right.

## Things to avoid

- **Premature optimization of the VM interpreter.** A `match` dispatch loop is fine. Profile before doing anything clever.
- **Dynamic dispatch in the hot path.** No `dyn Trait` for instructions, cells, or passes. Everything concrete and inlineable.
- **Allocations in the tick loop.** Pre-allocate vecs for snapshots, queued actions, packet lists. Reuse across ticks with `clear()` + refill.
- **Global mutable state.** All state flows through function arguments. The `Grid` is the world; passes are pure-ish functions over it.
- **Mixing observation with simulation.** The tick function computes the next state. The observer reads it. They don't interleave.
- **Over-abstracting the grid.** You don't need a generic `World<C: CellType>`. You have one cell type, one grid type. Keep it concrete.
