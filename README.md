# Proteus

Proteus is an artificial life and evolution simulator loosely inspired by Tierra and Life Engine.

## Design goals

- Emergent complexity of organism behavior and ecosystem dynamics, beginning with simple self-replicating organisms
- Believable, elegant physics system where organisms "fit in" to the rest of the world rather than being completely separate entities
- Massively scalable with easily distributed data and computation
- Inspiration from existing biology as well as computers, but not tied to particular aspects of either

## Physics system

- 2D space with square "cells", discrete time steps ("ticks"), and spatial interactions between adjacent cells (no diagonal interaction)
- "Speed of light" for information propagation of 1 grid cell/tick; physical laws invariant under 90 degree rotations and reflections
- Mass and energy are fundamental, conserved quantities. They are quantized and located within a single cell at a time. Energy may have velocity, but mass does not
- Fully discrete physics with no floating point calculations (except for probabilities, which are always rational values)
- Each grid cell can contain mass, energy, and non-physical elements related to computing
- Mass consists of program instructions and free mass
  - The collection of all instructions present in a cell is called a program, which may or may not be "alive" (e.g. capable of self-replicating). Only a single program can be present in a cell.
  - The CPU and other computing elements can be considered to be attached to the program rather than the cell. (They will move with the program between cells.)
  - Free mass is used by programs to write new instructions
  - Instructions in a program are organized into plasmids – loops of code with circular topology. Only a single plasmid is executing at a time.
  - The size of a program or plasmid is the total number of instructions it contains.
- Energy consists of free energy, directed radiation, and background radiation
  - Free energy is stationary, considered attached to the program in a cell (if a program is present)
  - Directed radiation is a packet of energy that propagates at 1 cell/tick. It carries a numeric value and so doubles as energy transfer and long-range communication.
  - Background radiation is the only energy input to the system from outside, arriving at a variable rate over space and time.
  - Energy is required to execute most program instructions that interact with the wider world, and can come from either free energy or background radiation (which brings a higher cost of instruction mutation).
  - Each program has a maintenance cost of `floor(program_size / 2^6)` per tick. If there is not enough free energy, then maintenance is paid with free mass or, as a last resort, instructions are destroyed.
- Mass and energy limits:
  - Program size is hard-capped at `2^15 - 1` (signed 16-bit integer max).
  - Free energy is soft-capped at program size. The excess decays exponentially (dividing by 2 each tick) and is permanently removed from the system.
  - Free mass is soft-capped at program size. The excess decays exponentially (dividing by 2 each tick) into the same quantity of free energy.
  - Cells without programs can still contain free energy and mass, but both will decay to 0.
- Program execution is simultaneous in all cells, with special rules for handling instructions that modify other cells.


## Programs and mutations

### Language and execution environment

- Stack-based memory model with specialized registers, 8-bit fixed length instructions, and no operands. Inspired by both Tierra and Forth.
- Each program has instructions along with its own CPU, registers, and stack. This is in addition to free energy and free mass.
- Instructions in a program are organized into an ordered list of circular plasmids containing labels for jumps
  - Plasmids are simply a way of organizing and manipulating chunks of related code.
  - There is a hard cap of 128 plasmids and 128 labels per plasmid.
  - Each plasmid has a permanent label 0 marking its "start".
- Instructions come in two types:
  - Immediate instructions execute instantly and cost nothing. Most register/stack manipulation and control flow instructions are immediate.
  - 1-tick instructions cost 1 energy from free energy or background radiation. Instructions that modify neighboring cells will fail unless a variable energy and/or mass cost is paid
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
    - Value of -1 (initial/default value) indicates current instruction
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
  - Values are always interpreted modulo the number of possible targets. For example, if a targeting another cell with 3 plasmids, `PO` = 4 will target plasmid `4 % 3 = 1`
- `Flag` register and error handling
  - `Flag` is set to 0 on successful instruction execution and 1 if the instruction fails (e.g. no write access, not enough energy/mass, divide by 0).
  - Operands for failing instructions are still removed from the stack.
  - Instructions that can receive messages (currently only `absorb`) set `Flag` to 1 if a message was received and 0 otherwise.

### Mutation model

- Mutation rates
  - There is a very small chance (1 in 2^16) an instruction is mutated after execution.
  - There is an additional, much higher chance of mutation if the program had 0 free energy and paid the base energy cost of an instruction using background radiation (chance of x in 2^8 if x background radiation was present).
  - Background radiation that hits a cell with no program has a chance (1 in 2^8) of creating a new program composed of a single no-op instruction.
