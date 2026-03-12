# Proteus

Proteus is an artificial life and evolution simulator loosely inspired by Tierra and Life Engine.

## Design goals

- Emergent complexity of organism behavior and ecosystem dynamics, beginning either with a "primordial soup" of random instructions or with minimal self-replicators
- Believable, elegant physics system where organisms "fit in" to the rest of the world rather than being completely separate entities
- Massively scalable with easily distributed data and computation
- Inspiration from existing biology as well as computers, but not tied to particular aspects of either

## The World

- 2D space with square "cells", discrete time steps ("ticks"), and spatial interactions between adjacent cells (no diagonal interaction)
- "Speed of light" for information propagation of 1 grid cell/tick; physical laws are ideally invariant under 90 degree rotations and reflections
- Each grid cell can contain mass, energy, and non-physical elements related to computing
- Rules of physics are fully discrete with integer and fixed-point math
- Fundamental constants:
  - `T_m`: maintenance cost is paid every `T_m` ticks
  - `M_m`: maintenance cost is `ceil(program_size / M_m)` energy whenever maintenance is paid
  - `M_v`: movement cost for N instructions is `ceil(N / M_v)` energy
  - `M_t`: transmutation cost for N instructions is `ceil(N / M_t)` energy
  
## Mass and Energy

- Mass and energy are fundamental, conserved quantities. They are quantized and located within a single cell at a time. Energy may have velocity, but mass does not
- Mass consists entirely of program instructions
  - The collection of all instructions present in a cell is called a program, which may or may not be "alive" (e.g. capable of self-replicating). Only a single program can be present in a cell.
  - Instructions in a program are organized into plasmids – loops of code with circular topology. Only a single plasmid is executing at a time
  - Each program (i.e., collection of plasmids in the same cell) has a single CPU containing registers and a stack. 
  - While plasmids are circular, there is a definite "start" used for the program counter. When instructions are appended or deleted from a plasmid, this often occurs at the end of the plasmid.
  - Program size and plasmid size are defined by the total number of instructions they contain.
  - Program strength is defined as the minimum of program size and free energy. Strength is important for interactions with other programs.
- Energy consists of free energy, directed radiation, and background radiation
  - Free energy is stationary, considered attached to the program in a cell (if a program is present)
  - Directed radiation is a packet of energy that propagates at 1 cell/tick. It carries a numeric value (16-bit signed integer) and so doubles as energy transfer and long-range communication.
  - Background radiation is the only energy input to the system from outside, having some probability of updating each tick. Background radiation can also decay into mass with a small probability (see below).
  - Energy is required to execute most program instructions that interact with the wider world, and can come from either free energy or background radiation (thought the latter brings a higher cost of instruction mutation).
  - Energy is also required to transmute mass from one instruction to another
  - Each program has a maintenance cost of `floor(program_size / C_m)` per tick. If there is not enough free energy, then maintenance is paid with mass from the end of the last plasmid
- Mass and energy limits:
  - Program size is hard-capped at `2^15 - 1` (signed 16-bit integer max).
  - Free energy is soft-capped at twice the program size. The excess decays exponentially (half-life of 1 tick) and is permanently removed from the system.
  - Cells without programs can still contain free energy, but it will decay to 0.
- Program execution is simultaneous in all cells, with special rules for handling instructions that modify other cells.


## Programs and mutations

### Language and execution environment

- Stack-based memory model with specialized registers, 8-bit fixed length instructions, and no operands. (Inspired by Tierra, Forth, and the work from Blaise Aguera y Arcas et al).
- Each program has instructions along with its own CPU, registers, and stack. This is in addition to any attached free energy
- Instructions in a program are organized into an ordered list of circular plasmids containing labels for jumps
  - Plasmids are simply a way of organizing and manipulating chunks of related code.
  - There is a hard cap of 128 plasmids and 128 labels per plasmid.
  - Each plasmid has a permanent label 0 marking its "start".
