# Proteus v0.1 Specification

An artificial life simulator where self-replicating programs emerge, compete, and evolve on a 2D grid with conserved mass and energy.

## Design Philosophy

Proteus v0 prioritizes a minimal substrate that enables emergent complexity. The instruction set is small enough that most single-point mutations produce functional programs, and the minimal self-replicator is short enough that evolution has a smooth fitness landscape to explore. Complex behaviors — movement, predation, cooperation, communication — emerge from generic read/write primitives rather than dedicated opcodes.

The physics layer provides real resource constraints (spatial locality, maintenance costs, conserved transactions with external energy and mass sources) that drive ecological dynamics without prescribing what those dynamics should look like. The substrate includes controlled stochastic elements — background radiation and mass arrival, mutation, `rand`, and probabilistic maintenance and decay — but is otherwise deterministic. All physics is fully discrete and translation-invariant under 90° rotations and reflections.

## Physics

### Grid

- 2D space with square cells, discrete time steps ("ticks")
- Spatial interactions between adjacent cells only (no diagonal; 4-connected grid)
- Speed of light: 1 cell/tick for information propagation (directed radiation)
- Physical laws invariant under 90° rotations and reflections; no preferred grid position

### Mass and Energy

Mass and energy are fundamental, quantized quantities located within a single cell at a time. The system has conserved internal transactions, with external inputs of both energy and mass driving the economy.

**Mass** exists in two forms:

- **Free mass**: loose raw material in a cell. Used by programs to create new instructions. Attached to the program if one is present.
- **Instructions**: each instruction in a program has mass 1. Instructions are created from free mass and can be destroyed back into free mass.

**Energy** exists in three forms:

- **Free energy**: stationary, attached to the program in a cell (if present). Used to pay for instruction execution and maintenance.
- **Directed radiation**: a packet of energy propagating at 1 cell/tick in a cardinal direction. Carries a 16-bit signed integer value, doubling as energy transfer and long-range communication.
- **Background radiation**: an external energy input. Each cell receives 1 unit of background radiation with probability `R_energy` per tick. Background radiation accumulates in the cell across ticks until captured by `absorb`, but each unit independently decays (is permanently removed) with probability `D_energy` per tick. At steady state, an untouched cell holds approximately `R_energy / D_energy` background radiation. Background radiation is a separate pool from free energy — it cannot be used to pay for instructions (except as emergency payment with elevated mutation risk; see Instruction Timing) and is not subject to storage thresholds. Only `absorb` converts background radiation into free energy.

**Background mass**: an external mass input, independent of energy. Each cell receives 1 unit of free mass with probability `R_mass` per tick. When mass arrives in a cell with no program, it has an additional probability `P_spawn` of nucleating a new single-`nop` program (see Spontaneous Program Creation).

### Decay

Free energy and free mass above a storage threshold decay stochastically each tick. For a cell containing a program of size `S`:

- **Storage threshold**: `T_cap × S` for both energy and mass, where `T_cap` is a system parameter.
- **Decay rule**: each unit of free energy above the threshold independently decays (is permanently removed) with probability `D_energy` per tick. Each unit of free mass above the threshold independently decays with probability `D_mass` per tick.
- **Empty cells** have a storage threshold of 0 — all free resources decay.

Resources at or below the threshold do not decay. Implementation: for each cell, compute `excess = max(0, amount - threshold)`, then draw from `Binomial(excess, D)` to determine units removed.

### Program Size Cap

Program size is hard-capped at 2^15 − 1 instructions.

### Maintenance

Each instruction in a **live** program independently requires 1 energy payment with probability `M` per tick. Inert programs use the same maintenance rule only **after** they are considered abandoned (see Program Lifecycle). Implementation: draw from `Binomial(program_size, maintenance_rate)` where `maintenance_rate` is `M` for live programs, `0` for inert programs still inside the grace window, and `M` again for inert programs that have exceeded the grace window.

If free energy is insufficient, maintenance is paid with free mass. If both are exhausted, instructions are destroyed from the end of the program.

### Program Strength

A program's **strength** is `min(program_size, free_energy)`. Strength determines the cost of hostile actions targeting the program.

### Program Age

Each live program tracks its **age**: the number of ticks since it became live. Age is initialized to 0 when a program becomes live (either by spontaneous creation or by being booted). Inert programs do not age. Age is not currently used mechanically but is tracked for diagnostic and analytical purposes.