- TODO: details of mutation model. At a basic level we can apply mutations at the bit level to instruction opcodes. Also exploring grouping instructions and using a context-free grammar to handle mutations between groups + insertions and deletions of code. It would be cool if that grammar is also evolvable...

### Handling cell interactions

- Instructions that only modify the host program or its grid cell are called "local". Other instructions are "nonlocal".
- A program and its grid cell is protected from nonlocal instructions executed by other programs *except when*:
  - A `nop`, `absorb`, or `trap` instruction was just executed by the targeted program (happens before instructions that interact, see below).
  - The targeted program was halted after executing too many immediate instructions.
- Merge rules applying to `move`, `clone`, `split`, `moveP`, and `cloneP`
  - These instructions involve one or more plasmids merging from a parent (source) cell to a target cell
  - "Control" of the merged cell is determined by comparing the total size of the plasmids moving from the parent cell to the program size of the target cell (ties favor the target cell).
    - Exception: If the target cell just executed a `trap` instruction, then it maintains control.
  - When control switches to the program that just moved in:
    - Stack is cleared
    - `PP` is set to the first plasmid moved from the source (parent) cell
    - `Msg` is set to `ID` of source cell
    - `Dir` is set to point to the parent cell
    - Other registers are re-initialized to 0

### Execution order

1. If no instructions present, go directly to step 4 (update physics).
2. Execute immediate instructions until reaching a 1-tick instruction. Each one has the usual chance of mutation after execution. If a number of immediate instructions equal to the total program size is executed at this step, go to xx to prevent an infinite loop.
3. Execute the next instruction if the base energy cost can be paid by free energy, or else if background radiation is present. In the latter case, the amount of background radiation is reduced by 1 and the instruction has a higher chance of mutating.
    - If the executed instruction has an variable free energy and/or mass cost, the cost is paid if possible; if the cost cannot be paid, the instruction does nothing and `Flag` is set to 1.
    - Details of executing instructions that modify other cells are below under "collision handling".
4. Update physics:
    - Directed radiation propagates to next cell. When multiple packets of directed radiation arrive in the same cell simultaneously, they are all converted to free energy.
    - Handle background radiation (disappears if program is present, else chance to create new program).
    - Energy maintenance is paid
    - Free energy and free mass above the soft cap decays
    - Background radiation irected radiation (possibly created by instruction) propagates to next cell. Background radiation is handled (disappears if any instruction present, else creates nop instruction). Energy maintenance is paid. Excess free energy and mass decays.

- **Collision handling**
  - First, execute local instruction
  - Then, nonlocal instructions whose target is protected fail. (Instructions are removed from the execution queue, and the executing programs do not get refunded any energy or mass costs.)
  - If a program/cell is targeted by nonlocal instructions multiple adjacent programs, then only only a single instruction can execute and the others fail. The winner is decided by the program with the most remaining free energy. Ties are broken based on where the targeteed program is pointing: the program pointed to by the target's `Dir` heading wins, or else the program pointed to first if that heading were rotated clockwise.
  - Finally, execute the successful nonlocal instructions simultaneously.

### Instruction set

- "Time" is 0 for immediate instructions and 1 for 1-tick instructions
- "Energy" is base energy cost, with "+" indicating an additional variable energy/mass cost

