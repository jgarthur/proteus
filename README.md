# Proteus

Proteus is an artificial life and evolution simulator loosely inspired by Tierra and Life Engine.

## Design goals

- Emergent complexity of organism behavior and ecosystem dynamics, beginning with simple self-replicating organisms
- Believable, elegant physics system where organisms "fit in" to the rest of the world rather than being completely separate entities
- Massively scalable with easily distributed data and computation
- Inspiration from existing biology as well as computers, but not tied to particular aspects of either

## Physics system

- 2D grid space with discrete time steps and spatial interaction in von Neumann neighborhoods (no diagonals)
- "Speed of light" for information propagation of 1 cell/tick; physical laws invariant under 90 degree rotations and reflections
- Mass and energy are fundamental, conserved quantities. They are quantized and "contained" within a cell at a time. Energy may have velocity, but mass does not
- Fully discrete physics (no real values, except perhaps with fixed-point representations)
- Each cell may contain mass (code) and energy, as well as extra properties used to define program execution
- Mass consists of instructions and free mass. The collection of instructions present in a cell is called a program, which may or may not be "alive" (e.g. capable of self-replicating). Free mass can be used by programs to write new instructions.
  - Instructions are organized into circular plasmids: ordered lists with circular topology that execute in loops. Only a single plasmid is executing at a time
  - Program size is defined as the total instruction count in a cell
- Energy consists of free energy that is stationary within cells, plus directed and background forms of radiation
  - Directed radiation can be sent by a program at a cost of 1 energy; it carries a numeric value and doubles as long-range communication
  - Background radiation the only energy input to the system from outside and will arrive with some spatiotemporal variation.
- Mass and energy limits:
  - Program size (total number of instructions) hard-capped at signed 16-bit integer max (2^15 - 1). Instructions that would add code above that size fail.
  - Energy maintenance of floor(program_size / 2^7) per tick. If not enough free energy, then pay with free mass, or else pay with instructions from end of program (last plasmid).
  - Free energy soft-capped at program size. The excess decays exponentially (divide by 2) and is removed from the system
  - Free mass soft-capped at program size. The excess decays exponentially (divide by 2) to free energy
- At each tick, every cell executes its next instruction if it (a) has free energy, which then decrements by 1, or (b) is struck by background radiation.
  - Exception for absorb instructions and no-op instructions, which both execute for free. Absorb instruction captures radiation as free energy while no-ops allow radiation to pass
  - There is a very small chance (1 in 2^16) the instruction is mutated before execution. This chance is higher (x in 2^8) if the instruction was executed by the presence of x background radiation 
  - Background radiation that hits a cell with no instructions creates a no-op instruction at that cell with some probability (1 in 2^8)
- Program execution is simultaneous in all cells, with collision handling rules varying by instruction

## Computing model

### Local execution

- Stack-based assembly language with fixed length and no operands, inspired by both Tierra and Forth
- Each cell has:
  - A virtual CPU executing a single instruction in a single plasmid at a time
  - An ordered list of circular plasmids. Each plasmid consists of 1 or more instructions and 1 or more massless numbered labels. 
    - Plasmids are simply a way of organizing and manipulating chunks of related code
    - Labels are pointers to instructions and are not themselves instructions
    - Each plasmid has a permanent label 0 marking its "start"
    - Hard cap of 128 plasmids and 128 labels per plasmid
    - Only one label per instruction
  - A single massless stack (LIFO memory storage) of signed 16-bit integers. Max size is signed 16-bit integer max (2^15 - 1)
    - Instructions are 8-bit, so first 8 bits are zero-padded or ignored
    - Logical operations consider only the least significant bit
  - Free energy counter and free mass counter
  - Read-only registers/flags
    - Initial values are all 0
    - `PP` = current plasmid pointer (8-bit signed)
    - `IP` = current instruction pointer (16-bit signed)
    - `Err` = error flag (boolean)
    - `LC` = loop counter (16-bit signed)
    - `Msg` = last received message (16-bit signed)
    - `MsgDir` = direction of last received message (16-bit signed)
  - Identification register (read-write, can be read by other cells)
    - `ID` = cell identifier, initialized randomly (8-bit signed)
  - Target registers (used as operands for instructions that target other cells/plasmids/labels/instructions)
    - Initial values are all 0 except `Dir` which is randomized
    - `Dir` = 2-bit directional heading (0/1/2/3 = right/up/left/down)
    - `Adj` = boolean indicating whether to target adjacent cell (1) or self (0)
    - `PO` = plasmid offset (8-bit signed)
    - `Lab` = label (8-bit signed)
      - Value of 0 indicates the permanant "start" label
      - Value of -1 indicates end of current plasmid
      - Value of -2 indicates no label
    - `IO` = instruction offset (16-bit signed)
  - Targets for instructions
    - Registers are used to define the target for instructions that refer to other locations
      - Target cell is self if `Adj` = 0, else adjacent cell in direction `Dir`
        - Excetion for jump instructions, which are executed as if `Adj` = 0
      - Target plasmid is `PP` + `PO` if targeting self, else `PO`. Value is always interpreted modulo plasmid count
      - Target instruction on target plasmid depends on `Lab` 
        - `Lab` ≥ 0: label position, offset by `IO`
        - `Lab` = -1: end of plasmid, offset by `IO`
        - `Lab` = -2: current instruction (given by `IP`), offset by `IO`. (If targeting adjacent cell then treat `IP` as 0)
        - (Values are all interpreted module plasmid size.)
  - Error handling
    - `Err` flag is set to 0 on successful execution and 1 if the instruction fails (e.g. no write access, not enough energy/mass, divide by 0)
    - Instructions that can receive messages (currently only `absorb`) set `Err` to 1 if a message was received
    - Operands for failing instructions are still removed from the stack