- Instructions come in two types:
  - Immediate instructions cost no energy to execute, and multiple immediate instructions can be executed in a single tick. Most register/stack manipulation and control flow instructions are immediate. Endless loops are prevented by capping the total number of instructions that can be executed in a single tick by the program size.
  - Timed instructions cost at least 1 energy to execute, after which program execution is suspended until the next tick. Timed instructions typically involve interaction with the wider world.

- Additional costs are paid with a combination of free energy and background radiation. Details are in the instruction table, but the following principles are applied
  - Transferring some amount mass from one cell to another costs a proportional amount of energy
  - Writing new instructions costs 1 free mass per instruction
  - Attempting to directly modify another program's code costs free energy equal to that program's strength (minimum of program size and free energy)
- Rules for labels
  - Labels are pointers to instructions and are not themselves instructions.
  - If there are L total labels, they are always numbered 0 to L - 1.
  - If a labeled instruction is deleted, the label moves to the next unlabeled instruction (edge case: delete label if all other instructions are labeled and rename other labels 0 through L - 1).
- The stack (LIFO memory storage) contains signed 16-bit integers which do not have mass. Max stack size is `2^15 - 1`.
  - Instructions (8-bit) use 0 padding when present on the stack.
- Read-only registers/flags
  - Initial values are all 0
  - `PP` = current plasmid pointer (8-bit signed)
  - `IP` = current instruction pointer (16-bit signed)
  - `Flag` = generally an error flag, but also indicates "message received" (boolean)
  - `LC` = loop counter (16-bit signed)
  - `Msg` = last received message (16-bit signed)
  - `MsgDir` = direction of last received message (0/1/2/3 = right/up/left/down)
- Identification register (read-write, can be read by other cells)
  - `ID` = cell identifier, initialized randomly (8-bit signed)
- Target registers (used to define operands for certain instructions)
  - Default values are all 0 except where indicated.
  - `Dir` = 2-bit directional heading (0/1/2/3 = right/up/left/down). Initialized randomly at the beginning of the simulation.
  - `Adj` = boolean indicating whether to target adjacent cell (1) or self (0)
  - `PO` = plasmid offset (8-bit signed)
  - `IO` = instruction offset (16-bit signed)
  - `Lab` = label (8-bit signed)
    - Values 0 or greater for labeled instructions
    - Value of -1 (initial/default value) indicates current instruction in this cell, or end of plasmid for adjacent cells.
    - Value of -2 indicates end of plasmid
- Use of target registers
  - Certain instructions target cells, plasmids, labels, or instructions using target registers to define the operand
  - Target cell is self if `Adj` = 0, else adjacent cell in direction `Dir`
    - `Adj` is ignored by instructions that can only target other cells or can never target other cells
  - Target plasmid is `PP` + `PO` if targeting self, else `PO`
  - Target label is `Lab` on target plasmid
  - Target instruction on target plasmid depends on `Lab`
    - `Lab` ≥ 0: labeled instruction, offset by `IO`
    - `Lab` = -1: `IP + IO` for self, or `IO` for adjacent cell
    - `Lab` = -2: end of plasmid, offset by `IO`
    - (Values are all interpreted module plasmid size.)
  - Values are always interpreted modulo the number of possible targets. For example, if a targeting another cell with 3 plasmids, `PO` = 4 will target plasmid `4 % 3 = 1`.
  - A program's own PP and IP registers are guaranteed to point to a valid instruction, unless the plasmid is empty. This is to avoid excessive division to perform modular arithmetic.
- `Flag` register and error handling
  - `Flag` is set to 0 on successful instruction execution and 1 if the instruction fails (e.g. no write access, not enough energy/mass, divide by 0).
  - Operands for failing instructions are still removed from the stack.
  - Instructions that can receive messages (currently only `absorb`) set `Flag` to 1 if a message was received and 0 otherwise.

### Mutation model

- Mutation rates
  - There is a very small probability (1 in 2^16) an instruction is mutated after execution.
  - There is an additional, much higher probability of mutation if the program had 0 free energy and paid the base energy cost of an instruction using background radiation (probability of `max(x/256, 1)` if x background radiation was present).
  - When excess background radiation (not used to pay an energy cost) is removed at the end of a tick's update, it is converted into mass with a small probability (1 in 256). If there is no program in the cell, a new program with a single no-op instruction is created. Otherwise, 1 free mass is added to the cell.
