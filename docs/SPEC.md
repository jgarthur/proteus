# Proteus v0 Specification

An artificial life simulator where self-replicating programs emerge, compete, and evolve on a 2D grid with conserved mass and energy.

## Design Philosophy

Proteus v0 prioritizes a minimal substrate that enables emergent complexity. The instruction set is small enough that most single-point mutations produce functional programs, and the minimal self-replicator is short enough (10 instructions) that evolution has a smooth fitness landscape to explore. Complex behaviors — movement, predation, cooperation, communication — emerge from generic read/write primitives rather than dedicated opcodes.

The physics layer provides real resource constraints (spatial locality, maintenance costs, conserved transactions with an external energy source) that drive ecological dynamics without prescribing what those dynamics should look like. The substrate includes controlled stochastic elements — background radiation arrival, mutation, `rand`, radiation-to-mass conversion, and probabilistic maintenance — but is otherwise deterministic. All physics is fully discrete and translation-invariant under 90° rotations and reflections.

## Physics

### Grid

- 2D space with square cells, discrete time steps ("ticks")
- Spatial interactions between adjacent cells only (no diagonal; 4-connected grid)
- Speed of light: 1 cell/tick for information propagation (directed radiation)
- Physical laws invariant under 90° rotations and reflections; no preferred grid position

### Mass and Energy

Mass and energy are fundamental, quantized quantities located within a single cell at a time. The system has conserved internal transactions and dissipative caps, with background radiation as the sole external energy input.

**Mass** exists in two forms:

- **Free mass**: loose raw material in a cell. Used by programs to create new instructions. Attached to the program if one is present.
- **Instructions**: each instruction in a program has mass 1. Instructions are created from free mass and can be destroyed back into free mass.

**Energy** exists in three forms:

- **Free energy**: stationary, attached to the program in a cell (if present). Used to pay for instruction execution.
- **Directed radiation**: a packet of energy propagating at 1 cell/tick in a cardinal direction. Carries a 16-bit signed integer value, doubling as energy transfer and long-range communication.
- **Background radiation**: the only external energy input to the system. Each cell has a probability of 1/8 of receiving 1 unit of background radiation each tick.

### Conservation and Caps

- **Program size** hard-capped at 2^15 − 1 instructions.
- **Free energy** soft-capped at 2 × program size. Each tick, 10% of excess above the cap is removed permanently from the system.
- **Free mass** soft-capped at 2 × program size. Each tick, 10% of excess above the cap decays into an equal quantity of free energy.
- **Cells without programs** can contain free energy and mass, but both decay toward 0 over time (10% per tick).

### Maintenance

Each program pays a maintenance cost per tick computed as follows: pay `floor(program_size / 64)` energy with certainty, plus 1 additional energy with probability `(program_size mod 64) / 64`. The expected cost is exactly `program_size / 64` per tick, with no threshold cliff.

If free energy is insufficient, maintenance is paid with free mass. If both are exhausted, instructions are destroyed from the end of the program.

### Program Strength

A program's **strength** is `min(program_size, free_energy)`. Strength determines the cost of hostile actions targeting the program.

### Program Age

Each program tracks its **age**: the number of ticks it has existed. Age is initialized to 0 when a program is created (by `appendAdj` into an empty cell, or by background radiation spawning a `nop` program). Age is used for conflict resolution tiebreaking (see Pass 2).

### Background Radiation Decay

When excess background radiation is removed at the end of a tick, it converts to mass with probability 1/256. If no program is present in the cell, a new program consisting of a single `nop` instruction is created (age 0, default registers). Otherwise, 1 free mass is added to the cell.

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
| `Flag` | 1 | read-only | 0 | 0 = last instruction succeeded; 1 = last instruction failed or message received via `absorb`. Successful instruction execution resets `Flag` to 0. |
| `Msg` | 16 | read-only | 0 | Value carried by last received directed radiation. |
| `ID` | 8 | read-write | random | Cell identifier. Initialized with randomness at program creation. |
| `LC` | 16 | hidden | 0 | Loop counter, used internally by `for`/`next`. Not directly accessible. |

### Newborn Program State

When a new program is created (first `appendAdj` into an empty cell, or background radiation `nop` spawn), all registers are initialized to their default values as listed above. `Dir` and `ID` are randomized independently. The stack is empty. Age is 0. Free energy and free mass in the cell are unchanged (they belong to the cell, not the program).

### Stack