- Execution order:
  1. If no instructions present, go to (iv)
  2. Decode current instruction
     1. If instruction requires no energy, execute and increment instruction pointer. Go to (iv).
     2. Else if free energy > 0, decrement free energy, permanently mutate current instruction with some probability
     3. Else if free energy = 0 but background radiation is present, permanently mutate current instruction with probability proportional to amount of radiation and decrement background radiation
     4. Else, go to (iv)
  3. Execute current instruction (see next section for interactions between cells) and increment instruction pointer `IP` if appropriate (e.g., not a jump instruction)
  4. Any directed radiation (possibly created by instruction) propagates to next cell. Background radiation is handled (disappears if any instruction present, else creates nop instruction). Energy maintenance is paid. Excess free energy and free mass decays.
     - Note: Multiple packets of directed radiation arriving in the same cell from different directions are all converted to free energy

### Global execution rules

- First all instructions that do not modify adjacent cells are executed
- Second, instructions that modify (including moving into) adjacent cells are executed. Note that outside modification is only allowed if the cell is empty or just executed a nop/harvest instruction
  - All modifications are made simultaneously except the stack/register updates associated with a merge. Then, the appropriate stack/register updates are made
  - If a cell would be modified by more than one adjacent cell, then the modification from the program with the most remaining free energy is performed and all other modifications are discarded. Ties are broken by comparing the directional headings of the modified cell to the adjacent cells (the adjacent cell pointed to by the modified cell wins, or else the cell pointed to first when rotating the heading counterclockwise)
- Merge rules applying to `move`, `clone`, `split`, `moveP`, and `cloneP`
  - These instructions involve one or more plasmids merging from a parent (source) cell to a target cell
  - Control of the merged cell is determined by comparing the total size of the plasmids moving from the parent cell to the program size of the target cell (ties favor the target cell). If the target cell just executed a `trap` instruction, then it maintains control.
  - If control goes to the plasmids that just moved in:
    - Stack is cleared
    - `PP` is set to the first plasmid moved from the source cell
    - `Msg` is set to `ID` of parent cell
    - `Dir` is set to point to the parent cell
    - Other registers are re-initialized to 0

### Instruction set

#### No-op

| Instruction | Opcode     | Description |
| ----------- | ---------- | ----------- |
| `nop`       | `00000000` | Do nothing; no energy cost. |

#### Self modification

| Instruction | Opcode | Description |
| ----------- | ------ | ----------- |
| `move`      | `TBD`  | Move self into adjacent cell, merging the two if successful (see move rules). Uses free m |
| `clone`     | `TBD`  | Clone self into adjacent cell, merging the two if successful (see move rules). Requires free mass equal to program size. |
| `split`     | `TBD`  | Split self, moving all ; details TBD. Direction required. |

#### Energy and mass