- TODO: details of mutation model. At a basic level we can apply mutations at the bit level to instruction opcodes. Also exploring grouping instructions and using a context-free grammar to handle mutations between groups + insertions and deletions of code. It would be cool if that grammar is also evolvable...

### Handling cell interactions

- Instructions that only modify the host program or its grid cell are called "local". Other instructions are "nonlocal". All immediate instructions are local and have no base energy cost.
- A program and its grid cell is protected from nonlocal instructions executed by other programs *except when*:
  - A `nop`, `absorb`, or `trap` instruction was just executed by the targeted program (happens before instructions that interact, see below).
  - The targeted program was halted after executing too many immediate instructions.
  - The targeted program failed to pay the base cost or additional cost for instruction execution
- Merge rules applying to `move`, `clone`, `split`
  - These instructions involve one or more plasmids merging from a parent (source) cell to a target cell
  - "Control" of the merged cell goes to the program with the higher strength (ties favor the program with higher size, and then the target cell).
    - Exception: If the target cell just executed a `trap` instruction, then it maintains control.
  - Free mass and free energy from the two merged programs are combined
  - If the incoming program wins control: [TODO only applies to reproduction]
    - Stack is cleared
    - `PP` is set to the first plasmid moved from the source (parent) cell
    - `Msg` is set to `ID` of source cell
    - `Dir` is set to point to the parent cell
    - Other registers are re-initialized to 0

### Global execution order

1. Local instruction pass:
   1. Execute immediate instructions in each cell until either reaching a 1-tick instruction or max number of immediate instructions have been executed (in which case the program is halted for this tick).
   2. If a 1-tick instruction is reached, pay the base energy cost or else halt and do not advance the instruction pointer. If the base cost is paid, then execute the instruction if it is local, else add it to the nonlocal instruction queue. In either case, the instruction has a chance to mutate (not affecting execution this tick) and the instruction pointer is incremented by 1.
2. Nonlocal instruction pass:
   1. Calculate and pay additional costs for nonlocal instructions. If the cost cannot be paid, the instruction fails.
   2. Nonlocal instructions targeting a protected cell fail.
   3. If two programs target one another, then only a single instruction can execute and the others fail. The winner is decided by the program with the highest strength. Ties are broken based on program size, and further ties result in both instructions failing.
   4. If a program/cell is targeted by multiple adjacent programs, then only a single instruction can execute and the others fail. The winner is decided by the program with the highest strength. Ties are broken based on program size. Further ties are broken if the targeted program is pointing (via its `Dir` register) to one of the targeting programs, else all instructions targeting that program fail.
   5. Execute the successful nonlocal instructions simultaneously. Note that there may be cycles.
3. Update physics:
   1. Directed radiation propagates to next cell. When multiple packets of directed radiation arrive in the same cell simultaneously, they are all converted to free energy.
   2. Background radiation decays into free mass with a small probability (1 in 256). If there is no program in the cell, a new program with a single no-op instruction is created. Otherwise, 1 free mass is added to the cell.
   3. Energy maintenance is paid
   4. Free energy and free mass above the soft cap decays

### Instruction set

- "Time" is 0 for immediate instructions and 1 for 1-tick instructions
- "Cost" is base energy cost, with "+" indicating an additional energy/mass cost
- Consider `MOVE_SCALE = 8`