LIFO stack of signed 16-bit integers. Maximum size: 2^15 − 1. Instructions (8-bit) are zero-padded to 16 bits when placed on the stack. Stack underflow/overflow sets `Flag` to 1; the failing operation is otherwise skipped.

### Stack-to-Instruction Truncation

When a 16-bit stack value is written as an instruction (via `write`, `writeAdj`, or `appendAdj`), the low 8 bits are used.

## Execution Model

### Instruction Timing

Instructions come in two types:

- **Immediate** (0-tick): execute instantly, cost no energy. Includes all stack, arithmetic, control flow, direction, and register instructions. The total number of immediate instructions per tick is capped at program size to prevent infinite loops.
- **1-tick**: cost 0 or more energy. A program executes immediate instructions until it reaches a 1-tick instruction. It then pays the base energy cost (or halts if unable), executes the instruction, advances `IP` by 1, and the tick ends for that program.

If a program has 0 free energy for a 1-tick instruction, it may pay using background radiation present in its cell. This carries a significantly higher mutation probability (see Mutations).

### Protection Model

Cells are **protected by default**. A cell becomes **open** (unprotected) for the current tick if its program:

- Executed `absorb` this tick
- Executed `nop` this tick
- Failed to pay the energy cost for any instruction this tick
- Was halted after exceeding the immediate instruction limit

**Empty cells are always open.**

Three instructions check protection:

- `delAdj` — fails against a protected cell
- `takeE` — fails against a protected cell
- `takeM` — fails against a protected cell

All other nonlocal instructions (`readAdj`, `writeAdj`, `appendAdj`, `senseSize`, `senseE`, `senseM`, `giveE`, `giveM`, `move`) work regardless of protection status. (Note: `move` targets empty cells only, which are always open.)

Rationale: `writeAdj` and `appendAdj` do not check protection because multi-tick replication into a cell would otherwise be impossible — the partially-written offspring becomes protected after its first tick, blocking further writes. Unrestricted writing also enables code injection as an emergent parasitic strategy, at a real energy and mass cost to the attacker.

### Global Execution Order

Each tick proceeds in three passes. All physics is translation-invariant.

**Pass 1 — Local execution (all cells simultaneously):**