| Instruction | Opcode     | Description |
| ----------- | ---------- | ----------- |
| `absorb`    | `00000001` | Capture all radiation as free energy; no energy cost. Sets `Msg`, `MsgDir`, and `Err` = 1 if directed radiation received. |
| `emit`      | `TBD`      | Sends radiation with top value from stack in direction `Dir`. |
| `giveE`     | `TBD`      | Give 1 free energy to target cell; no energy cost. |
| `giveM`     | `TBD`      | Give 1 free mass to target cell. |
| `senseE`      | `TBD`      | Read current free energy in target cell onto the stack. |
| `senseM`      | `TBD`      | Read current free mass in target cell onto the stack. |

#### Misc. cell-cell interaction

| Instruction | Opcode     | Description |
| ----------- | ---------- | ----------- |
| `trap`      | `TBD`      | Trap a plasmids that merge into this cell this tick. |

#### Register manipulation

| Instruction | Opcode | Description |
| ----------- | ------ | ----------- |
| `this`      | `TBD`  | Set all registers to point to this instruction (including `Adj` = 0). |
| `reset`     | `TBD`  | Reset all registers except `Dir` and `ID` to 0. |
| `getPP`     | `TBD`  | Push `PP` register value onto the stack. |
| `getIP`     | `TBD`  | Push `IP` register value onto the stack. |
| `getErr`    | `TBD`  | Push `Err` register value onto the stack. |
| `getMsg`    | `TBD`  | Push `Msg` register value onto the stack. Sets `Err` to 0 if message was present, 1 if no message. |
| `getMsgDir` | `TBD`  | Push `MsgDir` register value onto the stack. |
| `getID`     | `TBD`  | Push `ID` register value from target cell onto the stack. This is the only instance where a cell might read another cell's register |
| `getPO`     | `TBD`  | Push `PO` register value onto the stack. |
| `getLab`    | `TBD`  | Push `Lab` register value onto the stack. |
| `getIO`     | `TBD`  | Push `IO` register value onto the stack. |
| `getDir`    | `TBD`  | Push `Dir` register value onto the stack. |
| `getAdj`    | `TBD`  | Push `Adj` register value onto the stack. |
| `setID`     | `TBD`  | Set `ID` register to value from the stack. |
| `setPO`     | `TBD`  | Set `PO` register to value from the stack. |
| `setLab`    | `TBD`  | Set `Lab` register to value from the stack. |
| `setIO`     | `TBD`  | Set `IO` register to value from the stack. |
| `setDir`    | `TBD`  | Set `Dir` register to value from the stack. |
| `setAdj`    | `TBD`  | Set `Adj` register to value from the stack. |
| `cw`        | `TBD`  | Rotate `Dir` register 90 degrees clockwise. |
| `ccw`       | `TBD`  | Rotate `Dir` register 90 degrees counterclockwise. |
| `turn`      | `TBD`  | Rotate `Dir` register 180 degrees. |

#### Stack manipulation

| Instruction | Opcode     | Description |
| ----------- | ---------- | ----------- |
| `push##`    | `0000xxxx` | Push numeric value onto the stack (TBD). |
| `rand`      | `TBD`      | Push random value onto the stack. |
| `drop`      | `TBD`      | Remove top value from the stack. |
| `dup`       | `TBD`      | Duplicate top value on the stack. |
| `swap`      | `TBD`      | Swap top two values on the stack. |
| `clear`     | `TBD`      | Clear the stack. |

#### Arithmetic and logic

| Instruction | Opcode | Description |
| ----------- | ------ | ----------- |
| `add`       | `TBD`  | Add top two stack values. |
| `sub`       | `TBD`  | Subtract first value from second value on stack. |
| `mul`       | `TBD`  | Multiply top two stack values. |
| `div`       | `TBD`  | Divide second value by first value on stack. Returns 0 and sets `Err` if dividing by zero. |
| `mod`       | `TBD`  | Compute signed remainder from dividing second value by first value. Returns 0 and sets `Err` if dividing by zero. |
| `not`       | `TBD`  | Perform logical NOT on top stack value. |
| `and`       | `TBD`  | Perform logical AND on top two stack values. |
| `or`        | `TBD`  | Perform logical OR on top two stack values. |
| `xor`       | `TBD`  | Perform logical XOR on top two stack values. |

#### Control flow