| Instruction | Opcode | Time | Cost | Description |
|-------------|--------|--------|------|-------------|
| *Self*
| `nop` | `00000000` | 1 | 0 | Does nothing. |
| `move` | TBD |  1  | 1+ | Targets adjacent cell and attempts to move self (all plasmids) into that cell, initiating a merge if successful. Additional energy cost of `ceil(program_size / MOVE_SCALE)`. |
| `clone` | TBD |  1  | 1+ | Targets adjacent cell and attempts to clone self (all plasmids) into that cell, initiating a merge if successful. Additional energy cost of `ceil(program_size / MOVE_SCALE)` and mass cost of `program_size`. |
| `split` | TBD |  1  | 1+ | Targets adjacent cell as well as a plasmid in this cell. Attempts to move that plasmid and all following plasmids into the target cell, initiating a merge if successful. Additional energy cost of `ceil(moved_plasmids_size / MOVE_SCALE)` and mass cost of `moved_plasmids_size`.
| `getSize` | TBD |  0  | 0 | Pushes this cell's program size onto the stack. |
| `getSizeP` | TBD |  0  | 0 | Pushes size of target plasmid in this cell onto the stack. |
| `trap` | TBD |  1  | 1 | Attempts to trap all plasmids that merge into this cell this tick. |
| `getStrength` | TBD |  0  | 0 | Pushes this cell's program strength onto the stack. |
| *Sensing*
| `senseID` | TBD |  1  | 0 | Pushes `ID` register of adjacent cell onto the stack. |
| `senseSize` | TBD |  1  | 0 | Pushes program size of adjacent cell onto the stack. |
| `senseSizeP` | TBD |  1 | 0 | Pushes size of target plasmid in adjacent cell onto the stack. |
| `senseStrength` | TBD |  1  | 0 | Pushes strength of program in adjacent cell onto the stack. |
| `senseE` | TBD |  1  | 0 | Reads current free energy in adjacent cell onto the stack. |
| `senseM` | TBD |  1  | 0 | Reads current free mass in adjacent cell onto the stack. |
| *Energy/Mass*
| `absorb` | `00000001` | 1 | 0 | Captures all background and directed radiation in this cell as free energy. Sets `Msg`, `MsgDir`, and `Flag` = 1 if directed radiation was received. |
| `emit` | TBD |  1  | 1 | Sends radiation containing the top value from the stack in direction `Dir`. |
| `giveE` | TBD | 1 | 0+ | Transfers half of free energy, rounded up, to target adjacent cell. The energy transferred counts as an additional cost |
| `giveM` | TBD | 1 | 0+ | Transfers half of free mass, rounded up, to adjacent cell. The mass transferred counts as an additional cost, and there is an additional cost of energy equal to the amount of mass transferred. |
| `takeE` | TBD | 1 | 0+ | Takes half of the free energy, rounded up, from target adjacent cell. Additional cost of free energy equal to adjacent program's strength. |
| `takeM` | TBD | 1 | 0+ | Takes half of the free mass, rounded up, from target adjacent cell. Additional cost of free energy equal to adjacent program's strength plus amount of mass transferred. |
| *Registers*
| `this` | TBD | 0 | 0 | Sets registers to point to this instruction (including `Adj` = 0). |
| `reset` | TBD | 0 | 0 | Re-initializes all registers except `Dir` and `ID`. |
| `getPP` | TBD | 0 | 0 | Pushes `PP` register value onto the stack. |
| `getIP` | TBD | 0 | 0 | Pushes `IP` register value onto the stack. |
| `getFlag` | TBD | 0 | 0 | Pushes `Flag` register value onto the stack. |
| `getMsg` | TBD | 0 | 0 | Pushes `Msg` register value onto the stack. Sets `Flag` to 0 if message was present, 1 if no message. |
| `getMsgDir` | TBD | 0 | 0 | Pushes `MsgDir` register value onto the stack. |
| `getID` | TBD |  0  | 0 | Pushes `ID` register value from target cell onto the stack. This is the only instance where a cell might read another cell's register |
| `getPO` | TBD | 0 | 0 | Pushes `PO` register value onto the stack. |
| `getLab` | TBD | 0 | 0 | Pushes `Lab` register value onto the stack. |
| `getIO` | TBD | 0 | 0 | Pushes `IO` register value onto the stack. |
| `getDir` | TBD | 0 | 0 | Pushes `Dir` register value onto the stack. |
| `getAdj` | TBD | 0 | 0 | Pushes `Adj` register value onto the stack. |
| `setID` | TBD | 0 | 0 | Sets `ID` register to value from the stack. |
| `setPO` | TBD | 0 | 0 | Sets `PO` register to value from the stack. |
| `setLab` | TBD | 0 | 0 | Sets `Lab` register to value from the stack. |
| `setIO` | TBD | 0 | 0 | Sets `IO` register to value from the stack. |
| `setDir` | TBD | 0 | 0 | Sets `Dir` register to value from the stack. |
| `setAdj` | TBD | 0 | 0 | Sets `Adj` register to value from the stack. |
| `cw` | TBD | 0 | 0 | Rotates `Dir` register 90 degrees clockwise. |
| `ccw` | TBD | 0 | 0 | Rotates `Dir` register 90 degrees counterclockwise. |
| `turn` | TBD | 0 | 0 | Rotates `Dir` register 180 degrees. |
| *Stack*
| `push##` | `1000xxxx` | 0 | 0 | Pushes numeric value onto the stack (TBD). |
| `rand` | TBD | 0 | 0 | Pushes random value from 0 to 255onto the stack. |
| `drop` | TBD | 0 | 0 | Removes top value from the stack. |
| `dup` | TBD | 0 | 0 | Duplicates top value on the stack. |
| `swap` | TBD | 0 | 0 | Swaps top two values on the stack. |
| `clear` | TBD | 0 | 0 | Clears the stack. |
| *Math*
| `add` | TBD | 0 | 0 | Adds top two stack values. |
| `sub` | TBD | 0 | 0 | Subtracts first value from second value on stack. |
| `mul` | TBD | 0 | 0 | Multiplies top two stack values. |
| `div` | TBD | 0 | 0 | Divides second value by first value on stack, ignoring the remainder. Returns 0 and sets `Flag` if dividing by zero. |
| `mod` | TBD | 0 | 0 | Computes signed remainder from dividing second value by first value. Returns 0 and sets `Flag` if dividing by zero. |
| `neg` | TBD | 0 | 0 | Negates top stack value. |
| `not` | TBD | 0 | 0 | Performs logical NOT on top stack value. |
| `and` | TBD | 0 | 0 | Performs logical AND on top two stack values. |
| `or` | TBD | 0 | 0 | Performs logical OR on top two stack values. |
| `xor` | TBD | 0 | 0 | Performs logical XOR on top two stack values. |
| `eq` | TBD | 0 | 0 | Tests if top two stack values are equal. |
| `lt` | TBD | 0 | 0 | Tests if the second stack value is strictly less than the top value. |
| `gt` | TBD | 0 | 0 | Tests if the second stack value is strictly greaterd than the top value. |
| *Control Flow*
| `for` | TBD | 0 | 0 | Begins loop with iterations determined by top value on stack, setting the `LC` register. |
| `next` | TBD | 0 | 0 | If `LC` > 0, decrements it and jumps to the instruction following the matching `for` instruction; else moves to the next instruction as usual. |
| `break` | TBD | 0 | 0 | Exits the current `for` loop (jumps to instruction after the following `next` statement). |
| `if` | TBD | 0 | 0 | Pops from stack; if zero, moves to next instruction; else moves to instruction after the matching `else` instruction. |
| `else` | TBD | 0 | 0 | Marker for `if`/`else` block; does nothing if executed. |
| `endif` | TBD | 0 | 0 | Marker for `if`/`else` block; does nothing if executed. |
| `jmp` | TBD | 0 | 0 | Jumps to target instruction. |
| `jmpNZ` | TBD | 0 | 0 | Pops from stack and, if nonzero, jumps to target instruction. |
| `jmpZ` | TBD | 0 | 0 | Pops from stack and, if zero, jumps to target instruction. |
| *Labels*
| `newL` | TBD |  1  | 1 | Adds new label at current target instruction. Pushes the new label's index to the stack. Fails if label already exists or `Lab` < 0. |
| `delL` | TBD |  1  | 1 | Removes the first label at or above the current target instruction. Fails if label doesn't exist or `Lab` ≤ 0. |
| *Instructions*
| `readI` | TBD |  1  | 1 | Reads target instruction to the stack. Increments `IO` (to allow serial reads) unless `Lab` < 0. |
| `writeI` | TBD |  1  | 1+ | <!-- EDIT FROM HERE --> Writes instruction from the stack to target location, moving forward existing instructions. Increments `IO` (to allow serial writes) unless `Lab` < 0. Additional cost of 1 free mass. |
| `delI` | TBD |  1  | 1+ | Deletes target instruction. If targeting an adjecent cell, there is an additional energy cost equal to `min(free_energy, program_size)` in that cell. If successful, the instruction is converted to 1 free mass at the target cell. |
| *Plasmids*
| `newP` | TBD |  1  | 1 | Creates new empty plasmid immediately before target plasmid, moving forward the following plasmids in the list. (If plasmid 0 is targeted, the new plasmid will be appended rather than prepended.) Modifies `PO` to point the new plasmid. Note that an empty plasmid has no instructions and a single label 0. After writing the first instruction, label 0 is updated. |
| `delP` | TBD |  1  | 1+ | Deletes target plasmid. Additional energy cost equal to plasmid size. If successful, the target cell gains free mass equal to the plasmid size. |
| `copyP` | TBD |  1  | 1+ | Copies target plasmid from self to destination (either self or adjacent cell) as the last plasmid. Mass cost equal to plasmid size. Energy cost equal to plasmid size unless targeting self. This instruction does not attempt a merge when copying to an adjacent cell. |
| `shuffleP` | TBD |  1  | 1 | Moves target plasmid from this program to be the last plasmid. Modifies `PO` to point to the plasmid's new location. |
| `mergeP` | TBD |  1  | 1 | Pops from stack to get second plasmid offset; concatenates that plasmid to the end of the one indicated by `PO`. |
| `splitP` | TBD |  1  | 1 | Splits plasmid at current instruction offset `IO`, creating a new plasmid with the instructions after `IO`. Updates labels accordingly. Pushes the new plasmid offset to the stack. |