### Program Lifecycle

Programs exist in two states:

- **Live**: the program executes, pays maintenance, ages, and its cell follows normal protection rules.
- **Inert**: the program does not execute and does not age. Its cell is **always open**. While inert, maintenance depends on whether the program is still under active construction. If maintenance destroys all instructions, the program is removed and the cell becomes empty.

Each inert program tracks **ticks since last incoming write**. Any successful `appendAdj` or `writeAdj` targeting the inert program resets this timer to 0. While this timer is below `inert_grace_ticks`, the offspring is considered **under active construction** and pays no maintenance. Once the timer reaches `inert_grace_ticks`, the offspring is considered **abandoned** and normal maintenance resumes.

Rationale: this keeps cleanup endogenous. Offspring under active parental construction are protected from immediate tail erosion, but abandoned fragments still die through the same maintenance-and-starvation process as every other program. No forced activation or out-of-band dissolution is required.

**Spontaneous creation** (background mass nucleation in an empty cell): the program is created **live**. This is the primordial bootstrap — no parent required.

**Constructed creation** (first `appendAdj` into an empty cell): the program is created **inert**. It remains inert until a `boot` instruction transitions it to live. While inert, the cell is open, allowing continued construction by the parent or interference by others.

### Spontaneous Program Creation

When background mass arrives in a cell with no program, there is a probability `P_spawn` that a new live program is created instead of simply adding free mass. The program consists of a single `nop` instruction with default registers (`Dir` and `ID` randomized), empty stack, and age 0. Free energy and mass in the cell are unchanged.

### Synthesis