| Instruction | Opcode | Description |
| ----------- | ------ | ----------- |
| `for`       | `TBD`  | Begin `for` loop with iterations determined by top value on stack, setting the `LC` register. |
| `next`      | `TBD`  | If `LC` > 0, decrement it and jump to the instruction following the matching `for` instruction; else move to the next instruction as usual. |
| `break`     | `TBD`  | Exits the current `for` loop (jumps to instruction after the following `next` statement). |
| `if`        | `TBD`  | Pop from stack; if zero, move to next instruction; else move to instruction after the matching `else` instruction. |
| `else`      | `TBD`  | Marker for `if`/`else` block; does nothing if executed. |
| `endif`     | `TBD`  | Marker for `if`/`else` block; does nothing if executed. |
| `jmp`       | `TBD`  | Jump to target instruction. |
| `jmpNZ`     | `TBD`  | Pop from stack and, if nonzero, jump to target instruction. |
| `jmpZ`      | `TBD`  | Pop from stack and, if zero, jump to target instruction. |

#### Label manipulation

| Instruction | Opcode | Description |
| ----------- | ------ | ----------- |
| `newL`      | `TBD`  | Add new label at current target instruction. Pushes the new label's index to the stack. Fails if label already exists or `Lab` = -1. |
| `delL`      | `TBD`  | Remove the first label at or above the current target instruction. Fails if label doesn't exist or `Lab` ≤ 0. |

#### Instruction manipulation

| Instruction | Opcode | Description |
| ----------- | ------ | ----------- |
| `readI`     | `TBD`  | Read instruction at target location to the stack. Increments `IO` (to allow serial reads) unless `Lab` = -1. |
| `writeI`    | `TBD`  | Write instruction from the stack to target location, moving forward existing instructions. Increments `IO` (to allow serial writes) unless `Lab` = -1. |
| `delI`      | `TBD`  | Delete instruction at target location. Additional energy cost of 1. If successful, the instruction is converted to 1 free mass at the target cell.|

<!-- TODO edit from here for destination vs target -->

#### Plasmid manipulation

| Instruction | Opcode | Description |
| ----------- | ------ | ----------- |
| `newP`      | `TBD`  | Create new plasmid at target location, moving forward existing plasmids. Pushes the appropriate value to the stack for setting `PO`. |
| `delP`      | `TBD`  | Delete target plasmid. Additional energy cost equal to plasmid size. If successful, the target cell gains free mass equal to the plasmid size. |
| `moveP`     | `TBD`  | Move plasmid from self to target cell (as the last plasmid). Energy cost equal to plasmid size unless targeting self. Will never copy any registers or the stack to destination. |
| `writeP`    | `TBD`  | Clone plasmid indicated by `PP` and `PO` from self to destination (as the last plasmid). Energy cost equal to plasmid size. Does not move any registers or the stack. |
| `cloneP`    | `TBD`  | Clone plasmid indicated by `PP` and `PO` from self to destination (as the last plasmid). Energy and mass cost equal to plasmid size. Will potentially copy registers and stack. |
| `mergeP`    | `TBD`  | Pop from stack to get second plasmid offset; concatenate that plasmid to the end of the one indicated by `PO`. |
| `splitP`    | `TBD`  | Split plasmid at current instruction offset `IO`, creating a new plasmid with the instructions after `IO`. Updates labels accordingly. Pushes the new plasmid offset to the stack. |

## Next steps:

- Make `delI` and `writeI` target self only or have very high energy cost (e.g. free energy of adjacent cell)? Only merging plasmids with other cells.
- Double check and revisit energy and mass costs for instruction writing/moving
  - Do we need `writeP` AND `cloneP`?
  - Double check which instructions can affect other cells
  - Extra energy cost is incurred when a merge might happen
  - 1 energy per mass is probably too high. Try ceil(mass / 8)?
  - Also revisit energy cost per instruction. Could be 0 for control flow and most stack and register manipulation?
- Assign all instruction opcodes
- Mutation System: rates, mechanisms (e.g., mutations during execution vs. replication), and effects on different instruction types.
  - consider per-instruction mutation patterns that keep function, e.g. prefer mutation to instruction with similar operands.
  - Can these mutation patterns themselves be evolvable? That would seem to require tracking multiple sets of mutation rules OR a single global set, violating locality
- Define environmental variation
  - To start with, probably shifted 3d simplex noise as log-intensity of background radiation (varying over two spatial dimensions and one time dimension)
- Revisit communication mechanisms? Long-range sensing and pheromones?
- Revisit defense mechanisms and read/write permissions?

## Further tentative ideas:

- There is a lack of multiple space/time scales. What's the impact of this? How could it be elegantly improved?
  - Could increase speed of light for information transfer?
- Consider function call system, would need to implement call stack
