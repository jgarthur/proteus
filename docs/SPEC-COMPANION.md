# Proteus v0.2.0 Spec Companion

Companion to **Proteus v0.2.0 Specification**. This document is not a redesign. It is an implementation-facing clarification layer: condensed phase ordering, operational invariants, truth tables, and testable edge-case rules.

When prose in the main spec feels broad, this document gives the intended machine semantics.

---

## 1. Scope and intent

This companion exists to make the simulator easier to implement faithfully and easier to test.

It focuses on:
- tick-level ordering
- which entities are eligible to act, mutate, age, or pay maintenance
- `Flag` outcome semantics
- payment and refund rules
- Pass-2 class resolution and conflict handling
- instruction success/failure side effects
- deletion / cursor / `IP` behavior
- invariants worth asserting in code and tests

The main spec remains the conceptual reference. This file is the operational reference.

---

## 2. Tick model at a glance

For a single tick:

1. **Pass 0 — Snapshot**
   - Snapshot per-cell: free energy, free mass, background radiation, background mass, program size, program `ID`.
   - Snapshot the set `live_at_tick_start`.

2. **Pass 1 — Local execution / nonlocal queueing**
   - Only programs in `live_at_tick_start` execute.
   - Each such program gets `local_action_budget = max(1, floor(size_at_tick_start ^ alpha))`.
   - Local instructions execute directly in Pass 1.
   - The first nonlocal instruction reached attempts queueing. If operand capture succeeds, it is queued. If operand capture fails, no action is queued and `Flag` is set.
   - After successful base-cost payment, that first nonlocal instruction always advances `IP` by 1 and ends Pass 1 for that program for the remainder of the tick, whether or not operand capture succeeded.
   - At most one nonlocal instruction may be queued per program.

3. **Pass 2 — Nonlocal execution**
   - All validation reads the **pre-Pass-2** state.
   - Base costs were already paid in Pass 1.
   - Additional costs are paid only for successful Pass-2 execution.
   - Resolve by class, from least invasive to most invasive.

