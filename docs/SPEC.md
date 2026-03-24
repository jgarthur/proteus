# Proteus v0.2.1 Specification

An artificial life simulator where self-replicating programs emerge, compete, and evolve on a 2D grid with conserved mass and energy.

## Design Philosophy

Proteus prioritizes a minimal substrate that enables emergent complexity. The instruction set is small enough that most single-point mutations produce functional programs, and the minimal self-replicator is short enough that evolution has a smooth fitness landscape to explore. Complex behaviors — movement, predation, cooperation, communication — emerge from generic read/write primitives rather than dedicated opcodes.

The physics layer provides real resource constraints (spatial locality, maintenance costs, conserved transactions with external energy and mass sources) that drive ecological dynamics without prescribing what those dynamics should look like. The substrate includes controlled stochastic elements — background radiation and mass arrival, mutation, `rand`, and probabilistic maintenance and decay — but is otherwise deterministic. All physics is fully discrete and translation-invariant under 90° rotations and reflections.

### Changes from v0.2

v0.3 keeps the v0.2 execution model, instruction set, and lifecycle rules, but reinterprets the ambient arrival parameters `R_energy` and `R_mass` as **Poisson arrival means per cell per tick** instead of Bernoulli probabilities capped at one unit. Ambient decay remains per-quantum binomial thinning at `D_energy` / `D_mass`.

Fresh simulations now initialize background radiation and background mass from the stationary ambient law implied by those rules: `Poisson(R / D)` for each cell when `D > 0`. If `D = 0`, there is no finite steady state for that pool, so fresh simulations start with 0 background in that pool.

## Physics

### Grid

- 2D space with square cells, discrete time steps ("ticks")
- Spatial interactions between adjacent cells only (no diagonal; 4-connected grid)
- Speed of light: 1 cell/tick for information propagation (directed radiation)
- Physical laws invariant under 90° rotations and reflections; no preferred grid position

### Mass and Energy

Mass and energy are fundamental, quantized quantities located within a single cell at a time. The system has conserved internal transactions, with external inputs of both energy and mass driving the economy.

**Mass** exists in three forms:

- **Background mass**: an external mass input. Each cell receives `Poisson(R_mass)` units of background mass per tick. Background mass accumulates in the cell across ticks, and each unit independently decays (is permanently removed) with probability `D_mass` per tick. Background mass is a separate pool from free mass — it cannot be used to pay for instructions or maintenance. Only `collect` converts background mass into free mass. For `D_mass > 0`, an untouched cell has stationary distribution `Poisson(R_mass / D_mass)`, so its expected background mass is `R_mass / D_mass`. Fresh simulations initialize `bg_mass` in each cell by sampling that stationary distribution. If `D_mass = 0`, there is no finite steady state, so fresh simulations start with `bg_mass = 0`. When background mass arrives in a cell with no program, that arrival can mark the cell as a spawn candidate for end-of-tick spontaneous nucleation (see Spontaneous Program Creation).
- **Free mass**: loose raw material in a cell. Used by programs to create new instructions and as maintenance fallback. Attached to the program if one is present.
- **Instructions**: each instruction in a program has mass 1. Instructions are created from free mass. Explicit code-editing deletions such as `del` and `delAdj` recycle an instruction into 1 free mass; maintenance destruction instead consumes the instruction permanently.

**Energy** exists in three forms:

- **Background radiation**: an external energy input. Each cell receives `Poisson(R_energy)` units of background radiation per tick. Background radiation accumulates in the cell across ticks until captured by `absorb`, but each unit independently decays (is permanently removed) with probability `D_energy` per tick. For `D_energy > 0`, an untouched cell has stationary distribution `Poisson(R_energy / D_energy)`, so its expected background radiation is `R_energy / D_energy`. Fresh simulations initialize `bg_radiation` in each cell by sampling that stationary distribution. If `D_energy = 0`, there is no finite steady state, so fresh simulations start with `bg_radiation = 0`. Background radiation is a separate pool from free energy — it cannot be used to pay additional instruction costs and may only be used as emergency payment for base instruction costs (with elevated mutation risk; see Mutation). It is not subject to storage thresholds. Only `absorb` converts background radiation into free energy.
- **Free energy**: stationary, attached to the program in a cell (if present). Used to pay for instruction execution and maintenance.
- **Directed radiation**: a packet of energy propagating at 1 cell/tick in a cardinal direction. Each packet carries exactly 1 energy and a 16-bit signed integer message value, serving as both energy transfer and long-range communication.

### Resource Pool Summary

| Pool | Arrives | Decays | Threshold | Conversion | Can Pay Costs |
|------|---------|--------|-----------|------------|---------------|
| Background radiation | `Poisson(R_energy)` per tick | all units at `D_energy` | none | `absorb` → free energy | emergency only (elevated mutation) |
| Free energy | from absorb, listen, transfers, crystallization | above threshold at `D_energy` | `T_cap × S` | working currency | yes |
| Background mass | `Poisson(R_mass)` per tick | all units at `D_mass` | none | `collect` → free mass | no |
| Free mass | from collect, synthesize, del, delAdj, transfers, crystallization | above threshold at `D_mass` | `T_cap × S` | working currency (maintenance fallback) | yes (maintenance fallback) |

### Decay

Free energy and free mass above a storage threshold decay stochastically each tick. For a cell containing a program of size `S`:

- **Storage threshold**: `T_cap × S` for both free energy and free mass, where `T_cap` is a system parameter.
- **Decay rule**: each unit of free energy above the threshold independently decays (is permanently removed) with probability `D_energy` per tick. Each unit of free mass above the threshold independently decays with probability `D_mass` per tick.
- **Empty cells** have a storage threshold of 0 — all free resources decay.

Resources at or below the threshold do not decay. Implementation: for each cell, compute `excess = max(0, amount - threshold)`, then draw from `Binomial(excess, D)` to determine units removed.

Background radiation and background mass decay independently of thresholds (all units decay at their respective rates, regardless of cell contents).

### Program Size Cap

Program size is hard-capped at 2^15 − 1 instructions.

### Maintenance

Each program pays stochastic maintenance each tick, scaled by program size:

- Compute `q = size_current ^ beta` where `beta` is the `maintenance_exponent` system parameter.
- Draw from `Binomial(floor(q), M)` plus one additional `Bernoulli((q - floor(q)) × M)` to preserve expected maintenance for non-integer exponents.
- With `beta = 1.0` (default), this reduces to `Binomial(program_size, M)`, matching v0.1 behavior.

Maintenance applies to **live** programs every tick. Inert programs use the same rule only **after** they are considered abandoned (see Program Lifecycle).

If free energy is insufficient, maintenance is paid with free mass. If both are exhausted, instructions are destroyed from the end of the program. Each destroyed instruction satisfies one remaining maintenance quantum and is permanently removed; unlike `del` or `delAdj`, it does **not** enter the free-mass pool.

### Program Strength

A program's **strength** is `min(program_size, free_energy)`. Strength determines the cost of hostile actions targeting the program.

### Program Age

Each live program tracks its **age**: the number of ticks since it became live. Age is initialized to 0 when a program becomes live (either by spontaneous creation or by being booted). Inert programs do not age. A program that becomes live during a tick does **not** age on that same tick; it first ages at the end of the following tick. Age is not currently used mechanically but is tracked for diagnostic and analytical purposes.

### Program Lifecycle

Programs exist in two states:

- **Live**: the program executes, pays maintenance, ages, and its cell follows normal protection rules.
- **Inert**: the program does not execute and does not age. Its cell is **always open**. While inert, maintenance depends on whether the program is still under active construction. If maintenance destroys all instructions, the program is removed and the cell becomes empty.

Each inert program tracks **ticks since last incoming write**. Any successful `appendAdj` or `writeAdj` targeting the inert program resets this timer to 0. While this timer is below `inert_grace_ticks`, the offspring is considered **under active construction** and pays no maintenance. Once the timer reaches `inert_grace_ticks`, the offspring is considered **abandoned** and normal maintenance resumes.

**Spontaneous creation** (background mass nucleation in an empty cell): the program is created **live**. This is the primordial bootstrap — no parent required.

**Constructed creation** (first `appendAdj` into an empty cell): the program is created **inert**. It remains inert until a `boot` instruction transitions it to live. While inert, the cell is open, allowing continued construction by the parent or interference by others.

A program that is created or transitions from inert to live during a tick is considered **newborn** for the remainder of that tick. Newborn programs do not execute, pay maintenance, age, or mutate until the following tick. This newborn exemption is uniform: even a previously abandoned inert program that is booted in Pass 2 skips maintenance on that boot tick.

### Spontaneous Program Creation

Spontaneous creation is resolved at the **end of the tick**. If background mass arrived earlier in the tick into a cell that was empty at the moment of arrival, that cell becomes a **spawn candidate**. At the end of the tick, each spawn candidate independently creates a new live program with probability `P_spawn`.

The spawned program consists of a single `nop` instruction with default registers (`Dir` and `ID` randomized), empty stack, and age 0. At the moment of creation, all background radiation in the cell is converted to free energy and all background mass in the cell is converted to free mass (a one-time "crystallization event"). This gives the nascent program a small buffer of working resources.

Because spontaneous creation happens at end-of-tick, the newborn program does not execute, age, pay maintenance, or mutate until the following tick.

### Synthesis