1. Each program executes immediate instructions until reaching a 1-tick instruction, or until the immediate instruction limit (equal to program size) is exceeded, in which case the program is halted and the cell becomes open.
2. If a 1-tick instruction is reached, pay its base energy cost. If payment fails, halt, do not advance `IP`, and mark cell as open. If paid, determine whether the instruction is local or nonlocal.
3. Local 1-tick instructions execute immediately.
4. Nonlocal 1-tick instructions are placed in the nonlocal queue for Pass 2.
5. `IP` advances by 1. The executed instruction has a chance to mutate (not affecting this tick's execution).

Pass 1 requires no inter-cell communication and is trivially parallelizable.

**Pass 2 — Nonlocal execution (snapshot-and-apply):**

All nonlocal instructions are resolved against a snapshot of the grid state taken after Pass 1.

1. Pay additional costs for each nonlocal instruction. If costs cannot be paid, the instruction fails.
2. Check protection on target cells. `delAdj`, `takeE`, and `takeM` targeting a protected cell fail.
3. Group remaining instructions by target cell. For each target cell:
   - If one instruction targets it: that instruction succeeds.
   - If multiple instructions target it: resolve using the conflict resolution rule (see below).
4. Execute all winning instructions simultaneously against the snapshot. Each winner reads pre-resolution state and writes to its target cell. Since each target cell has at most one winner, there are no write conflicts.
5. Source-side effects (e.g., mass/energy deducted from the attacking cell, cursor increments) are applied unconditionally for each winning instruction. Since each cell executes at most one nonlocal instruction per tick, source-side effects never conflict with each other.

**Conflict resolution** (multiple instructions targeting the same cell):

1. Highest program strength wins.
2. Ties: highest program size wins.
3. Ties: oldest program (highest age) wins.
4. Ties: random.

This rule is deterministic through tier 3 and requires only source-program properties, making it well-defined for all target cells including empty ones.

Note: mutual targeting (A targets B while B targets A) does not require special handling. These instructions target different cells and both can succeed independently. No conflict graph or cycle detection is needed.

**Pass 3 — Physics update:**

1. Directed radiation propagates to its next cell. When multiple packets of directed radiation arrive in the same cell simultaneously, all are converted to free energy.
2. Background radiation: each cell receives 1 unit with probability 1/8.
3. Unused background radiation decays into free mass with probability 1/256. If no program exists, a single `nop` program is created (age 0, default registers). Otherwise, 1 free mass is added.
4. Maintenance costs are paid: `floor(program_size / 64)` energy with certainty, plus 1 additional energy with probability `(program_size mod 64) / 64`. Deducted from free energy, then free mass, then instructions from end of program.
5. Excess free energy and free mass above soft caps decay (10% of excess removed per tick).
6. All program ages increment by 1.

## Instruction Set

66 opcodes out of 256 possible byte values. All other byte values are no-ops: immediate, free, and they do **not** open the cell. They are simply skipped during execution.

### Encoding

| Range | Category | Count |
|-------|----------|-------|
| `0000 xxxx` | Push literals (−8 to +7) | 16 |
| `0001 0000` – `0001 0100` | Stack operations | 5 |
| `0010 0000` – `0010 1000` | Arithmetic / logic | 9 |
| `0011 0000` – `0011 0100` | Control flow | 5 |
| `0100 0000` – `0100 1100` | Direction and register access | 13 |
| `0101 0000` – `0110 0001` | World interaction | 18 |
| All other values | No-op (immediate, free, does not open cell) | 190 |

The 74% no-op space provides generous neutral territory for mutations: most random bit flips land on no-ops, providing genetic drift without lethality.

### Push Literals (16 opcodes, immediate)

| Instruction | Opcode | Description |
|-------------|--------|-------------|
| `push N` | `0000 xxxx` | Push sign-extended 4-bit value (−8 to +7) onto stack. `xxxx` encodes the value in two's complement. |

### Stack Operations (5 opcodes, immediate)

| Instruction | Opcode | Description |
|-------------|--------|-------------|
| `dup` | `0001 0000` | Duplicate top of stack. |
| `drop` | `0001 0001` | Remove top of stack. |
| `swap` | `0001 0010` | Swap top two stack values. |
| `over` | `0001 0011` | Copy second element to top of stack. |
| `rand` | `0001 0100` | Push random value (0–255) onto stack. |

### Arithmetic / Logic (9 opcodes, immediate)

All binary operations pop two operands and push one result. Unary operations pop one and push one. Operands are consumed even if the operation fails (e.g., division by zero).

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

### Control Flow (5 opcodes, immediate)

| Instruction | Opcode | Description |
|-------------|--------|-------------|
| `for` | `0011 0000` | Pop count from stack into `LC`. If `LC` ≤ 0, scan forward (modulo program size) for the next `next` instruction and skip past it. The scan counts as a single immediate instruction regardless of distance scanned. If no `next` is found after a full wrap, set `Flag` to 1 and continue. |
| `next` | `0011 0001` | Decrement `LC`. If `LC` > 0, jump to instruction after the matching `for` (found by scanning backward modulo program size for the nearest `for`). If `LC` ≤ 0, continue. Unmatched `next` is a no-op. |
| `jmp` | `0011 0010` | Pop offset from stack. Jump to `IP + offset`. |
| `jmpNZ` | `0011 0011` | Pop value, then pop offset. If value ≠ 0, jump to `IP + offset`. |
| `jmpZ` | `0011 0100` | Pop value, then pop offset. If value = 0, jump to `IP + offset`. |

`for`/`next` do not nest. There is a single `LC` register. A `for` matches the next `next` found by forward scan; a `next` matches the nearest `for` found by backward scan. Both scans wrap modulo program size. This is a deliberate v0 simplification.

### Direction and Register Access (13 opcodes, immediate)

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

### World Interaction (18 opcodes, 1-tick)

#### Local (target self)

| Instruction | Opcode | Base Cost | Add'l Cost | Description |
|-------------|--------|-----------|------------|-------------|
| `nop` | `0101 0000` | 0 | — | No operation. **Opens cell.** |
| `absorb` | `0101 0001` | 0 | — | Capture all background and directed radiation in this cell as free energy. If directed radiation was received: set `Msg` to its value, set `Dir` to the direction it arrived from, set `Flag` to 1. **Opens cell.** |
| `emit` | `0101 0010` | 1 | — | Pop value from stack. Send directed radiation carrying that value in direction `Dir`. |
| `read` | `0101 0011` | 0 | — | Push instruction at `self[Src mod size]` onto stack. Increment `Src`. |
| `write` | `0101 0100` | 1 | — | Pop value. Overwrite instruction at `self[Dst mod size]` with value (low 8 bits). Increment `Dst`. Does not change program size. |
| `del` | `0101 0101` | 1 | — | Delete instruction at `self[Dst mod size]`. Program size decreases by 1. Freed as 1 free mass in this cell. |

#### Nonlocal (target adjacent cell in direction `Dir`)

| Instruction | Opcode | Base Cost | Add'l Cost | Protection | Description |
|-------------|--------|-----------|------------|------------|-------------|
| `readAdj` | `0101 0110` | 0 | — | No | Push instruction at `neighbor[Src mod size]` onto stack. Increment `Src`. If neighbor cell is empty, push 0 and set `Flag` to 1. |
| `writeAdj` | `0101 0111` | 1 | — | No | Pop value. Overwrite instruction at `neighbor[Dst mod size]` (low 8 bits). The old instruction is recycled (no net mass cost). Increment `Dst`. Fails if neighbor cell is empty (nothing to overwrite). |
| `appendAdj` | `0101 1000` | 1 | 1 mass | No | Pop value. Append instruction (low 8 bits) to end of neighbor's program (or create new program if cell is empty). Costs 1 free mass. Does not use or modify `Dst`. |
| `delAdj` | `0101 1001` | 1 | E = target strength | **Yes** | Delete instruction at `neighbor[Dst mod size]`. Target program size decreases by 1. Freed mass (1) goes to **this** cell (the attacker). |
| `senseSize` | `0101 1010` | 0 | — | No | Push program size of adjacent cell (0 if empty). |
| `senseE` | `0101 1011` | 0 | — | No | Push free energy of adjacent cell. |
| `senseM` | `0101 1100` | 0 | — | No | Push free mass of adjacent cell. |
| `giveE` | `0101 1101` | 0 | — | No | Transfer half of own free energy (rounded up) to adjacent cell. |
| `giveM` | `0101 1110` | 1 | — | No | Transfer half of own free mass (rounded up) to adjacent cell. |
| `takeE` | `0101 1111` | 1 | E = target strength | **Yes** | Take half of target's free energy (rounded up). |
| `takeM` | `0110 0000` | 1 | — | **Yes** | Take half of free mass (rounded up) from adjacent cell. Takes only loose mass, not instructions. |
| `move` | `0110 0001` | 1 | — | No | Relocate this program and all its resources (free energy, free mass) to the adjacent cell in direction `Dir`. Fails if the target cell is occupied. Target must be empty (empty cells are always open, so protection is not applicable). |

#### Instruction Deletion Semantics

When an instruction at index `i` is deleted from a program (via `del` or `delAdj`):

- All instructions after index `i` shift down by 1.
- If the affected program's `IP > i`, decrement `IP` by 1. This prevents the program from skipping an instruction or executing past its end.
- `Src` and `Dst` are **not** adjusted. These are raw cursor values with no fixed association to self or neighbor — they are interpreted modulo program size at the point of use. If a deletion shifts instructions around, the cursor may land on a different instruction next time it is used. This is a natural consequence of code disruption, not a bug.
- If the program reaches size 0, it is removed and the cell becomes empty.

### Instruction Count Summary

| Category | Count |
|----------|-------|
| Push literals | 16 |
| Stack operations | 5 |
| Arithmetic / logic | 9 |
| Control flow | 5 |
| Direction + registers | 13 |
| World interaction (local) | 6 |
| World interaction (nonlocal) | 12 |
| **Total opcodes** | **66** |
| No-op byte values | **190** (74% of opcode space) |

## Mutations

### Mutation Rates

- After any 1-tick instruction is executed, there is a 1 in 2^16 probability that the instruction mutates. A mutation consists of a random bit flip in its 8-bit opcode. The mutation does not affect the current tick's execution.
- If the program had 0 free energy and paid the base cost using background radiation, the mutation probability increases to `min(x/256, 1)` where `x` is the amount of background radiation present in the cell.

### Background Radiation to Mass

When excess background radiation is removed during the physics update, it converts to mass with probability 1/256. If no program exists in the cell, a new single-`nop` program is created (age 0, default registers). Otherwise, 1 free mass is added.

## Seed Replicator

The following 10-instruction program is the recommended initial organism:

```
; === Seed Replicator (10 instructions) ===
;
absorb          ; Tick 1:  gather energy (opens cell — vulnerability tradeoff)
takeM           ; Tick 2:  forage free mass from neighbor
cw              ;          rotate Dir (covers different neighbors over cycles)
push 0          ;
setSrc          ;          reset Src to 0
getSize         ;          push program size (10) for loop count
for             ;          begin loop
  read          ; Tick 3+: read self[Src], Src++
  appendAdj     ; Tick 4+: append to neighbor in Dir
next            ;          decrement LC, loop if > 0
;
; After the loop, IP wraps to 0 and the cycle repeats.
;
; Timing:  2 + (2 × 10) = 22 ticks per replication cycle
; Energy:  1 (takeM) + 10 (appendAdj base costs) = 11 energy
; Mass:    10 (one per appended instruction)
;
; The offspring receives no energy or mass. It will begin executing
; its own code on the next tick, starting with absorb.
```

### Why This Replicator is Plausible as a First Evolver

The seed replicator uses four distinct 1-tick instructions (`absorb`, `takeM`, `read`, `appendAdj`) and six immediate instructions (`cw`, `push`, `setSrc`, `getSize`, `for`, `next`). It requires no knowledge of the target cell's state — `appendAdj` works on both empty and occupied cells. The `Dst` register is never used. The only cursor management is resetting `Src` to 0 each cycle.

The probability of this sequence emerging from random noise is extremely low. Proteus v0 uses manual seeding of the seed replicator to bypass the origin-of-life bottleneck and focus on evolutionary dynamics.

### Evolutionary Optimization Opportunities

The seed replicator is intentionally unoptimized. Evolution can discover improvements such as:

- **Energy gating**: check energy/mass levels before attempting replication to avoid wasting resources on failed writes
- **Offspring provisioning**: `giveE`/`giveM` after replication to help offspring survive
- **Directional awareness**: `senseSize` to find empty cells before choosing replication direction
- **Predation**: `delAdj` to consume neighbor instructions as mass, `takeE` to steal energy (requires target to be open)
- **Defense**: minimize time spent in open states (`absorb`, `nop`) to reduce vulnerability to `delAdj`, `takeE`, and `takeM`
- **Communication**: `emit`/`absorb` to coordinate with kin
- **Code injection**: `writeAdj` to modify neighbor programs (parasitism, mutualism)
- **Streamlining**: eliminate unnecessary instructions to reduce maintenance cost and replication time
- **Movement**: `move` to relocate toward resources or away from threats

## Implementation Notes

### Snapshot-and-Apply Pattern

Pass 2 (nonlocal execution) uses a snapshot-and-apply pattern to ensure translation invariance and avoid order-dependent results:

1. After Pass 1 completes, take a logical snapshot of the grid state.
2. Resolve all nonlocal instruction conflicts (grouping by target cell, applying the strength → size → age → random tiebreaker).
3. Each winning instruction reads from the snapshot and writes to its target cell. Source-side effects are applied unconditionally for each winner.
4. Since each target cell has at most one winner, all writes target distinct cells and can be applied in any order (or simultaneously). Since each cell executes at most one nonlocal instruction per tick, source-side effects never conflict.

In practice, this does not require copying the entire grid. Most cells are not targeted on any given tick. An implementation can:

- Collect all nonlocal instructions into a sparse list.
- Group by target cell (a sort or hash map).
- Resolve each group independently (trivially parallel).
- Apply changes. Only cells that are targeted need snapshotted values, which can be saved when the instruction is queued.

### Parallelization

The execution model is designed for parallelism:

- **Pass 1** is embarrassingly parallel: each cell executes independently with no inter-cell communication.
- **Pass 2** conflict resolution is independent per target cell and trivially parallel after grouping.
- **Pass 3** physics update is a local stencil operation (radiation propagation) plus per-cell decay, both parallel.

No grid coloring, checkerboard decomposition, or symmetry-breaking structure is needed. The snapshot-and-apply pattern eliminates data races and preserves translation invariance.

### Grid Boundary

For finite grids, use periodic (toroidal) boundary conditions to maintain translation invariance. Directed radiation wraps around grid edges.

## Future Directions (post-v0)

These features are intentionally deferred. They may be added if the v0 ecosystem demonstrates a need or reaches a complexity plateau.

- **Plasmids**: modular code organization, enabling horizontal gene transfer
- **Insert instruction**: insert (rather than overwrite) at a target position, shifting subsequent instructions
- **Evolvable instruction sets**: organisms define their own opcode-to-behavior mapping
- **Environmental variation**: spatially and temporally varying background radiation intensity (e.g., 3D simplex noise over x, y, t)
- **Labels**: named jump targets for more structured control flow
- **mul/div/mod**: complex arithmetic (currently achievable via loops)
- **Trap instruction**: active defense mechanism (capture or redirect incoming writes)
- **Function calls**: call stack for subroutine reuse
- **Long-range sensing**: detect programs or energy gradients beyond adjacent cells
- **Multicellular structures**: extended-body organisms spanning multiple cells