4. **Pass 3 — Physics update**
   - Directed radiation propagation / listening / collision
   - Background radiation absorb resolution
   - Background radiation decay then arrival
   - `collect` (background mass -> free mass in the program's own cell)
   - Background mass decay then arrival
   - Inert abandonment timer update
   - Maintenance
   - Free-resource decay
   - Age update
   - End-of-tick spontaneous creation

---

## 3. Entity eligibility sets

### 3.1 `live_at_tick_start`

This set is frozen in Pass 0.

A program in `live_at_tick_start`:
- may execute in Pass 1
- may queue one nonlocal instruction
- may pay maintenance in Pass 3
- may mutate at end of tick
- increments age in Pass 3

A program **not** in `live_at_tick_start` does none of the above this tick.

### 3.2 Newborns

A program is **newborn** for the remainder of the tick if it:
- is created during the tick, or
- transitions from inert to live during the tick

Newborns:
- do not execute this tick
- do not pay maintenance this tick
- do not mutate this tick
- do not age this tick

This applies both to:
- end-of-tick spontaneous creation
- `boot` success in Pass 2

A program created by first `appendAdj` into an empty cell is inert and therefore newborn only once it is later booted. Under the current snapshot-then-apply Pass-2 model, that fresh inert offspring cannot also be booted in the same tick because `boot` validates against the pre-Pass-2 target state.

The newborn exemption is uniform: even a previously abandoned inert program that is booted this tick does not pay maintenance until the following tick.

### 3.3 Inert programs

Inert programs:
- never execute
- never age
- are always open
- pay no maintenance while inside the active-construction grace window
- begin paying normal maintenance once abandoned

---

## 4. `Flag` semantics

`Flag` is not binary success/failure memory. It has **three outcome modes**:

- **clear**: write `Flag = 0`
- **set**: write `Flag = 1`
- **neutral**: leave `Flag` unchanged

Default rule:
- hard success => **clear**
- hard failure => **set**
- explicitly designated soft no-op / informational cases => **neutral**

### 4.1 Canonical neutral cases

These are deliberately **not** treated as success or failure signals:
- unknown no-op byte values
- unmatched `next`
- `for` with `LC <= 0` that successfully skips to the matching `next`
- `giveE` / `giveM` with nonpositive requested amount
- `listen` execution in Pass 1
- `listen` when no packet is captured

### 4.2 Canonical set cases

These set `Flag = 1`:
- stack underflow / overflow
- `for` with no matching `next`
- empty-target failure
- protection failure
- Pass-2 conflict loss
- inability to pay a required base or additional cost
- `appendAdj` failure due to size cap
- deletion attempt against a size-1 program
- successful packet capture via `listen`

### 4.3 Canonical clear cases

These clear `Flag = 0` on success unless explicitly overridden:
- normal local arithmetic / stack / register instructions
- successful `read`, `write`, `del`, `synthesize`
- successful `sense*`
- successful `readAdj`, `writeAdj`, `appendAdj`, `delAdj`, `move`, `boot`
- successful `emit`, `collect`, `absorb`

---

## 5. Cost-payment rules

## 5.1 Base rule

When an instruction is reached, its **base cost** is attempted first.

- Base costs may be paid from free energy.
- If free energy is insufficient and background radiation is present, background radiation may pay **base cost only**.
- If base-cost payment fails entirely:
  - the instruction does not execute
  - `IP` does not advance
  - execution halts for the tick
  - the cell is marked open
  - remaining local action budget is lost
  - this is a hard failure (`Flag = 1`)

## 5.2 Additional costs

Additional costs are never payable from background radiation.

They must be paid from the specified working pool:
- free energy, or
- free mass

If an instruction cannot pay a required additional cost:
- the instruction fails
- `Flag = 1`
- no target/source success-side effects occur
- no additional cost is charged

## 5.3 Local vs nonlocal payment timing

### Local instructions
- Base cost is checked at execution time.
- Additional cost, if any, is checked at execution time immediately after the base cost succeeds.
- Both must succeed for the instruction to succeed.
- If either payment fails: `Flag = 1`, `IP` does not advance, the cell is marked open, and the remaining local action budget is forfeit.
- If the additional cost fails, the already-paid base cost is **not** refunded.

### Nonlocal instructions
- Base cost is paid in Pass 1 when queueing is attempted.
- Operands are captured in Pass 1.
- Operand capture is atomic. If it fails (for example due to stack underflow), base cost is not refunded, no operands are consumed, no action is queued, `Flag = 1`, `IP` advances by 1, the program stops for the remainder of the tick, and the cell does **not** open.
- Additional cost, if any, is checked only in Pass 2 and only for a candidate that otherwise succeeds.
- If the candidate later fails, there is no refund of base cost and no restoration of operands.

## 5.4 Background-radiation-stressed mutation trigger

A program is considered background-radiation-stressed this tick iff **any base instruction cost** for that program used background radiation.

Background radiation used for no other purpose changes mutation risk only through this rule.

---

## 6. Pass-2 semantics

All Pass-2 logic reads from the **pre-Pass-2 state**.

That means:
- occupancy checks read pre-Pass-2 occupancy
- target size / strength / protection checks read pre-Pass-2 state
- `readAdj` reads pre-Pass-2 target code
- `move` sees the target as occupied if it was occupied before any Pass-2 move resolves
- swaps therefore fail

### 6.1 Class ordering heuristic

Pass 2 is ordered from **least invasive** to **most invasive**:

1. **Read-only class** — observe target state only
2. **Additive transfer class** — move resources without changing code / liveness / occupancy
3. **Exclusive class** — may change code, liveness, or occupancy

This is a semantic ordering, not just an implementation convenience.

### 6.2 Within-class ordering rule

For the non-conflicting classes, semantics are defined so that **within-class instruction order does not matter**.

Implementation rule:
- validate all instructions in the class against the pre-Pass-2 snapshot
- compute their effects
- apply source/target updates as a simultaneous class commit

No stable per-cell scan order is required within read-only or additive-transfer classes.

That works because:
- each source queues at most one nonlocal instruction per tick
- `readAdj` does not write target state
- `giveE` / `giveM` are additive and do not conflict

### 6.3 No fallback winner rule

If an exclusive-class tentative winner later fails due to an additional-cost failure, there is **no fallback** to another candidate for that target cell.

That target simply receives no successful exclusive-class action this tick.

---

## 7. Pass-2 truth tables

## 7.1 Read-only class

### `readAdj`

| Aspect | Rule |
|---|---|
| Conflict participation | none |
| Protection check | none |
| Empty target | fail |
| Target read view | pre-Pass-2 target code |
| Base cost timing | Pass 1 |
| Additional cost | none |
| Source operand capture | none |
| On success | push target instruction, increment `Src`, clear `Flag` |
| On empty target | push 0, do not increment `Src`, set `Flag = 1` |

## 7.2 Additive transfer class

### `giveE`

| Aspect | Rule |
|---|---|
| Conflict participation | none |
| Protection check | none |
| Empty target | succeeds; energy moves into the adjacent cell's free-energy pool |
| Amount source | popped in Pass 1 |
| Amount cap | `min(requested_amount, source_free_energy_post_pass1)` |
| Nonpositive amount | neutral no-op |
| Base cost timing | Pass 1 |
| Additional cost | none |
| Intentional asymmetry | base cost 0; moving energy is free once it already exists in the source working pool |
| Success side effect | transfer energy |
| `Flag` | clear on positive transfer; neutral on nonpositive request |

### `giveM`

| Aspect | Rule |
|---|---|
| Conflict participation | none |
| Protection check | none |
| Empty target | succeeds; mass moves into the adjacent cell's free-mass pool |
| Amount source | popped in Pass 1 |
| Amount cap | `min(requested_amount, source_free_mass_post_pass1)` |
| Nonpositive amount | neutral no-op |
| Base cost timing | Pass 1 |
| Additional cost | none |
| Intentional asymmetry | base cost 1; moving matter requires work even though the transferred mass comes from the source pool |
| Success side effect | transfer mass |
| `Flag` | clear on positive transfer; neutral on nonpositive request |

## 7.3 Exclusive class

### Shared exclusive-class rules

| Aspect | Rule |
|---|---|
| Validation view | pre-Pass-2 state |
| Maximum successes per target cell | one, except multi-`boot` special case |
| Conflict resolution | by target cell |
| Winner selection | highest strength, then random weighted by size |
| Base cost refund on loss | none |
| Operand restoration on loss | none |
| Source cursor increments on loss | none |
| `Flag` on loss | set |
| Fallback winner after winner later fails extra-cost payment | none |

### `writeAdj`

| Aspect | Rule |
|---|---|
| Target must exist | yes |
| Target may be protected | no |
| Additional cost | none |
| Operand capture | popped in Pass 1 |
| On success | overwrite target instruction at `Dst mod size`, increment `Dst`, clear `Flag` |
| On failure | no write, no `Dst` increment, set `Flag = 1` |

### `appendAdj`

| Aspect | Rule |
|---|---|
| Empty target | create new inert program |
| Occupied target protected | fail |
| Additional cost | 1 free mass, paid only on success |
| Operand capture | popped in Pass 1 |
| Size-cap check | uses would-be post-append size |
| Size-cap failure | no append, no mass charge, set `Flag = 1` |
| On success to occupied target | append instruction, clear `Flag` |
| On success to empty target | create inert offspring with one instruction, abandonment timer starts at 0, clear `Flag` |

### `delAdj`

| Aspect | Rule |
|---|---|
| Target must exist | yes |
| Target may be protected | no |
| Target size must exceed 1 | yes |
| Additional cost | target strength in pre-Pass-2 state |
| Additional cost timing | paid only on success |
| On success | delete target instruction at `Dst mod size`, increment `Dst`, attacker gains 1 free mass, clear `Flag` |
| On failure | no deletion, no `Dst` increment, no mass gain, no extra-cost payment, set `Flag = 1` |

`delAdj` increments the attacker's `Dst` on success because the cursor is tracking a remote target. This differs intentionally from local `del`, which leaves `Dst` fixed so repeated self-deletions keep addressing the next surviving local instruction.

### `move`

| Aspect | Rule |
|---|---|
| Target must be empty in pre-Pass-2 state | yes |
| Additional cost | none |
| Swap with simultaneous opposing move | fails, because each sees occupied target |
| On success | move program + free resources to target cell; background pools remain behind; clear `Flag` |
| On failure | source remains in place, set `Flag = 1` |

### `boot`

| Aspect | Rule |
|---|---|
| Target must exist | yes |
| Target must be inert | yes |
| Protection check | none; inert cells are always open |
| Additional cost | none |
| Multi-boot special case | if the only valid exclusive-class instructions on that target are one or more `boot`s, all succeed |
| On success | target becomes live with `IP = 0`, target is newborn for remainder of tick, clear `Flag` |
| On failure | no state change, set `Flag = 1` |

---

## 8. Local instruction notes that matter for testing

### 8.1 No-op bytes

Unknown byte values:
- cost 0
- consume 1 local action
- do not open the cell
- are `Flag`-neutral
- advance execution normally

### 8.2 `for` / `next`

- `for` with `LC > 0` enters the loop and is a normal hard success.
- `for` with `LC <= 0` scans forward for the matching `next`.
  - if found: skip is `Flag`-neutral
  - if not found after full wrap: set `Flag = 1`
- `next` with no active loop context is a neutral no-op.
- Matching scan cost is **1 local action total**, not per scanned instruction.

### 8.3 `listen`

`listen` has two distinct effects:
- in Pass 1 it marks the program open and marks it for packet capture; this Pass-1 execution is **Flag-neutral**
- in Pass 3, if packets are captured, it gains total packet energy and sets `Msg`, `Dir`, and `Flag`

If no packet is captured, `listen` is `Flag`-neutral.

### 8.4 `absorb`

`absorb` does not open the cell.
Repeated `absorb` in the same tick only expands footprint:
- first call sets `absorb_count = 1` and `absorb_dir = Dir`
- later calls increment `absorb_count` up to 4
- later calls do not change `absorb_dir`
- calls after `absorb_count = 4` have no further effect

---

## 9. Deletion, `IP`, and cursor invariants

## 9.1 Raw-cursor invariant

`Src` and `Dst` are stored as raw integers.
They are **not** normalized after size changes.
They are interpreted modulo current size only when dereferenced.

## 9.2 Continue-with-next-surviving-instruction invariant

Deletion is defined so that execution continues with the next surviving instruction in program order.

### Local `del`
If deletion removes index `i` and `i` is at or before the currently executing instruction index, decrement `IP` by 1 before the normal fallthrough increment.

Equivalent intuition:
- after local self-deletion, execution continues with what was originally the next instruction after the deleted one, in post-deletion code.

### Nonlocal `delAdj`
The target already finished its Pass-1 execution for the current tick, so only stored next-tick `IP` matters.
If deleted index `i` is strictly less than target stored `IP`, decrement target stored `IP` by 1.

## 9.3 Size-1 deletion invariant

Neither `del` nor `delAdj` may delete the final remaining instruction of a program.

Attempting to do so:
- fails
- sets `Flag = 1`
- leaves program unchanged

Program death from size collapse happens only through maintenance / erosion semantics, not explicit delete instructions.

---

## 10. Resource and conservation invariants

These are useful as assertions.

## 10.1 Directed radiation packet invariant

Each directed-radiation packet carries exactly:
- 1 energy
- 1 signed 16-bit message payload
- 1 cardinal direction of propagation

Energy accounting invariant:
- `emit` base cost of 1 becomes exactly one packet
- packet capture contributes exactly one free energy per packet
- uncaptured collision of `n >= 2` packets contributes exactly `n` free energy to the cell

## 10.2 Background-pool conversion invariant

Background pools are never spent directly as working currency except:
- background radiation may pay **base instruction cost only**
- `absorb` converts captured background radiation to free energy
- `collect` converts all cell background mass to free mass
- spontaneous crystallization converts all remaining background radiation/mass in a newly spawned cell into free resources

## 10.3 Working-pool invariant

Only free energy / free mass are ordinary working pools.
- base costs draw from free energy first, then optionally background radiation
- additional costs draw only from free energy / free mass
- maintenance draws from free energy, then free mass, then instructions
- instructions destroyed by maintenance are consumed permanently rather than first entering the free-mass pool

## 10.4 Free-resource decay threshold invariant

For a cell with program size `S`:
- threshold = `T_cap * S`

For an empty cell:
- threshold = 0

Only **excess** free resources decay.
Background pools decay independently of thresholds.

---

## 11. Arrival/decay ordering invariants

Both background pools use the same rule:

1. existing units decay
2. a new unit may arrive

Equivalent interpretation:
- newly arrived background units do **not** decay on the same tick they arrive

This rule should be consistent with steady-state calculations used in analysis.

---

## 12. Suggested implementation data model

A faithful simulator usually wants per-program transient tick fields:
- `used_bg_for_base_cost: bool`
- `absorb_count: int`
- `absorb_dir: Dir or None`
- `did_listen: bool`
- `did_collect: bool`
- `did_nop: bool`
- `queued_nonlocal: Optional[QueuedAction]`
- `is_newborn_this_tick: bool`

Suggested queued-action payload:
- source cell
- opcode
- target cell
- captured operands
- captured source-side raw cursor values if helpful
- precomputed additional-cost operands if helpful

For Pass 2, it is usually easiest to build:
- read-only action list
- additive-transfer action list
- exclusive-candidate map keyed by target cell

---

## 13. Suggested tests

## 13.1 `Flag` tests
- unknown no-op leaves `Flag` unchanged
- unmatched `next` leaves `Flag` unchanged
- `for` skip-found leaves `Flag` unchanged
- `for` skip-missing sets `Flag`
- `listen` execution itself leaves `Flag` unchanged in Pass 1
- `listen` with no packet leaves `Flag` unchanged
- `listen` with packet sets `Flag`
- `senseSize` / `senseE` / `senseM` on empty neighbor clear `Flag`
- `senseID` on empty neighbor sets `Flag`
- Pass-2 conflict loss sets `Flag`
- size-cap `appendAdj` failure sets `Flag`

## 13.2 Payment tests
- base cost may be paid from background radiation
- additional cost may not be paid from background radiation
- failed additional-cost payment does not refund base cost
- failed additional-cost payment does not increment `Src`/`Dst`
- failed nonlocal operand capture does not refund base cost, does not queue an action, and still advances `IP`
- maintenance-destroyed instructions are removed permanently and do not become free mass

## 13.3 Pass-2 tests
- two `readAdj` into same target both succeed
- multiple `giveE` into same target all succeed and sum
- multiple `giveM` into same target all succeed and sum
- `writeAdj` and `appendAdj` into same target conflict
- `boot` + `boot` on same inert target both succeed
- `boot` + hostile action on same target enters exclusive conflict resolution
- winning `delAdj` that cannot pay strength cost causes no fallback winner
- opposing `move`s do not swap

## 13.4 Lifecycle tests
- newborn spontaneous spawn does not act / age / mutate / pay maintenance until next tick
- newborn booted offspring does not act / age / mutate / pay maintenance until next tick
- booted previously abandoned inert program also skips maintenance on its boot tick
- inert offspring stays open
- append-then-boot in the same tick is impossible under pre-Pass-2 validation
- abandonment timer resets on successful incoming `appendAdj` / `writeAdj`

## 13.5 Deletion tests
- local `del` continues with next surviving instruction
- `delAdj` adjusts stored target `IP` only when deleted index is strictly below it
- local `del` leaves `Dst` fixed while successful `delAdj` increments the attacker's `Dst`
- `Src` / `Dst` are never renormalized after deletion
- `del` / `delAdj` against size-1 target fail

---

## 14. Non-obvious consequences worth remembering

1. **A program can do substantial local work without opening itself.**
   Only `listen`, `nop`, inert state, or instruction-payment failure open the cell.

2. **`readAdj` is intentionally privileged.**
   It is nonlocal but non-hostile and non-conflicting.

3. **`giveE` / `giveM` can feed protected cells.**
   Protection gates only hostile/structural interference.

4. **Exclusive-class failure is sticky for the tick.**
   If the chosen winner later cannot pay extra cost, nobody else gets to win.

5. **Directed-radiation capture is both sensing and metabolism.**
   `listen` gains energy and message information together, and packet choice among simultaneous captures is randomized uniformly.

---

## 15. Priority checklist before coding changes to the spec

If the main spec is edited later, re-check these first:
- Does the change alter who is in `live_at_tick_start`?
- Does it change whether an effect should be clear / set / neutral for `Flag`?
- Does it change whether a cost is base vs additional?
- Does it belong in read-only, additive-transfer, or exclusive Pass-2 class?
- Does it create a new source-side effect that should happen only on success?
- Does it interact with newborn semantics?
- Does it require a deletion / cursor / occupancy edge-case rule?