| Instruction | Opcode | Time | Energy | Description |
|-------------|--------|--------|------|-------------|
| *No-op*
| `nop` | 00000000 | 0 | 0 | Do nothing; no energy cost. |
| *Self*
| `move` | TBD |  1  | 1+ | Move self into adjacent cell, merging the two if successful (see move rules). Uses free m |
| `clone` | TBD |  1  | 1+ | Clone self into adjacent cell, merging the two if successful (see move rules). Requires free mass equal to program size. |
| `split` | TBD |  1  | 1+ | Split self, moving all ; details TBD. Direction required. |
| `getSize` | TBD |  1  | 1 | Push program size of target cell onto the stack. |
| *Energy/Mass*
| `absorb` | 00000001 | 0 | 0 | Capture all radiation as free energy; no energy cost. Sets `Msg`, `MsgDir`, and `Flag` = 1 if directed radiation received. |
| `emit` | TBD |  1  | 1 | Sends radiation with top value from stack in direction `Dir`. |
| `giveE` | TBD | 0 | 0 | Give 1 free energy to target cell; no energy cost. |
| `giveM` | TBD | 0 | 0 | Give 1 free mass to target cell. |
| `senseE` | TBD |  1  | 1 | Read current free energy in target cell onto the stack. |
| `senseM` | TBD |  1  | 1 | Read current free mass in target cell onto the stack. |
| `trap` | TBD |  1  | 1 | Trap a plasmids that merge into this cell this tick. |
| *Registers*
| `this` | TBD | 0 | 0 | Set all registers to point to this instruction (including `Adj` = 0). |
| `reset` | TBD | 0 | 0 | Reset all registers except `Dir` and `ID` to 0. |
| `getPP` | TBD | 0 | 0 | Push `PP` register value onto the stack. |
| `getIP` | TBD | 0 | 0 | Push `IP` register value onto the stack. |
| `getFlag` | TBD | 0 | 0 | Push `Flag` register value onto the stack. |
| `getMsg` | TBD | 0 | 0 | Push `Msg` register value onto the stack. Sets `Flag` to 0 if message was present, 1 if no message. |
| `getMsgDir` | TBD | 0 | 0 | Push `MsgDir` register value onto the stack. |
| `getID` | TBD |  1  | 1 | Push `ID` register value from target cell onto the stack. This is the only instance where a cell might read another cell's register |
| `getPO` | TBD | 0 | 0 | Push `PO` register value onto the stack. |
| `getLab` | TBD | 0 | 0 | Push `Lab` register value onto the stack. |
| `getIO` | TBD | 0 | 0 | Push `IO` register value onto the stack. |
| `getDir` | TBD | 0 | 0 | Push `Dir` register value onto the stack. |
| `getAdj` | TBD | 0 | 0 | Push `Adj` register value onto the stack. |
| `setID` | TBD | 0 | 0 | Set `ID` register to value from the stack. |
| `setPO` | TBD | 0 | 0 | Set `PO` register to value from the stack. |
| `setLab` | TBD | 0 | 0 | Set `Lab` register to value from the stack. |
| `setIO` | TBD | 0 | 0 | Set `IO` register to value from the stack. |
| `setDir` | TBD | 0 | 0 | Set `Dir` register to value from the stack. |
| `setAdj` | TBD | 0 | 0 | Set `Adj` register to value from the stack. |
| `cw` | TBD | 0 | 0 | Rotate `Dir` register 90 degrees clockwise. |
| `ccw` | TBD | 0 | 0 | Rotate `Dir` register 90 degrees counterclockwise. |
| `turn` | TBD | 0 | 0 | Rotate `Dir` register 180 degrees. |
| *Stack*
| `push##` | 0000xxxx | 0 | 0 | Push numeric value onto the stack (TBD). |
| `rand` | TBD | 0 | 0 | Push random value onto the stack. |
| `drop` | TBD | 0 | 0 | Remove top value from the stack. |
| `dup` | TBD | 0 | 0 | Duplicate top value on the stack. |
| `swap` | TBD | 0 | 0 | Swap top two values on the stack. |
| `clear` | TBD | 0 | 0 | Clear the stack. |
| *Math*
| `add` | TBD | 0 | 0 | Add top two stack values. |
| `sub` | TBD | 0 | 0 | Subtract first value from second value on stack. |
| `mul` | TBD | 0 | 0 | Multiply top two stack values. |
| `div` | TBD | 0 | 0 | Divide second value by first value on stack. Returns 0 and sets `Flag` if dividing by zero. |
| `mod` | TBD | 0 | 0 | Compute signed remainder from dividing second value by first value. Returns 0 and sets `Flag` if dividing by zero. |
| `not` | TBD | 0 | 0 | Perform logical NOT on top stack value. |
| `and` | TBD | 0 | 0 | Perform logical AND on top two stack values. |
| `or` | TBD | 0 | 0 | Perform logical OR on top two stack values. |
| `xor` | TBD | 0 | 0 | Perform logical XOR on top two stack values. |
| `eq` | TBD | 0 | 0 | Test if top two stack values are equal. |
| `lt` | TBD | 0 | 0 | Test if the second stack value is strictly less than the top value. |
| `gt` | TBD | 0 | 0 | Test if the second stack value is strictly greaterd than the top value. |
| *Control Flow*
| `for` | TBD | 0 | 0 | Begin loop with iterations determined by top value on stack, setting the `LC` register. |
| `next` | TBD | 0 | 0 | If `LC` > 0, decrement it and jump to the instruction following the matching `for` instruction; else move to the next instruction as usual. |
| `break` | TBD | 0 | 0 | Exits the current `for` loop (jumps to instruction after the following `next` statement). |
| `if` | TBD | 0 | 0 | Pop from stack; if zero, move to next instruction; else move to instruction after the matching `else` instruction. |
| `else` | TBD | 0 | 0 | Marker for `if`/`else` block; does nothing if executed. |
| `endif` | TBD | 0 | 0 | Marker for `if`/`else` block; does nothing if executed. |
| `jmp` | TBD | 0 | 0 | Jump to target instruction. |
| `jmpNZ` | TBD | 0 | 0 | Pop from stack and, if nonzero, jump to target instruction. |
| `jmpZ` | TBD | 0 | 0 | Pop from stack and, if zero, jump to target instruction. |
| *Labels*
| `newL` | TBD |  1  | 1 | Add new label at current target instruction. Pushes the new label's index to the stack. Fails if label already exists or `Lab` = -1. |
| `delL` | TBD |  1  | 1 | Remove the first label at or above the current target instruction. Fails if label doesn't exist or `Lab` ≤ 0. |
| *Instructions*
| `readI` | TBD |  1  | 1 | Read instruction at target location to the stack. Increments `IO` (to allow serial reads) unless `Lab` = -1. |
| `writeI` | TBD |  1  | 1+ | Write instruction from the stack to target location, moving forward existing instructions. Increments `IO` (to allow serial writes) unless `Lab` = -1. |
| `delI` | TBD |  1  | 1+ | Delete instruction at target location. Additional energy cost of 1. If successful, the instruction is converted to 1 free mass at the target cell.|
| *Plasmids*
| `newP` | TBD |  1  | 1 | Create new empty plasmid immediately before target plasmid, moving forward the following plasmids in the list. (If plasmid 0 is targeted, the new plasmid will be appended rather than prepended.) Modifies `PO` to point the new plasmid. Note that an empty plasmid has no instructions and a single label 0. After writing the first instruction, label 0 is updated. |
| `moveP` | TBD |  1  | 1+ | Move plasmid from self to target cell (as the last plasmid). Energy cost equal to plasmid size unless targeting self. Will never copy any registers or the stack to destination. |
| `cloneP` | TBD |  1  | 1+ | Clone plasmid indicated by `PP` and `PO` from self to destination (as the last plasmid). Energy and mass cost equal to plasmid size. Will potentially copy registers and stack. |
| `delP` | TBD |  1  | 1+ | Delete target plasmid. Additional energy cost equal to plasmid size. If successful, the target cell gains free mass equal to the plasmid size. |
| `writeP` | TBD |  1  | 1+ | Clone plasmid indicated by `PP` and `PO` from self to destination (as the last plasmid). Energy cost equal to plasmid size. Does not move any registers or the stack. |
| `mergeP` | TBD |  1  | 1 | Pop from stack to get second plasmid offset; concatenate that plasmid to the end of the one indicated by `PO`. |
| `splitP` | TBD |  1  | 1 | Split plasmid at current instruction offset `IO`, creating a new plasmid with the instructions after `IO`. Updates labels accordingly. Pushes the new plasmid offset to the stack. |
| `getSizeP` | TBD |  1  | 1 | Push size of target plasmid onto the stack. |

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

- Add cycle, mass, energy costs to table along with targets
  - `delI` and `writeI` in adjacent cell have high energy cost (e.g. free energy of adjacent cell)? What about attempting merge with cell?
  - 1 energy per mass to move is probably too high. Maybe ceil(mass / 8)? Or that plus min(program size, free energy) of adjacent cell?
- Rethink mass economy in light of simple self replicators.
- Do we need `writeP` AND `cloneP`?
- Go back to simple self-replicators
- Mutation System: rates, mechanisms (e.g., mutations during execution vs. replication), and effects on different instruction types.
  - Assign all instruction opcodes
  - consider per-instruction mutation patterns that keep function, e.g. prefer mutation to instruction with similar operands.
  - Can these mutation patterns themselves be evolvable? That would seem to require tracking multiple sets of mutation rules OR a single global set, violating locality
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