## Examples

### Minimal self-replicator

```
;;;; Minimal organism with
; Plasmid 0
; Label 0
absorb      ; collect energy
getE
getSize
push4
add
lt          ;; E < Size + 4?
jmpNZ       ; if yes go back to start
push1
setAdj      ; Adj = 1, target adjacent cells
push4
for         ; do 4 times:
delI        ;; deconstruct mass
takeM       ;; take mass
cw          ; turn clockwise
next        ; repeat
push0
setAdj      ; Adj = 0, target self
getM
getSize
lt          ; M < size?
jmpNZ       ; if yes, go back to label 0
clone       ;; reproduce
```

### Even more minimal self-replicator

```
;;;; Super minimal organism powered by radiation
; Plasmid 0
; Label 0
cw          ; turn clockwise
delI        ;; deconstruct mass
takeM       ;; take mass
clone       ;; reproduce
```

## Next steps:

- Clarify ownership of the CPU and other computing elements -- are they attached to the program or the cell?
- Clarify ownership of free energy -- is it attached to the program or the cell?
- Clarify maintenance cost payment and mass:energy ratio
- Handle edge cases in interaction resolution (cycles, etc.)
  - Consider exact order of local execution, payment of additional costs, and interactions, with an eye on performance.
  - Keep in mind case where A moves to B and B moves to empty cell. Does A pay a large cost?
- Go back to review simple self-replicators
- Mutation System: set instruction opcodes, mutation rates, mechanisms (e.g., mutations during execution vs. replication), and effects on different instruction types.
- Define environmental variation
  - Shifted 3d simplex noise as log-intensity of background radiation (varying over two spatial dimensions and one time dimension)
  - Come up with some cool non-living but semi-stable programs.
- Implementation!
- Revisit communication mechanisms? Long-range sensing and pheromones?
- Revisit defense mechanisms and read/write permissions?
  - Consider adding more cases where write access is granted, e.g. failing to pay energy costs

## Further tentative ideas:

- There is a lack of multiple space/time scales. What's the impact of this? How could it be elegantly improved?
  - Could increase speed of light for information transfer?
- Consider function call system, would need to implement call stack