Programs can actively convert energy into mass using the `synthesize` instruction. This consumes `N_synth` energy (in addition to the instruction's base cost of 1) and produces 1 free mass. This is the primary metabolic pathway — organisms convert energy surplus into building material for replication.

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
| `Flag` | 1 | read-only | 0 | 0 = last 1-tick instruction succeeded; 1 = any instruction failed or message received via `absorb`. Only successful 1-tick instruction execution resets `Flag` to 0; immediate instructions can set `Flag` to 1 (on failure) but never clear it. |
| `Msg` | 16 | read-only | 0 | Value carried by last received directed radiation. |
| `ID` | 8 | read-write | random | Cell identifier. Initialized with randomness at program creation. |
| `LC` | 16 | hidden | 0 | Loop counter, used internally by `for`/`next`. Not directly accessible. |

### Newborn Program State

When a new program is created (first `appendAdj` into an empty cell, or spontaneous `nop` spawn), all registers are initialized to their default values as listed above. `Dir` and `ID` are randomized independently. The stack is empty. Age is 0. Free energy and free mass in the cell are unchanged (they belong to the cell, not the program).

Programs created by `appendAdj` into an empty cell are **inert** (see Program Lifecycle). Programs created by spontaneous nop-spawn are **live**.

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
- Is **inert** (always open until booted)

**Empty cells are always open.**

The following instructions fail against a protected target cell:

- `delAdj` — fails against a protected cell
- `takeE` — fails against a protected cell
- `takeM` — fails against a protected cell
- `writeAdj` — fails against a protected cell
- `appendAdj` (on occupied cell) — fails against a protected cell

The following nonlocal instructions work regardless of protection status:

- `readAdj`, `senseSize`, `senseE`, `senseM`, `senseID` — read-only, non-destructive
- `giveE`, `giveM` — beneficial to target
- `move` — targets empty cells only, which are always open
- `appendAdj` (into empty cell) — empty cells are always open
- `boot` — targets inert programs only (which are always open); fails if target is empty or already live

Rationale: protection is uniform — a protected cell is genuinely protected against all hostile actions including code injection. Multi-tick replication is enabled by the inert offspring mechanism: offspring are inert (and therefore open) until explicitly booted, allowing the parent to write their full genome before activation.

### Global Execution Order

Each tick proceeds in three passes. All physics is translation-invariant.

**Pass 1 — Local execution (all cells simultaneously):**

Only **live** programs execute. Inert programs are skipped entirely.

1. Each program executes immediate instructions until reaching a 1-tick instruction, or until the immediate instruction limit (equal to program size) is exceeded, in which case the program is halted and the cell becomes open.
2. If a 1-tick instruction is reached, pay its base energy cost. If payment fails, halt, do not advance `IP`, and mark cell as open. If paid, determine whether the instruction is local or nonlocal.
3. Local 1-tick instructions execute immediately.
4. Nonlocal 1-tick instructions are placed in the nonlocal queue for Pass 2.
5. `IP` advances by 1. The executed instruction has a chance to mutate (not affecting this tick's execution).

Pass 1 requires no inter-cell communication and is trivially parallelizable.

**Pass 2 — Nonlocal execution (snapshot-and-apply):**

All nonlocal instructions are resolved against a snapshot of the grid state taken after Pass 1.

1. Pay additional costs for each nonlocal instruction. If costs cannot be paid, the instruction fails.
2. Check protection on target cells. `delAdj`, `takeE`, `takeM`, `writeAdj`, and `appendAdj` (on occupied cell) targeting a protected cell fail. `boot` targeting an empty or already-live cell fails.
3. Group remaining instructions by target cell. For each target cell:
   - If one instruction targets it: that instruction succeeds.
   - If multiple instructions target it: resolve using the conflict resolution rule (see below).
4. Execute all winning instructions simultaneously against the snapshot. Each winner reads pre-resolution state and writes to its target cell. Since each target cell has at most one winner, there are no write conflicts.
5. Source-side effects (e.g., mass/energy deducted from the attacking cell, cursor increments) are applied unconditionally for each winning instruction. Since each cell executes at most one nonlocal instruction per tick, source-side effects never conflict with each other.

**Conflict resolution** (multiple instructions targeting the same cell):

1. Highest program strength wins.
2. Ties: random, weighted by program size (each tied program's probability of winning is proportional to its size).

This rule is deterministic only at tier 1. At equal strength, larger programs have better odds but are not guaranteed to win, preventing pure incumbency advantages while still rewarding investment in size. The rule requires only source-program properties, making it well-defined for all target cells including empty ones.

Note: mutual targeting (A targets B while B targets A) does not require special handling. These instructions target different cells and both can succeed independently. No conflict graph or cycle detection is needed.

**Pass 3 — Physics update:**

1. **Directed radiation** propagates to its next cell. When multiple packets of directed radiation arrive in the same cell simultaneously, all are converted to free energy.
2. **Background radiation**: each cell receives 1 unit with probability `R_energy`. This accumulates in the cell. Each existing unit of background radiation independently decays with probability `D_energy`.
3. **Absorb resolution**: for each cell containing accumulated background radiation, distribute it equally (floor division) among all programs that executed `absorb` this tick and are adjacent to or occupying that cell. Remainder stays in the cell. Background radiation captured this way becomes free energy in the absorbing program's cell.
4. **Background mass**: each cell receives 1 free mass with probability `R_mass`. If the cell has no program, the arriving mass has an additional probability `P_spawn` of nucleating a new live `nop` program (see Spontaneous Program Creation).
5. **Inert lifecycle update**: for each inert program, if it received an incoming `appendAdj` or `writeAdj` this tick, reset its abandonment timer to 0. Otherwise increment the timer by 1. This determines whether the offspring is still inside its grace window (`inert_ticks_without_write < inert_grace_ticks`) or has become abandoned.
6. **Maintenance**: for each program, draw from `Binomial(program_size, maintenance_rate)` where `maintenance_rate = M` for live programs, `0` for inert programs still inside the grace window, and `M` for inert programs that have exceeded the grace window. Deduct from free energy, then free mass, then instructions from end of program.
7. **Decay**: for each cell, compute excess energy and mass above the storage threshold (`T_cap × program_size`, or 0 for empty cells). For each resource, draw from `Binomial(excess, D)` to determine units removed permanently.
8. All live program ages increment by 1.

## Instruction Set

71 opcodes out of 256 possible byte values. All other byte values are no-ops: immediate, free, and they do **not** open the cell. They are simply skipped during execution.

### Encoding

| Range | Category | Count |
|-------|----------|-------|
| `0000 xxxx` | Push literals (−8 to +7) | 16 |
| `0001 0000` – `0001 0100` | Stack operations | 5 |
| `0010 0000` – `0010 1000` | Arithmetic / logic | 9 |
| `0011 0000` – `0011 0100` | Control flow | 5 |
| `0100 0000` – `0100 1110` | Direction, register, and local resource access | 15 |
| `0101 0000` – `0110 0100` | World interaction | 21 |
| All other values | No-op (immediate, free, does not open cell) | 185 |

The 72% no-op space provides generous neutral territory for mutations: most random bit flips land on no-ops, providing genetic drift without lethality.

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

### Control Flow (5 opcodes, immediate)

| Instruction | Opcode | Description |
|-------------|--------|-------------|
| `for` | `0011 0000` | Pop count from stack into `LC`. If `LC` ≤ 0, scan forward (modulo program size) for the next `next` instruction and skip past it. The scan counts as a single immediate instruction regardless of distance scanned. If no `next` is found after a full wrap, set `Flag` to 1 and continue. |
| `next` | `0011 0001` | Decrement `LC`. If `LC` > 0, jump to instruction after the matching `for` (found by scanning backward modulo program size for the nearest `for`). If `LC` ≤ 0, continue. Unmatched `next` is a no-op. |
| `jmp` | `0011 0010` | Pop offset from stack. Jump to `IP + offset`. |
| `jmpNZ` | `0011 0011` | Pop value, then pop offset. If value ≠ 0, jump to `IP + offset`. |
| `jmpZ` | `0011 0100` | Pop value, then pop offset. If value = 0, jump to `IP + offset`. |

`for`/`next` do not nest. There is a single `LC` register. A `for` matches the next `next` found by forward scan; a `next` matches the nearest `for` found by backward scan. Both scans wrap modulo program size. This is a deliberate v0 simplification.

### Direction, Register, and Local Resource Access (15 opcodes, immediate)

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

### World Interaction (21 opcodes, 1-tick)

#### Local (target self)

| Instruction | Opcode | Base Cost | Add'l Cost | Description |
|-------------|--------|-----------|------------|-------------|
| `nop` | `0101 0000` | 0 | — | No operation. **Opens cell.** |
| `absorb` | `0101 0001` | 0 | — | Mark this program as absorbing for Pass 3 background radiation distribution. Capture all directed radiation in this cell as free energy. If directed radiation was received: set `Msg` to its value, set `Dir` to the direction it arrived from, set `Flag` to 1. **Opens cell.** |
| `emit` | `0101 0010` | 1 | — | Pop value from stack. Send directed radiation carrying that value in direction `Dir`. |
| `read` | `0101 0011` | 0 | — | Push instruction at `self[Src mod size]` onto stack. Increment `Src`. |
| `write` | `0101 0100` | 1 | — | Pop value. Overwrite instruction at `self[Dst mod size]` with value (low 8 bits). Increment `Dst`. Does not change program size. |
| `del` | `0101 0101` | 1 | — | Delete instruction at `self[Dst mod size]`. Program size decreases by 1. Freed as 1 free mass in this cell. |
| `synthesize` | `0101 0110` | 1 | E = `N_synth` | Convert energy to mass. Consumes `N_synth` additional free energy and produces 1 free mass in this cell. Fails (sets `Flag`) if insufficient energy. |

#### Nonlocal (target adjacent cell in direction `Dir`)

| Instruction | Opcode | Base Cost | Add'l Cost | Protection | Description |
|-------------|--------|-----------|------------|------------|-------------|
| `readAdj` | `0101 0111` | 0 | — | No | Push instruction at `neighbor[Src mod size]` onto stack. Increment `Src`. If neighbor cell is empty, push 0 and set `Flag` to 1. |
| `writeAdj` | `0101 1000` | 1 | — | **Yes** | Pop value. Overwrite instruction at `neighbor[Dst mod size]` (low 8 bits). The old instruction is recycled (no net mass cost). Increment `Dst`. Fails if neighbor cell is empty (nothing to overwrite). |
| `appendAdj` | `0101 1001` | 1 | 1 mass | **Yes** (occupied) | Pop value. Append instruction (low 8 bits) to end of neighbor's program (or create new inert program if cell is empty). Costs 1 free mass. Does not use or modify `Dst`. Checks protection only if the target cell is occupied; empty cells are always open. |
| `delAdj` | `0101 1010` | 1 | E = target strength | **Yes** | Delete instruction at `neighbor[Dst mod size]`. Target program size decreases by 1. Freed mass (1) goes to **this** cell (the attacker). |
| `senseSize` | `0101 1011` | 0 | — | No | Push program size of adjacent cell (0 if empty). |
| `senseE` | `0101 1100` | 0 | — | No | Push free energy of adjacent cell. |
| `senseM` | `0101 1101` | 0 | — | No | Push free mass of adjacent cell. |
| `senseID` | `0101 1110` | 0 | — | No | Push `ID` register of adjacent cell's program (0 if empty, sets `Flag` to 1). |
| `giveE` | `0101 1111` | 0 | — | No | Pop amount. Transfer `min(amount, free_energy)` to adjacent cell. If amount ≤ 0, no transfer. |
| `giveM` | `0110 0000` | 1 | — | No | Pop amount. Transfer `min(amount, free_mass)` to adjacent cell. If amount ≤ 0, no transfer. |
| `takeE` | `0110 0001` | 1 | E = target strength | **Yes** | Take half of target's free energy (rounded up). |
| `takeM` | `0110 0010` | 1 | — | **Yes** | Take half of free mass (rounded up) from adjacent cell. Takes only loose mass, not instructions. |
| `move` | `0110 0011` | 1 | — | No | Relocate this program and all its resources (free energy, free mass) to the adjacent cell in direction `Dir`. Fails if the target cell is occupied. Target must be empty (empty cells are always open, so protection is not applicable). |
| `boot` | `0110 0100` | 0 | — | No | Transition an inert program in the adjacent cell (direction `Dir`) to live. The target begins executing on the next tick with `IP` = 0. Fails if the target cell is empty, or if the target program is already live. Inert cells are always open, so protection is not applicable. Subject to conflict resolution if multiple programs attempt to boot the same target. |

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
| Direction + registers + local resource access | 15 |
| World interaction (local) | 7 |
| World interaction (nonlocal) | 14 |
| **Total opcodes** | **71** |
| No-op byte values | **185** (72% of opcode space) |

## Mutations

### Mutation Rates

- After any 1-tick instruction is executed, the instruction mutates with probability `2^(-mutation_base_log2)`. A mutation consists of a random bit flip in its 8-bit opcode. The mutation does not affect the current tick's execution.
- If the program had 0 free energy and paid the base cost using background radiation present in its cell, the mutation probability increases to `min(x / 2^(mutation_background_log2), 1)` where `x` is the amount of background radiation present in the cell at payment time.

## System Parameters

All parameters governing external input rates, decay, maintenance, and synthesis are collected here for tuning.

| Parameter | Symbol | Description | Suggested Start |
|-----------|--------|-------------|-----------------|
| Energy arrival rate | `R_energy` | P(cell receives 1 background radiation per tick) | 0.25 |
| Mass arrival rate | `R_mass` | P(cell receives 1 free mass per tick) | 0.05 |
| Nop-spawn probability | `P_spawn` | P(mass arrival in empty cell nucleates live nop) | 0 |
| Energy decay rate | `D_energy` | P(each unit of background radiation or excess free energy removed per tick) | 0.01 |
| Mass decay rate | `D_mass` | P(each excess mass unit removed per tick) | 0.01 |
| Decay threshold | `T_cap` | Multiplier on program_size for decay floor | 4 |
| Maintenance rate | `M` | P(each live instruction costs 1 energy per tick) | 1/128 |
| Inert grace window | `inert_grace_ticks` | Ticks without incoming write before an inert offspring starts paying normal maintenance | 10 |
| Synthesis cost | `N_synth` | Additional energy consumed per mass produced | 1 |
| Baseline mutation exponent | `mutation_base_log2` | Baseline mutation rate is `2^(-value)` | 16 |
| Background mutation exponent | `mutation_background_log2` | Background-paid mutation rate is `min(x / 2^(value), 1)` | 8 |

## Seed Replicator

The following program is the recommended initial organism:

```
; === Seed Replicator (11 instructions) ===
;
absorb          ; Tick 1:  gather energy (opens cell — vulnerability tradeoff)
takeM           ; Tick 2:  forage free mass from neighbor
cw              ;          rotate Dir (covers different neighbors over cycles)
push 0          ;
setSrc          ;          reset Src to 0
getSize         ;          push program size (11) for loop count
for             ;          begin loop
  read          ; Ticks 3+: read self[Src], Src++
  appendAdj     ; Ticks 4+: append to neighbor in Dir
next            ;          decrement LC, loop if > 0
boot            ; Tick 25: activate offspring
;
; After the loop, IP wraps to 0 and the cycle repeats.
;
; Timing:  2 + (2 × 11) + 1 = 25 ticks per replication cycle
; Energy:  1 (takeM) + 11 (appendAdj base costs) = 12 energy per cycle
; Mass:    11 (one per appended instruction)
;          sources: foraged from neighbor + ambient in own cell
;
; The offspring is inert during construction (open, no execution,
; and no maintenance while the parent keeps writing to it). It becomes live when boot executes, starting
; with absorb on the next tick.
```

### Initial Conditions

The seed organism should be placed in a pre-loaded environment: the seed cell and its neighbors should contain sufficient free mass (≥ 11) and free energy (≥ 20) so that the first replication cycle succeeds. This models a resource-rich primordial environment. Once the first generation of offspring begins absorbing energy and foraging, the population becomes self-sustaining at appropriate parameter settings.

If replication fails mid-cycle (mass or energy exhausted), the `appendAdj` calls fail silently. A later explicit `boot` may activate a partial offspring. If no further writes arrive, the abandoned offspring remains inert but eventually starts paying normal maintenance after `inert_grace_ticks` ticks, so its tail erodes away naturally unless construction resumes.

### Why This Replicator is Plausible as a First Evolver

The seed replicator uses four distinct 1-tick instructions (`absorb`, `takeM`, `read`, `appendAdj`, plus `boot` — five total) and six immediate instructions (`cw`, `push`, `setSrc`, `getSize`, `for`, `next`). The offspring is inert during construction, preventing premature execution of partially written code. The `Dst` register is never used. The only cursor management is resetting `Src` to 0 each cycle.

The seed does not check resource levels before attempting replication, does not provision offspring with energy, and does not use `synthesize` to manufacture mass. These are all evolutionary optimization opportunities.

The probability of this sequence emerging from random noise is extremely low. Proteus v0 uses manual seeding of the seed replicator to bypass the origin-of-life bottleneck and focus on evolutionary dynamics.

### Evolutionary Optimization Opportunities

The seed replicator is intentionally unoptimized. Evolution can discover improvements such as:

- **Resource gating**: check energy/mass levels before attempting replication to avoid wasting resources on failed writes
- **Synthesis**: use `synthesize` to convert energy surplus into mass, reducing dependence on foraging
- **Multi-cycle foraging**: loop `absorb`/`takeM`/`synthesize` multiple times to accumulate mass before replicating
- **Offspring provisioning**: `giveE`/`giveM` after boot to help offspring survive and accelerate early post-boot growth
- **Directional awareness**: `senseSize` to find empty cells before choosing replication direction
- **Predation**: `delAdj` to consume neighbor instructions as mass, `takeE` to steal energy (requires target to be open)
- **Defense**: minimize time spent in open states (`absorb`, `nop`) to reduce vulnerability
- **Communication**: `emit`/`absorb` to coordinate with kin
- **Kin recognition**: `senseID` to distinguish kin from non-kin, enabling cooperation and altruism
- **Code injection**: `writeAdj` to modify open neighbor programs (parasitism, mutualism)
- **Streamlining**: eliminate unnecessary instructions to reduce maintenance cost and replication time
- **Movement**: `move` to relocate toward resources or away from threats

## Reference Energy/Mass Budget

The following parametric equations describe the per-cycle resource budget for a reference organism. These support analytical parameter exploration before simulation.

Let `S` = program size (instructions), `T` = ticks per replication cycle, `K` = mass produced via `synthesize` per cycle (0 for the seed).

### Energy Income

After an `absorb` drains a cell's background radiation to 0, it refills over `T` ticks accounting for ongoing decay:

```
refill(T) = R_energy × (1 − (1 − D_energy)^T) / D_energy
```

This approaches the steady-state value `R_energy / D_energy` for large `T`. A solitary absorber drains its own cell plus 4 neighbors:

```
E_in = C_absorb × refill(T)
```

Where `C_absorb` = cells contributing (up to 5 for a solitary organism).

### Energy Costs

```
E_maintain  = T × S × M                            (expected maintenance)
E_instruct  = sum of base costs per cycle           (1-tick instruction base costs)
E_synth     = K × (N_synth + 1)                    (synthesis base + additional cost)
E_decay     ≈ negligible at low stockpiles          (stochastic, hard to compute exactly)
```

### Viability Condition (energy)

```
E_in  >  E_maintain + E_instruct + E_synth + E_decay
```

With margin — the organism needs surplus energy to survive variance and occasional failed cycles.

### Mass Income

For an empty neighbor cell drained to 0 by `takeM`, mass refills accounting for decay (empty cells have threshold 0, so all mass decays):

```
neighbor_refill(T) = R_mass × (1 − (1 − D_mass)^T) / D_mass
```

For the organism's own cell with threshold `T_cap × S`, ambient mass below threshold does not decay:

```
own_cell(T) = T × R_mass    (if current mass stays below threshold)
```

```
M_forage  = neighbor_refill(T) / 2                 (takeM takes half, one neighbor per cycle)
M_synth   = K                                       (from synthesize, if used)
M_ambient = own_cell(T)                             (accumulates in own cell between cycles)
```

### Mass Cost

```
M_out = S                                           (one per appended instruction in offspring)
M_offspring_maint ≈ max(0, T_construct − inert_grace_ticks) × S_avg × M
```

Where `T_construct` is ticks the offspring exists while inert, and `S_avg` is its average size during construction. If the parent keeps writing frequently enough that construction stays inside the grace window, this term is effectively 0. It becomes relevant only for long stalls or abandoned fragments.

### Viability Condition (mass)

```
M_forage + M_synth + M_ambient  ≥  S
```

### Example: Seed Replicator at Current Default Parameters

With `R_energy = 0.25`, `R_mass = 0.05`, `D_energy = D_mass = 0.01`, `M = 1/128`, `S = 11`, `T = 25`, solo absorber (`C_absorb = 5`), no synthesis (`K = 0`):

```
refill(25) = 0.25 × (1 − 0.99^25) / 0.01 = 0.25 × 22.2 = 5.55

E_in       = 5 × 5.55                = 27.8
E_maintain = 25 × 11 × (1/128)       =  2.1
E_instruct = 1 (takeM) + 11 (appendAdj) + 0 (boot) = 12.0
E_total    = 14.1

Energy surplus: 13.7 per cycle. Comfortable.

neighbor_refill(25) = 0.05 × (1 − 0.99^25) / 0.01 = 0.05 × 22.2 = 1.11
M_forage   = 1.11 / 2                = 0.56   (half of one neighbor)
M_ambient  = 25 × 0.05               = 1.25
M_total    = 1.81 per cycle

Need: 11. Shortfall of ~9.2 per cycle.
```

**The seed cannot self-replicate in a single cycle from ambient resources alone.** It needs either pre-loaded mass (see Initial Conditions) or multiple foraging cycles. An evolved organism that accumulates mass across several absorb/takeM cycles before replicating, and uses `synthesize` to convert its energy surplus (~13.7/cycle) into additional mass (~6.8 mass at `N_synth = 1`), reaches ~8.6 mass per cycle — viable replication roughly every 2 cycles.

This is intentional. The seed is a bootstrap organism, not a steady-state design. Evolution's immediate pressure is toward resource gating and metabolic efficiency.

## Implementation Notes

### Snapshot-and-Apply Pattern

Pass 2 (nonlocal execution) uses a snapshot-and-apply pattern to ensure translation invariance and avoid order-dependent results:

1. After Pass 1 completes, take a logical snapshot of the grid state.
2. Resolve all nonlocal instruction conflicts (grouping by target cell, applying the strength → size-weighted random tiebreaker).
3. Each winning instruction reads from the snapshot and writes to its target cell. Source-side effects are applied unconditionally for each winner.
4. Since each target cell has at most one winner, all writes target distinct cells and can be applied in any order (or simultaneously). Since each cell executes at most one nonlocal instruction per tick, source-side effects never conflict.

In practice, this does not require copying the entire grid. Most cells are not targeted on any given tick. An implementation can:

- Collect all nonlocal instructions into a sparse list.
- Group by target cell (a sort or hash map).
- Resolve each group independently (trivially parallel).
- Apply changes. Only cells that are targeted need snapshotted values, which can be saved when the instruction is queued.

### Stochastic Implementation

Several mechanics use per-quantum independent probabilities, requiring efficient binomial sampling:

- **Background radiation decay**: `Binomial(bg_radiation, D_energy)` per cell per tick.
- **Free energy/mass decay**: `Binomial(excess, D_energy)` and `Binomial(excess, D_mass)` per cell per tick.
- **Maintenance**: `Binomial(program_size, M)` per program (live and inert) per tick.
- **Background input**: `Bernoulli(R_energy)` and `Bernoulli(R_mass)` per cell per tick.

For small counts and low probabilities, direct Bernoulli trials are efficient. For large counts, precomputed binomial lookup tables or fast approximations (e.g., normal approximation with continuity correction for large `n`) are recommended. A single high-quality PRNG (e.g., xoshiro256++) seeded deterministically provides reproducible simulations.

### Parallelization

The execution model is designed for parallelism:

- **Pass 1** is embarrassingly parallel: each cell executes independently with no inter-cell communication.
- **Pass 2** conflict resolution is independent per target cell and trivially parallel after grouping.
- **Pass 3** physics update is a local stencil operation (radiation propagation, absorb distribution) plus per-cell decay and maintenance, both parallel. Absorb distribution requires knowing which programs executed `absorb` in Pass 1, which is available from the Pass 1 results.

No grid coloring, checkerboard decomposition, or symmetry-breaking structure is needed. The snapshot-and-apply pattern eliminates data races and preserves translation invariance.

### Grid Boundary

For finite grids, use periodic (toroidal) boundary conditions to maintain translation invariance. Directed radiation wraps around grid edges.

## Future Directions (post-v0)

These features are intentionally deferred. They may be added if the v0 ecosystem demonstrates a need or reaches a complexity plateau.

- **Plasmids**: modular code organization, enabling horizontal gene transfer
- **Insert instruction**: insert (rather than overwrite) at a target position, shifting subsequent instructions
- **Evolvable instruction sets**: organisms define their own opcode-to-behavior mapping
- **Environmental variation**: spatially and temporally varying background radiation/mass intensity (e.g., 3D simplex noise over x, y, t)
- **Labels**: named jump targets for more structured control flow
- **mul/div/mod**: complex arithmetic (currently achievable via loops)
- **Trap instruction**: active defense mechanism (capture or redirect incoming writes)
- **Function calls**: call stack for subroutine reuse
- **Long-range sensing**: detect programs or energy gradients beyond adjacent cells
- **Multicellular structures**: extended-body organisms spanning multiple cells
- **Nested loops**: multiple `LC` registers for nested `for`/`next`

## Known Limitations (monitor in simulation)

These are identified design tensions in v0 that may or may not require intervention. They should be monitored during simulation rather than fixed preemptively.

- **`absorb` coupling**: `absorb` simultaneously captures background energy, captures directed radiation, sets `Msg`, overwrites `Dir`, sets `Flag`, and opens the cell. A program that needs energy must accept messages and lose its heading. If communication fails to emerge, or emerges only as a nuisance (parasitic `Dir` resets disrupting directional replication), consider splitting into separate `absorb` (energy only) and `listen` (communication only) instructions.
- **Anti-complexity pressure**: larger programs pay proportionally more maintenance but still get exactly one 1-tick action per tick. Size buys storage capacity and strength, but not actuation bandwidth, concurrency, or spatial extent. If evolution consistently collapses toward minimal replicators and larger organisms never gain a foothold, consider letting size buy real capability (e.g., queued actions, multi-cell bodies) or softening size-dependent maintenance.
- **Stationary environments may under-select reproduction**: if local resource conditions are effectively constant, a stationary absorber can remain viable for very long periods, weakening the selection pressure for lineage spread. If reproduction consistently fails to persist despite viable local ecologies, consider adding slow spatial/temporal variation in external inputs.
- **Movement in dense populations**: `move` only works into empty cells. Once a region fills, spatial dynamics freeze — no fleeing, clustering, or resource-seeking movement is possible. If simulations show static fronts with no spatial reorganization, consider a `swap` instruction or displacement movement.
- **`for`/`next` non-nesting**: the single `LC` register prevents nested iteration, which limits algorithmic complexity (e.g., 2D spatial scanning, multi-step planning). If evolved programs are clearly bumping against this ceiling, add additional `LC` registers for nesting.