Programs can actively convert energy into mass using the `synthesize` instruction. This consumes `N_synth` energy (in addition to the instruction's base cost of 1) and produces 1 free mass. This is a primary metabolic pathway — organisms convert energy surplus into building material for replication.

## Programs

### Structure

A program is a flat sequence of 8-bit instructions occupying a single cell. There is no sub-structure (no plasmids, no segments). Programs are circular: the instruction pointer wraps modulo program size.

Only one program may occupy a cell at a time.

### CPU and Registers

Each program has its own CPU with the following registers:

| Register | Bits | Access | Default | Description |
|----------|------|--------|---------|-------------|
| `IP` | 16 | read-only | 0 | Instruction pointer (current position in program) |
| `Dir` | 2 | read-write | random | Direction heading: 0=right, 1=up, 2=left, 3=down. Initialized with randomness at program creation. |
| `Src` | 16 | read-write | 0 | Source cursor. Auto-increments on read operations. Interpreted modulo target program size when used. |
| `Dst` | 16 | read-write | 0 | Destination cursor. Auto-increments on write operations. Interpreted modulo target program size when used. |
| `Flag` | 1 | read-only | 0 | Failure/signal bit. Hard success clears it to 0, hard failure sets it to 1, and some instructions/events are neutral and leave it unchanged. Directed-radiation capture via `listen` also sets it to 1. |
| `Msg` | 16 | read-only | 0 | Value carried by last received directed radiation (set by `listen`). |
| `ID` | 8 | read-write | random | Cell identifier. Initialized with randomness at program creation. |
| `LC` | 16 | hidden | 0 | Loop counter, used internally by `for`/`next`. Not directly accessible. |

### Newborn Program State

When a new program is created (first `appendAdj` into an empty cell, or spontaneous `nop` spawn), all registers are initialized to their default values as listed above. `Dir` and `ID` are randomized independently. The stack is empty. Age is 0.

Programs created by `appendAdj` into an empty cell are **inert** (see Program Lifecycle). Programs created by spontaneous nop-spawn are **live**, and a crystallization event converts all background resources in the cell to free resources.

### Flag Semantics

`Flag` has three possible outcomes after an instruction or event:

- **Clear**: set `Flag = 0`. This is the default outcome for hard success.
- **Set**: set `Flag = 1`. Hard failures set the flag. Successful directed-radiation capture via `listen` also sets the flag as a signal.
- **Neutral**: leave `Flag` unchanged.

Unless an instruction explicitly says otherwise, successful execution is **clear** and failure is **set**.

Common **neutral** cases include:
- unknown no-op byte values
- unmatched `next`
- `for` with `LC ≤ 0` that successfully skips to the matching `next`
- `giveE` / `giveM` with nonpositive requested amount
- `listen` when no packet is captured

Common **set** cases include:
- stack underflow / overflow
- `for` with no matching `next`
- protection failure
- empty-target failure
- Pass-2 conflict loss
- failure to pay an instruction's required energy or mass cost

### Stack

LIFO stack of signed 16-bit integers. Maximum size: 2^15 − 1. Instructions (8-bit) are zero-padded to 16 bits when placed on the stack. Stack underflow/overflow sets `Flag` to 1; the failing operation is otherwise skipped. For nonlocal instructions, Pass-1 operand capture is atomic: if required stack operands are unavailable, no operands are consumed and no Pass-2 action is created.

### Stack-to-Instruction Truncation

When a 16-bit stack value is written as an instruction (via `write`, `writeAdj`, or `appendAdj`), the low 8 bits are used.

## Execution Model

### Instruction Timing

The old immediate-vs-1-tick distinction is removed. During each global tick, instructions are classified as either **local** or **nonlocal**:

- **Local**: target the executing program's own cell or read-only adjacent state. Base energy cost is 0 or more depending on the instruction.
- **Nonlocal**: modify or transfer resources to/from adjacent cells. Base energy cost is 0 or more depending on the instruction.

### Local Throughput

Each live program receives a per-tick **local action budget**:

```
local_action_budget = max(1, floor(size_at_tick_start ^ alpha))
```

Where `alpha` is the `local_action_exponent` system parameter (default 1.0).

During Pass 1 execution, each local instruction consumes 1 local action. When the first nonlocal instruction is reached, Pass 1 attempts to queue it. If operand capture succeeds, the instruction is queued (consuming no local action), `IP` advances by 1, and that program stops executing for the remainder of the current tick. If operand capture fails, no action is queued, but `IP` still advances by 1 and that program still stops executing for the remainder of the tick. If no nonlocal instruction is reached, the program stops executing for the tick when its local action budget is exhausted or execution halts.

At most **one nonlocal instruction** may be queued per program per tick. A queued nonlocal instruction is then resolved in Pass 2.

### Energy Payment

Each instruction has a base energy cost (0 for most local instructions, 1 for most world-interaction instructions). Some instructions also have an **additional cost** listed in the instruction table.

When an instruction is reached:

1. If the base cost is > 0, attempt to pay from free energy.
2. If free energy is insufficient and background radiation is present in the cell, background radiation may be used to pay the **base cost only** (elevated mutation risk applies to this tick's mutation check).
3. Additional costs are **never** paid from background radiation. They must be paid from the relevant working pool (`free_energy` or `free_mass` as specified by the instruction).
4. If base-cost payment fails entirely, halt execution, do not advance `IP`, set `Flag = 1`, and mark the cell as open. The remaining local action budget is forfeit.
5. For local instructions, any additional cost is also checked and paid at execution time. If the additional cost cannot be paid, the instruction fails: set `Flag = 1`, do not advance `IP`, mark the cell as open, and forfeit the remaining local action budget. The base cost already paid is **not** refunded.

For nonlocal instructions, the base cost is paid in Pass 1 when queueing is attempted. Operand capture also happens in Pass 1. If operand capture fails, the base cost is not refunded, no action is queued, `IP` still advances by 1, execution for that program ends for the tick, and the cell does **not** open. Any additional cost is paid only if the instruction successfully executes in Pass 2.

### Protection Model

Cells are **protected by default**. A cell becomes **open** (unprotected) for the current tick if its program:

- Executed `listen` this tick
- Executed `nop` this tick
- Failed to pay the energy cost for any instruction this tick
- Is **inert** (always open until booted)

**Empty cells are always open.**

A program that exhausts its local action budget without hitting a nonlocal instruction simply finishes executing for the tick normally; the cell remains protected (unless opened by one of the conditions above).

The following instructions fail against a protected target cell:

- `delAdj` — fails against a protected cell
- `writeAdj` — fails against a protected cell
- `appendAdj` (on occupied cell) — fails against a protected cell

The following nonlocal instructions work regardless of protection status:

- `readAdj` — read-only, non-destructive
- `giveE`, `giveM` — beneficial to target
- `move` — targets empty cells only, which are always open
- `appendAdj` (into empty cell) — empty cells are always open
- `boot` — targets inert programs only (which are always open); fails if target is empty or already live

The following local instructions read adjacent state but do not modify it, and work regardless of protection:

- `senseSize`, `senseE`, `senseM`, `senseID` — read from Pass-1 start snapshot

### Global Execution Order

Each tick proceeds in three passes. All physics is translation-invariant.

**Pass 0 — Snapshot:**

Before any execution, take a snapshot of each cell's free energy, free mass, background radiation, background mass, program size, and program `ID`. This snapshot is used by local sensing instructions (`senseSize`, `senseE`, `senseM`, `senseID`) to ensure sensing results are iteration-order independent.

Also record the set of programs that are **live at tick start**. Only those programs are eligible to execute in Pass 1, pay maintenance this tick, age at end of tick, and mutate at end of tick.

**Pass 1 — Local execution (all cells simultaneously):**

Only programs that were **live at tick start** execute. Inert programs and newborn live programs are skipped entirely.

1. Each program executes instructions sequentially from `IP`:
   - If the instruction is **local**: pay its base energy cost (if any), then pay any additional cost required by the instruction, execute it, and consume 1 local action. If any required payment fails (base or additional), set `Flag = 1`, do not advance `IP`, mark the cell open, and forfeit the remaining local action budget. If the additional cost fails, the already-paid base cost is not refunded.
   - If the instruction is **nonlocal**: pay its base energy cost, then attempt to capture its operands. If base-cost payment fails, halt and mark the cell open. If operand capture succeeds, queue the instruction for Pass 2. If operand capture fails, queue nothing and set `Flag = 1`; this is **not** an open-cell event. In either case after successful base-cost payment, advance `IP` by 1 and stop executing that program for the remainder of the tick.
   - If the local action budget is exhausted, stop executing that program for the remainder of the tick.
2. Track per-program: `absorb_count` (0 if no `absorb` was executed; otherwise 1–4, capped), `absorb_dir` (Dir at first `absorb` call), whether `listen` was executed, whether `collect` was executed, whether `nop` was executed, and whether any base-cost payment this tick used background radiation.

Pass 1 requires no inter-cell communication (sensing reads from the Pass-0 snapshot) and is trivially parallelizable.

**Pass 2 — Nonlocal execution (snapshot-then-apply):**

All nonlocal instructions resolve against the grid state after Pass 1 and before any Pass-2 effects are applied. Validation, protection checks, occupancy checks, target reads, and conflict resolution all use this **pre-Pass-2** state. In particular, `move` sees target occupancy in this pre-Pass-2 state, so swaps do not succeed.

General Pass-2 rules:

- The nonlocal instruction's **base cost** was already paid in Pass 1 and is never refunded.
- Any stack operands needed by the nonlocal instruction are captured in Pass 1 and are not restored if the instruction later fails.
- **Additional costs** are paid only on successful execution in Pass 2.
- Additional costs are always paid from working pools (`free_energy` / `free_mass`), never from background radiation or background mass.
- If a tentative Pass-2 winner later cannot pay its additional cost, that instruction fails and there is **no fallback winner**.
- Source-side cursor increments (`Src`, `Dst`) happen only on successful execution.
- Failed Pass-2 instructions set `Flag = 1` unless explicitly stated otherwise.

Pass-2 instructions are resolved from **least invasive** to **most invasive**:

1. **Read-only class** — `readAdj`
   - All valid `readAdj` instructions succeed independently; they do not conflict with each other.
   - A valid `readAdj` reads from the target program in the pre-Pass-2 state, pushes the instruction value to the source stack, and increments `Src`.
   - If the target cell is empty, `readAdj` fails: it pushes 0, sets `Flag = 1`, and does **not** increment `Src`.

2. **Additive transfer class** — `giveE`, `giveM`
   - All valid transfers succeed independently; they do not conflict with each other.
   - Transfers into the same target cell sum.
   - Transfer amount is computed from the source cell's post-Pass-1 working resources and capped by the source's available free resource.
   - A nonpositive requested amount is a **neutral** no-op.
   - `giveE` transfers only free energy. `giveM` transfers only free mass.

3. **Exclusive class** — `writeAdj`, `appendAdj`, `delAdj`, `move`, `boot`
   - These instructions may change target code, target liveness, or target occupancy.
   - For each target cell, collect all valid exclusive-class instructions targeting that cell.
   - If the only valid exclusive-class instructions targeting a cell are one or more `boot`s, and the pre-Pass-2 target contains an inert program, then **all** of those `boot`s succeed.
   - Otherwise, at most one exclusive-class instruction may succeed for that target cell. Resolve a winner using the conflict rule below.
   - All winning exclusive-class instructions read pre-Pass-2 target state and then apply their writes/effects simultaneously across the grid.

**Conflict resolution** (exclusive-class instructions targeting the same cell):

1. Highest program strength wins.
2. Ties: random, weighted by program size (each tied program's probability of winning is proportional to its size).

Note: mutual targeting (A targets B while B targets A) does not require special handling. These instructions target different cells and both can succeed independently.

**Pass 3 — Physics update:**

1. **Directed radiation propagation**: all directed radiation packets (including newly emitted ones) propagate 1 cell in their direction.
2. **Directed radiation listening**: for each cell, if the program executed `listen` this tick, capture all directed radiation packets present. Free energy gained equals the number of captured packets. If one or more packets were captured, choose one captured packet uniformly at random; set `Msg` to that packet's message value, set `Dir` to the direction that packet arrived from, and set `Flag = 1`.
3. **Directed radiation collision**: for each cell with uncaptured packets: if 2 or more packets are present, all convert to free energy in the cell and are removed. If exactly 1 packet is present, it persists and continues traveling next tick.
4. **Background radiation distribution (`absorb` resolution)**: for each cell containing accumulated background radiation, determine which programs have that cell in their absorb footprint. The absorb footprint for a program with `absorb_count` N and `absorb_dir` D is:
   - N = 0: no footprint (program did not absorb)
   - N ≥ 1: own cell
   - N ≥ 2: own cell + cell in direction D (front)
   - N ≥ 3: own cell + front + cells perpendicular to D (two sides)
   - N ≥ 4: own cell + front + sides + cell opposite D (rear) — all 5 cells  
   Distribute background radiation in each cell equally (floor division) among all programs whose footprint includes that cell. Remainder stays. Captured background radiation becomes free energy in each absorbing program's cell.
5. **Background radiation decay then arrival**: in each cell, each existing unit of background radiation independently decays with probability `D_energy`. After decay, the cell receives `Poisson(R_energy)` new units of background radiation.
6. **Collect resolution**: for each program that executed `collect` this tick, convert all background mass currently in the program's own cell to free mass.
7. **Background mass decay then arrival**: in each cell, each existing unit of background mass independently decays with probability `D_mass`. After decay, the cell receives `Poisson(R_mass)` new units of background mass. If any mass arrives into a cell that is empty at the moment of arrival, mark that cell as a **spawn candidate** for end-of-tick spontaneous creation.
8. **Inert lifecycle update**: for each inert program, if it received an incoming `appendAdj` or `writeAdj` this tick, reset its abandonment timer to 0. Otherwise increment the timer by 1.
9. **Maintenance**: for each program that existed at tick start and is **not newborn this tick**, compute `q = size_current ^ beta`. Draw from `Binomial(floor(q), M)` plus `Bernoulli((q - floor(q)) × M)` where `M` is `maintenance_rate` for live programs, `0` for inert programs still inside the grace window, and `maintenance_rate` for abandoned inert programs. Deduct from free energy, then free mass, then instructions from the end of the program. Each destroyed instruction pays one remaining maintenance quantum and is permanently removed.
10. **Decay**: for each cell, compute excess free energy and free mass above the storage threshold (`T_cap × program_size`, or 0 for empty cells). For each resource, draw from `Binomial(excess, D)` to determine units removed permanently. (Background pools decay in steps 5 and 7 above.)
11. **Age update**: all programs that were live at tick start increment age by 1.
12. **Spontaneous creation**: for each spawn candidate cell, if it is still empty, create a new live single-`nop` program with probability `P_spawn` and immediately crystallize all background radiation and background mass in that cell into free resources.

## Instruction Set

71 opcodes out of 256 possible byte values. All other byte values are no-ops: they cost 0 energy, consume 1 local action, **do not** open the cell, and are **Flag-neutral**. They are simply skipped during execution.

### Encoding

| Range | Category | Count |
|-------|----------|-------|
| `0000 xxxx` | Push literals (−8 to +7) | 16 |
| `0001 0000` – `0001 0100` | Stack operations | 5 |
| `0010 0000` – `0010 1000` | Arithmetic / logic | 9 |
| `0011 0000` – `0011 0100` | Control flow | 5 |
| `0100 0000` – `0100 1110` | Direction, register, and local resource access | 15 |
| `0101 0000` – `0110 0100` | World interaction | 21 |
| All other values | No-op (cost 0, does not open cell) | 185 |

The 72% no-op space provides generous neutral territory for mutations: most random bit flips land on no-ops, providing genetic drift without lethality.

### Push Literals (16 opcodes, local, cost 0)

| Instruction | Opcode | Description |
|-------------|--------|-------------|
| `push N` | `0000 xxxx` | Push sign-extended 4-bit value (−8 to +7) onto stack. `xxxx` encodes the value in two's complement. |

### Stack Operations (5 opcodes, local, cost 0)

| Instruction | Opcode | Description |
|-------------|--------|-------------|
| `dup` | `0001 0000` | Duplicate top of stack. |
| `drop` | `0001 0001` | Remove top of stack. |
| `swap` | `0001 0010` | Swap top two stack values. |
| `over` | `0001 0011` | Copy second element to top of stack. |
| `rand` | `0001 0100` | Push random value (0–255) onto stack. |

### Arithmetic / Logic (9 opcodes, local, cost 0)

All binary operations pop two operands and push one result. Unary operations pop one and push one. Operands are consumed even if the operation fails.

| Instruction | Opcode | Description |
|-------------|--------|-------------|
| `add` | `0010 0000` | Push `second + top`. |
| `sub` | `0010 0001` | Push `second − top`. |
| `neg` | `0010 0010` | Push `−top`. |
| `eq` | `0010 0011` | Push 1 if `second == top`, else 0. |
| `lt` | `0010 0100` | Push 1 if `second < top`, else 0. |
| `gt` | `0010 0101` | Push 1 if `second > top`, else 0. |
| `not` | `0010 0110` | Push 1 if top is 0, else 0. |
| `and` | `0010 0111` | Push 1 if both operands are nonzero, else 0. |
| `or` | `0010 1000` | Push 1 if either operand is nonzero, else 0. |

### Control Flow (5 opcodes, local, cost 0)

| Instruction | Opcode | Description |
|-------------|--------|-------------|
| `for` | `0011 0000` | Pop count from stack into `LC`. If `LC` ≤ 0, scan forward (modulo program size) for the next `next` instruction and skip past it; the scan costs 1 local action total (not per-instruction scanned). If a matching `next` is found, this skip is **Flag-neutral**. If no `next` is found after a full wrap, set `Flag` to 1 and continue. |
| `next` | `0011 0001` | Decrement `LC`. If `LC` > 0, jump to instruction after the matching `for` (found by scanning backward modulo program size for the nearest `for`). If `LC` ≤ 0, continue. Unmatched `next` is a **Flag-neutral** no-op. Costs 1 local action. |
| `jmp` | `0011 0010` | Pop offset from stack. Jump to `IP + offset`. |
| `jmpNZ` | `0011 0011` | Pop value, then pop offset. If value ≠ 0, jump to `IP + offset`. |
| `jmpZ` | `0011 0100` | Pop value, then pop offset. If value = 0, jump to `IP + offset`. |

`for`/`next` do not nest. There is a single `LC` register. A `for` matches the next `next` found by forward scan; a `next` matches the nearest `for` found by backward scan. Both scans wrap modulo program size.

### Direction, Register, and Local Resource Access (15 opcodes, local, cost 0)

| Instruction | Opcode | Description |
|-------------|--------|-------------|
| `cw` | `0100 0000` | Rotate `Dir` 90° clockwise. |
| `ccw` | `0100 0001` | Rotate `Dir` 90° counterclockwise. |
| `getSize` | `0100 0010` | Push this program's instruction count. |
| `getIP` | `0100 0011` | Push `IP` value. |
| `getFlag` | `0100 0100` | Push `Flag` value (0 or 1). |
| `getMsg` | `0100 0101` | Push `Msg` value. |
| `getID` | `0100 0110` | Push `ID` value. |
| `getSrc` | `0100 0111` | Push `Src` value. |
| `getDst` | `0100 1000` | Push `Dst` value. |
| `setDir` | `0100 1001` | Pop value, set `Dir` to `value mod 4`. |
| `setSrc` | `0100 1010` | Pop value, set `Src`. |
| `setDst` | `0100 1011` | Pop value, set `Dst`. |
| `setID` | `0100 1100` | Pop value, set `ID` (truncated to 8 bits). |
| `getE` | `0100 1101` | Push this cell's free energy. Does not include background radiation. |
| `getM` | `0100 1110` | Push this cell's free mass. |

### World Interaction (21 opcodes)

#### Local (target self)

| Instruction | Opcode | Base Cost | Add'l Cost | Description |
|-------------|--------|-----------|------------|-------------|
| `nop` | `0101 0000` | 0 | — | No operation. **Opens cell.** |
| `absorb` | `0101 0001` | 0 | — | Mark this program as absorbing for Pass 3 background radiation distribution. First execution per tick sets `absorb_count = 1` and `absorb_dir` to current `Dir`. Subsequent executions increment `absorb_count` (capped at 4) but do not update `absorb_dir`. Calls after `absorb_count = 4` have no further effect. Does **not** open cell. |
| `listen` | `0101 0010` | 0 | — | Mark this program as listening for Pass 3 directed radiation capture. Executing `listen` in Pass 1 is **Flag-neutral**: it opens the cell and marks the program for capture, but does not otherwise change `Flag` immediately. In Pass 3, all directed radiation packets in this cell are captured: free energy gained equals number of packets; if any packets were captured, choose one captured packet uniformly at random, set `Msg` to that packet's value, set `Dir` to the direction that packet arrived from, and set `Flag` to 1. **Opens cell.** Idempotent within a tick (multiple executions have no additional effect). |
| `collect` | `0101 0011` | 0 | — | Mark this program for Pass 3 background mass collection. All background mass in this cell is converted to free mass during Pass 3. Idempotent within a tick. |
| `emit` | `0101 0100` | 1 | — | Pop message value from stack. Send a directed radiation packet in direction `Dir` carrying that message value and 1 energy. The base cost of 1 energy is the energy that becomes the packet. |
| `read` | `0101 0101` | 0 | — | Push instruction at `self[Src mod size]` onto stack. Increment `Src`. |
| `write` | `0101 0110` | 1 | — | Pop value. Overwrite instruction at `self[Dst mod size]` with value (low 8 bits). Increment `Dst`. Does not change program size. |
| `del` | `0101 0111` | 1 | — | Delete instruction at `self[Dst mod size]`. Freed mass (1) becomes free mass in this cell. Fails and sets `Flag` if the program size is 1. Does not modify `Dst`. |
| `synthesize` | `0101 1000` | 1 | E = `N_synth` | Convert energy to mass. Consumes `N_synth` additional free energy and produces 1 free mass in this cell. Fails (sets `Flag`) if insufficient energy. |

#### Local sensing (read-only adjacent state, from Pass-0 snapshot)

`senseSize`, `senseE`, and `senseM` treat an empty adjacent cell as a valid reading and therefore clear `Flag` on success. `senseID` is different: because `ID = 0` is a valid program state, it uses `Flag = 1` to disambiguate an empty neighbor from a real neighbor with `ID = 0`.

| Instruction | Opcode | Base Cost | Description |
|-------------|--------|-----------|-------------|
| `senseSize` | `0101 1001` | 0 | Push program size of adjacent cell in direction `Dir` (0 if empty). Empty is a valid reading. Reads from Pass-0 snapshot. |
| `senseE` | `0101 1010` | 0 | Push free energy of adjacent cell in direction `Dir` (0 if empty). Empty is a valid reading. Reads from Pass-0 snapshot. |
| `senseM` | `0101 1011` | 0 | Push free mass of adjacent cell in direction `Dir` (0 if empty). Empty is a valid reading. Reads from Pass-0 snapshot. |
| `senseID` | `0101 1100` | 0 | Push `ID` register of adjacent cell's program in direction `Dir` (0 if empty, sets `Flag` to 1). Reads from Pass-0 snapshot. |

#### Nonlocal (target adjacent cell in direction `Dir`)

| Instruction | Opcode | Base Cost | Add'l Cost | Protection | Description |
|-------------|--------|-----------|------------|------------|-------------|
| `readAdj` | `0101 1101` | 0 | — | No | Push instruction at `neighbor[Src mod size]` onto stack. On success, increment `Src`. If neighbor cell is empty, push 0, set `Flag` to 1, and do **not** increment `Src`. |
| `writeAdj` | `0101 1110` | 1 | — | **Yes** | Pop value. Overwrite instruction at `neighbor[Dst mod size]` (low 8 bits). The old instruction is recycled (no net mass cost). On success, increment `Dst`. Fails if neighbor cell is empty, protected, or loses Pass-2 conflict resolution. |
| `appendAdj` | `0101 1111` | 1 | 1 mass | **Yes** (occupied) | Pop value. Append instruction (low 8 bits) to end of neighbor's program (or create new inert program if cell is empty). The additional cost of 1 free mass is paid only on success. Does not use or modify `Dst`. Checks protection only if the target cell is occupied; empty cells are always open. Fails if the occupied target is protected, if it loses Pass-2 conflict resolution, or if the resulting program would exceed the size cap. |
| `delAdj` | `0110 0000` | 1 | E = target strength | **Yes** | Delete instruction at `neighbor[Dst mod size]`. The additional energy cost equals the target's strength in the pre-Pass-2 state and is paid only on success. Target program size decreases by 1. Freed mass (1) goes to **this** cell (the attacker). Fails if the target is empty, protected, size 1, or loses Pass-2 conflict resolution. On success, increment `Dst` (unlike local `del`, which leaves `Dst` fixed so repeated self-deletions keep targeting the next surviving local instruction). |
| `giveE` | `0110 0001` | 0 | — | No | Pop amount. Transfer `min(amount, free_energy)` to adjacent cell. This has base cost 0: moving energy is treated as free once the energy already exists in the source cell. If amount ≤ 0, no transfer and the instruction is **Flag-neutral**. |
| `giveM` | `0110 0010` | 1 | — | No | Pop amount. Transfer `min(amount, free_mass)` to adjacent cell. This has base cost 1: moving matter requires work even though the transferred mass itself comes from the source free-mass pool. If amount ≤ 0, no transfer and the instruction is **Flag-neutral**. |
| `move` | `0110 0011` | 1 | — | No | Relocate this program and all its free resources (free energy, free mass) to the adjacent cell in direction `Dir`. Background pools remain in the origin cell. Occupancy is checked against the pre-Pass-2 grid state, so swaps do not succeed. Fails if the target cell is occupied or if it loses Pass-2 conflict resolution. |
| `boot` | `0110 0100` | 0 | — | No | Transition an inert program in the adjacent cell (direction `Dir`) to live. The target begins executing on the next tick with `IP` = 0 and is newborn for the remainder of the current tick. Fails if the target cell is empty, if the target program is already live, or if it loses exclusive-class Pass-2 conflict resolution. Multiple `boot`s targeting the same inert program all succeed if no other exclusive-class instruction targets that cell. |

#### Instruction Deletion Semantics

When an instruction at index `i` is deleted from a program, all instructions after index `i` shift down by 1. `Src` and `Dst` are **not** adjusted; they are raw cursor values interpreted modulo program size at the point of use.

Deletion uses the heuristic **continue with the next surviving instruction**:

- **Local `del`**: if the deleted instruction is at or before the currently executing instruction index, decrement `IP` by 1 before the normal fallthrough increment. Equivalently, execution continues with the instruction that would have followed the deleted instruction in the post-deletion program.
- **Nonlocal `delAdj`**: the target program has already finished its Pass-1 execution for the current tick, so only its stored next-tick `IP` matters. If the deleted index `i` is strictly less than the target program's stored `IP`, decrement that stored `IP` by 1.

Neither `del` nor `delAdj` may delete the final remaining instruction of a program. If a deletion targets a size-1 program, it fails and sets `Flag = 1`.

Programs may still be removed by maintenance or decay-driven destruction; explicit deletion instructions never reduce a program below size 1.

### Instruction Count Summary

| Category | Count |
|----------|-------|
| Push literals | 16 |
| Stack operations | 5 |
| Arithmetic / logic | 9 |
| Control flow | 5 |
| Direction + registers + local resource access | 15 |
| World interaction (local) | 9 |
| World interaction (local sensing) | 4 |
| World interaction (nonlocal) | 8 |
| **Total opcodes** | **71** |
| No-op byte values | **185** (72% of opcode space) |

## Mutations

### Mutation Model

Once per tick, each program that was **live at tick start** has a chance to mutate. A single instruction is selected uniformly at random from the program, and one random bit in its 8-bit opcode is flipped. The mutation probability depends on whether the program used background radiation to pay any **base instruction cost** this tick:

- **Normal**: probability `2^(-mutation_base_log2)` per tick.
- **Background-radiation-stressed**: if any base-cost payment this tick used background radiation, the probability increases to `min(x / 2^(mutation_background_log2), 1)` where `x` is the total amount of background radiation consumed this tick for base-cost payment.

Mutations do not affect the current tick's execution.

## System Parameters

| Parameter | Symbol | Description | Suggested Start |
|-----------|--------|-------------|-----------------|
| Energy arrival rate | `R_energy` | Mean background-radiation arrivals per cell per tick (`Poisson(R_energy)`) | 0.25 |
| Mass arrival rate | `R_mass` | Mean background-mass arrivals per cell per tick (`Poisson(R_mass)`) | 0.05 |
| Nop-spawn probability | `P_spawn` | P(end-of-tick nucleation in a spawn-candidate empty cell) | 0 |
| Energy decay rate | `D_energy` | P(each unit of background radiation or excess free energy removed per tick) | 0.01 |
| Mass decay rate | `D_mass` | P(each unit of background mass or excess free mass removed per tick) | 0.01 |
| Decay threshold | `T_cap` | Multiplier on program_size for free resource decay floor | 4 |
| Maintenance rate | `M` | P(each maintenance quantum costs 1 energy per tick) | 1/128 |
| Inert grace window | `inert_grace_ticks` | Ticks without incoming write before abandoned inert pays maintenance | 10 |
| Synthesis cost | `N_synth` | Additional energy consumed per mass produced | 1 |
| Baseline mutation exponent | `mutation_base_log2` | Baseline mutation rate is `2^(-value)` per program per tick | 16 |
| Background mutation exponent | `mutation_background_log2` | Background-stressed mutation rate is `min(x / 2^(value), 1)` | 8 |
| Local action exponent | `alpha` | Local action budget = `max(1, floor(size^alpha))` | 1.0 |
| Maintenance exponent | `beta` | Maintenance quanta = `size^beta` | 1.0 |

## Seed Replicator

The following program is the recommended initial organism:

```
; === Seed Replicator (12 instructions) ===
;
absorb          ; Tick 1:  gather energy (bg radiation from own cell)
collect         ; Tick 1:  gather mass (bg mass from own cell)
cw              ;          rotate Dir (covers different neighbors over cycles)
push 0          ;
setSrc          ;          reset Src to 0
getSize         ;          push program size (12) for loop count
for             ;          begin loop
  read          ; Tick 1+: read self[Src], Src++
  appendAdj     ; Tick 2+: append to neighbor in Dir (nonlocal, stops execution for the tick)
next            ;          decrement LC, loop if > 0
boot            ; Tick N:  activate offspring (nonlocal, stops execution for the tick)
nop             ;          padding (opens cell — vulnerability window)
;
; Execution trace:
;   Tick 1: absorb, collect, cw, push 0, setSrc, getSize, for, read
;           (8 local actions, all within budget of 12)
;           then appendAdj (nonlocal, queued, execution for the tick stops)
;   Tick 2–12: next, read (2 local actions), then appendAdj (nonlocal)
;   Tick 13: next (LC=0, falls through), boot (nonlocal)
;   Tick 14: nop (local, opens cell), absorb, collect, cw, ... cycle repeats
;
; Timing:  1 + 11 + 1 + 1 = 14 ticks per replication cycle
; Energy:  12 (appendAdj base costs) = 12 energy per cycle
; Mass:    12 (one per appended instruction)
;          sources: collect from own cell bg mass + synthesize (evolved)
;
; The offspring is inert during construction. It becomes live when
; boot executes, starting with absorb on the next tick.
```

### Initial Conditions

The seed organism should be placed in a pre-loaded environment: the seed cell and its neighbors should contain sufficient free mass (≥ 12) and free energy (≥ 20) so that the first replication cycle succeeds. This models a resource-rich primordial environment.

### Why This Replicator is Plausible as a First Evolver

The seed replicator uses five distinct local instructions with world effects (`absorb`, `collect`, `nop`, `read`, plus control flow and register ops) and three nonlocal instructions (`appendAdj`, `boot`, plus `read` feeding into `appendAdj`). The offspring is inert during construction, preventing premature execution of partially written code. The `Dst` register is never used. The only cursor management is resetting `Src` to 0 each cycle.

The seed does not check resource levels before attempting replication, does not provision offspring with energy, and does not use `synthesize` to manufacture mass. These are all evolutionary optimization opportunities.

### Evolutionary Optimization Opportunities

The seed replicator is intentionally unoptimized. Evolution can discover improvements such as:

- **Resource gating**: check energy/mass levels before attempting replication
- **Synthesis**: use `synthesize` to convert energy surplus into mass
- **Multi-cycle harvesting**: loop `absorb`/`collect`/`synthesize` across multiple ticks to accumulate mass before replicating
- **Expanded harvesting**: execute `absorb` multiple times per tick to collect from adjacent cells
- **Offspring provisioning**: `giveE`/`giveM` after boot to help offspring survive
- **Directional awareness**: `senseSize` to find empty cells before replication
- **Predation**: `delAdj` to consume neighbor instructions as mass (requires target to be open)
- **Defense**: minimize time in open states (`listen`, `nop`) to reduce vulnerability
- **Communication**: `emit`/`listen` to coordinate with kin
- **Kin recognition**: `senseID` to distinguish kin from non-kin
- **Code injection**: `writeAdj` to modify open neighbor programs
- **Streamlining**: eliminate unnecessary instructions to reduce maintenance and replication time
- **Movement**: `move` to relocate toward resources or away from threats

## Reference Energy/Mass Budget

Let `S` = program size, `T` = ticks per replication cycle, `K` = mass produced via `synthesize` per cycle (0 for the seed).

### Energy Income

After an `absorb` drains a cell's background radiation to 0, its **expected** refill over `T` ticks accounting for ongoing decay is:

```
refill(T) = R_energy × (1 − (1 − D_energy)^T) / D_energy
```

These estimates assume a solitary absorber with exclusive access to its footprint. If multiple organisms overlap on the same absorb footprint, each receives only its share of the captured background radiation and the realized income is lower.

A solitary absorber at absorb_count=1 drains only its own cell:

```
E_in = 1 × refill(T)
```

At absorb_count=4 (all 5 cells):

```
E_in = 5 × refill(T)
```

### Energy Costs

```
E_maintain  = T × (S^beta) × M                     (expected maintenance)
E_instruct  = sum of base costs per cycle           (instruction base costs)
E_synth     = K × (N_synth + 1)                    (synthesis base + additional cost)
```

### Viability Condition (energy)

```
E_in  >  E_maintain + E_instruct + E_synth
```

### Mass Income

For the organism's own cell, expected background-mass refill is similar:

```
mass_refill(T) = R_mass × (1 − (1 − D_mass)^T) / D_mass
```

```
M_collect  = mass_refill(T)                         (from collect, own cell only)
M_synth    = K                                       (from synthesize)
```

### Mass Cost

```
M_out = S                                           (one per appended instruction)
```

### Viability Condition (mass)

```
M_collect + M_synth  ≥  S
```

### Example: Seed Replicator at Default Parameters

With `R_energy = 0.25`, `R_mass = 0.05`, `D_energy = D_mass = 0.01`, `M = 1/128`, `S = 12`, `T = 14`, `alpha = beta = 1.0`, absorb_count = 1 (own cell only), no synthesis (`K = 0`):

```
refill(14) = 0.25 × (1 − 0.99^14) / 0.01 = 0.25 × 13.1 = 3.28

E_in       = 1 × 3.28                = 3.28   (own cell only)
E_maintain = 14 × 12 × (1/128)       = 1.31
E_instruct = 12 (appendAdj)          = 12.0
E_total    = 13.31

Energy deficit: −10.0 per cycle with single absorb.
```

With absorb_count = 4 (all 5 cells):

```
E_in       = 5 × 3.28                = 16.4
Energy surplus: 3.1 per cycle. Marginal.
```

```
mass_refill(14) = 0.05 × (1 − 0.99^14) / 0.01 = 0.05 × 13.1 = 0.66
M_collect  = 0.66 per cycle

Need: 12. Shortfall of ~11.3 per cycle.
```

**The seed requires pre-loaded resources and cannot self-sustain from ambient alone in a single cycle.** Evolution's immediate pressure is toward multi-cycle harvesting, expanded absorb footprints, and synthesis. A mature organism that executes `absorb` 4× per tick (harvesting 5 cells), accumulates across 3 cycles, and uses `synthesize` to convert energy surplus to mass becomes viable.

## Implementation Notes

### Pass-0 Snapshot

Before Pass 1, snapshot each cell's free energy, free mass, background radiation, background mass, program size, and `ID`. This snapshot is read by `senseSize`, `senseE`, `senseM`, `senseID` during Pass 1. The snapshot ensures sensing results are independent of cell iteration order. In practice, only cells adjacent to programs that execute sense instructions need snapshotted values; lazy snapshotting is valid.

### Snapshot-and-Apply for Pass 2

Pass 2 uses a snapshot-and-apply pattern. In practice, most cells are not targeted. An implementation can collect nonlocal instructions into a sparse list, group by target cell, resolve each group independently (trivially parallel), and apply changes.

### Stochastic Implementation

Several mechanics use simple stochastic laws that benefit from dedicated helpers:

- **Background radiation/mass decay**: `Binomial(pool_size, D)` per cell per tick.
- **Free energy/mass decay**: `Binomial(excess, D)` per cell per tick.
- **Maintenance**: `Binomial(floor(q), M)` plus `Bernoulli(frac(q) × M)` per program per tick.
- **Background input**: `Poisson(R_energy)` and `Poisson(R_mass)` per cell per tick.
- **Mutation**: `Bernoulli(mutation_rate)` per live program per tick.

### Parallelization

- **Pass 0** is embarrassingly parallel (read-only snapshot).
- **Pass 1** is embarrassingly parallel: each cell executes independently using only the Pass-0 snapshot for sensing.
- **Pass 2** conflict resolution is independent per target cell.
- **Pass 3** is a combination of local stencil operations and per-cell updates.

### Grid Boundary

Periodic (toroidal) boundary conditions. Directed radiation wraps around grid edges.

## Future Directions (post-v0.2)

- **Plasmids**: modular code, horizontal gene transfer
- **Insert instruction**: insert at target position, shifting subsequent
- **Evolvable instruction sets**: organisms define opcode-to-behavior mapping
- **Environmental variation**: spatially/temporally varying resource rates
- **Labels**: named jump targets
- **mul/div/mod**: complex arithmetic
- **Trap instruction**: active defense (redirect incoming writes)
- **Function calls**: call stack for subroutine reuse
- **Long-range sensing**: detect programs beyond adjacent cells
- **Multicellular structures**: extended-body organisms
- **Nested loops**: multiple `LC` registers
- **Swap movement**: exchange positions with adjacent program

## Known Limitations (monitor in simulation)

- **`absorb`/`listen` split adequacy**: the split separates energy metabolism from communication. Monitor whether programs need to sacrifice defense (opening cell via `listen`) too often for energy, or whether the split creates clean evolutionary specialization between energy-focused and communication-focused strategies.
- **Anti-complexity pressure**: with alpha = beta = 1.0, larger programs get linear local actions but pay linear maintenance. If evolution consistently collapses toward minimal replicators, consider alpha > 1 or beta < 1 to give size a net advantage.
- **Mass scarcity**: with `takeM` removed, mass comes only from `collect` (own cell) and `synthesize`. If mass bottleneck is too severe, tune `R_mass`, `D_mass`, or `N_synth`.
- **Stationary environments**: if local conditions are constant, stationary absorbers can persist indefinitely. Consider spatial/temporal variation if reproduction fails to persist.
- **Prefix-writing bias**: failed replication creates whatever prefix was already copied.
- **Movement in dense populations**: `move` only works into empty cells. Consider `swap` if simulations show frozen spatial dynamics.
- **`for`/`next` non-nesting**: single `LC` limits algorithmic complexity.
- **Directed radiation range**: packets travel indefinitely until absorbed, collided, or wrapped. In large grids, stray packets may accumulate. Monitor energy balance.
