# Backend Testing Strategy

This document covers test infrastructure, property-based testing, fuzzing, multi-tick scenarios, pass-boundary verification, parallelism correctness, and edge cases that the companion doesn't enumerate. It complements the specific test cases already defined in `SPEC-COMPANION.md` (section 13), the four high-level test types in `BACKEND-GUIDELINES.md`, and the build order that sequences testing with implementation.

---

## 1. Test Infrastructure & Harness Design

### Test helper module (`tests/helpers/` or `src/test_helpers.rs` behind `#[cfg(test)]`)

Build these early — they pay for themselves immediately:

**`WorldBuilder`** — fluent builder for micro-world setup:
```rust
WorldBuilder::new(4, 4)          // small grid
    .seed(42)
    .at(1, 1, ProgramBuilder::new()
        .code(&[opcodes::ABSORB, opcodes::COLLECT, opcodes::NOP])
        .free_energy(10)
        .free_mass(5)
        .live(true))
    .at(2, 1, ProgramBuilder::new()
        .code(&[opcodes::READ_ADJ])
        .dir(Dir::Left)
        .live(true))
    .bg_radiation_at(1, 1, 3)
    .build()                     // -> (Grid, SimConfig)
```

**`ProgramBuilder`** — sets code, registers, stack, resources, liveness. Sensible defaults (live, empty stack, zero resources). This is critical because raw struct construction will be verbose and error-prone in tests.

**`assert_cell!` / `assert_program!` macros** — readable assertions on cell/program state after a tick:
```rust
assert_program!(grid, (1,1), flag == 0, stack == &[7], src == 1);
assert_cell!(grid, (1,1), free_energy == 9, free_mass == 5);
```

**`run_ticks(grid, config, n)`** — runs n ticks, returns grid. Simple but essential for multi-tick tests.

**`diff_grids(a, b)`** — compares two grids cell-by-cell, returns structured diff. Used in determinism tests to pinpoint exactly where divergence occurs instead of just "grids differ."

### Test organization

```
tests/
  helpers/mod.rs          # WorldBuilder, ProgramBuilder, assertion macros
  unit/
    opcodes/              # one file per opcode category (push, stack, arith, control, etc.)
    flag_semantics.rs
    cost_payment.rs
    deletion.rs
  integration/
    pass0_snapshot.rs
    pass1_local.rs
    pass2_nonlocal.rs
    pass2_conflicts.rs
    pass3_physics.rs
    lifecycle.rs
    multi_tick.rs
    determinism.rs
    conservation.rs
    seed_replicator.rs
  property/               # proptest tests
  fuzz/                   # fuzz targets (cargo-fuzz)
```

---

## 2. Property-Based Testing (proptest)

These catch bugs that hand-authored cases miss. Use the `proptest` crate.

### Conservation properties (most important)

**Energy conservation per tick:**
```
For any grid state and config:
  total_energy_before + expected_arrivals - expected_decay_removals - maintenance_destruction
  == total_energy_after
```
Where `total_energy = sum of (free_energy + bg_radiation + in-flight_packet_count)` across all cells. Maintenance that destroys instructions removes energy permanently (instructions consumed, not converted). Run with small grids (4x4 to 8x8) and random programs.

**Mass conservation per tick:** Same pattern. `total_mass = sum of (free_mass + bg_mass + instruction_count)` across all cells. Mass arrivals add, decay removes, maintenance destroys instructions permanently.

Note: these are stochastic (arrivals/decay are probabilistic), so either:
- Use `d_energy = 0.0, d_mass = 0.0, r_energy = 0.0, r_mass = 0.0` to remove stochastic terms and test strict conservation of internal transfers
- Or track the RNG draws and compute expected values exactly

**Recommendation: test BOTH ways.** Zero-rate configs test that internal operations conserve perfectly. Nonzero configs with known seeds test that the stochastic accounting is correct.

### Snapshot isolation property

```
For any grid with N live programs:
  Run Pass 0 to create snapshot.
  Mutate grid arbitrarily.
  Verify all sensing instructions (senseSize, senseE, senseM, senseID) still read from snapshot, not mutated grid.
```

### Stack invariants

```
For any sequence of stack operations on a random initial stack:
  - Stack depth never exceeds STACK_CAP (2^15 - 1)
  - Stack depth never goes below 0
  - Any operation that WOULD overflow/underflow sets Flag=1 and leaves stack unchanged
```

### Grid wrapping property

```
For any (x, y) and direction:
  neighbor(index(x,y), dir) always returns a valid index
  The result equals the expected toroidal-wrapped coordinate
```

### Instruction idempotency where expected

```
absorb called N times: absorb_count = min(N, 4), absorb_dir = dir_at_first_call
Multiple collect calls: same effect as one (idempotent marking)
Multiple listen calls: same effect as one (idempotent marking)
```

### Pass-2 commutativity

```
For any set of read-only class actions: order of application doesn't matter (all succeed independently)
For any set of additive transfers: order of application doesn't matter (sums are commutative)
Shuffle the action lists, verify identical outcomes.
```

---

## 3. Fuzzing Strategy

Use `cargo-fuzz` with `libfuzzer`.

### Fuzz target 1: VM interpreter with random programs

Feed random byte sequences as program code into a single cell on a small grid. Run 1 tick. Assert:
- No panics, no UB
- Stack depth within bounds
- IP within bounds (modulo program size)
- Flag is 0 or 1
- Free energy/mass are non-negative (no underflow past zero — use u32 so this is automatic, but verify no wrapping)

This is the highest-value fuzz target. The 185 no-op opcodes mean most random bytes are harmless, but the 71 real opcodes in random sequences will hit weird interactions.

### Fuzz target 2: Pass-2 conflict resolution with random action sets

Generate random sets of exclusive-class actions targeting overlapping cells. Verify:
- At most one winner per target (except multi-boot)
- No panics in conflict resolution
- Winner has highest strength (or is a valid random tiebreak)

### Fuzz target 3: Maintenance with random program sizes and resource levels

Random `(size, free_energy, free_mass)` tuples -> run maintenance -> verify:
- Payment order: energy -> mass -> instructions
- No more quanta destroyed than drawn by binomial
- Program size never reaches 0 via maintenance in a single tick (or if it does, handle correctly)

---

## 4. Multi-Tick Integration Tests

Beyond the seed replicator, these catch temporal bugs:

### Test: Resource accumulation over time
Place a single `absorb` program (just `absorb` + `nop` looping). Run 50 ticks. Verify free energy accumulates, then stabilizes near the decay threshold. Tests the absorb/free_energy/decay equilibrium.

### Test: Inert grace period expiry
Create inert program. Run for `inert_grace_ticks - 1` ticks with no writes -> verify no maintenance paid. Run 1 more tick -> verify maintenance kicks in. Then send a `writeAdj` -> verify timer resets.

### Test: Directed radiation propagation across grid
Emit a packet from one corner. Track it tick-by-tick as it crosses the grid. Verify it wraps toroidally. Verify it's captured by a `listen`-ing program at the expected tick. Verify energy accounting.

### Test: Population dynamics
Place 2 seed replicators facing each other. Run 200 ticks. Verify population > 2 (replication happened) and that programs eventually fill available space. This catches resource starvation bugs that single-replicator tests miss.

### Test: Predation / delAdj over time
Place a "predator" program that does `delAdj` on neighbors alongside inert prey. Verify the predator accumulates mass, prey shrinks, and the size-1 deletion guard works under repeated pressure.

### Test: Extinction and respawn
Run programs that consume all resources. Verify maintenance eventually kills them (program shrinks to 0 instructions -> death). If `P_spawn > 0`, verify spontaneous creation eventually repopulates empty cells.

---

## 5. Pass Boundary Correctness Tests

These are subtle and critical — they verify the snapshot isolation model actually works.

### Test: Pass-2 reads pre-Pass-2 state, not post-move state
Two programs both try to `move` into the same empty cell. Verify:
- Both see the cell as empty (pre-Pass-2)
- Exactly one wins via conflict resolution
- The loser stays in place

### Test: Move doesn't enable swap
Program A at (0,0) moves right. Program B at (1,0) moves left. Both see the other's cell as occupied in pre-Pass-2. Both fail. Neither moves.

### Test: readAdj reads pre-Pass-2 code
Program A does `writeAdj` to target. Program B does `readAdj` to same target. B should read the ORIGINAL code (pre-Pass-2), not A's written value. (Even if A's write resolves first in the exclusive class.)

### Test: giveE/giveM sums don't interfere with exclusive class
Two programs `giveE` to a cell. A third does `writeAdj` to the same cell. Verify: both energy transfers succeed AND the write succeeds (additive class resolves before exclusive class, both are valid).

### Test: appendAdj into empty cell is not bootable same tick
Program A does `appendAdj` into empty cell (creates inert offspring). Program B does `boot` on that same cell. Boot should fail because pre-Pass-2 state shows the cell as empty (no inert program to boot).

---

## 6. Parallelism Correctness (Rayon)

### The golden test: single-threaded == multi-threaded

```rust
#[test]
fn rayon_determinism() {
    let configs = vec![
        small_config(),    // 8x8, sparse programs
        medium_config(),   // 64x64, moderate density
        dense_config(),    // 32x32, every cell has a program
    ];
    for config in configs {
        let grid_st = run_single_threaded(&config, 100);
        let grid_mt = run_multi_threaded(&config, 100);  // same seed
        assert_grids_equal(&grid_st, &grid_mt);
    }
}
```

This is the ultimate Rayon correctness check. If it passes with the `diff_grids` helper, you know parallelism didn't break anything. Run with multiple thread counts (1, 2, 4, 8) via `RAYON_NUM_THREADS`.

### Watch for: thread-local action collection ordering

Pass 1 collects queued nonlocal actions into thread-local vecs that get merged. The merge order must not affect Pass 2 results. The Pass-2 commutativity property tests (section 2) catch this, but also verify it explicitly with Rayon enabled.

---

## 7. Edge Cases Not in the Companion

### Grid boundaries
- Program at (0, 0) moving left -> wraps to (width-1, 0)
- Program at (width-1, height-1) emitting right -> packet wraps
- `readAdj` across wrap boundary reads correct cell
- All 4 corners, all 4 directions

### Resource extremes
- Program with 0 free energy, 0 bg radiation attempts cost-1 instruction -> base cost fails, cell opens, Flag=1
- Program with u32::MAX free energy (if constructible) -> verify no overflow in transfers or decay
- `giveE` of i16::MAX energy -> verify cap at actual free energy
- `giveE` of negative amount (i16 is signed) -> neutral no-op

### Stack extremes
- Stack at capacity (2^15 - 1 entries) -> push sets Flag=1, stack unchanged
- Stack empty -> pop sets Flag=1, stack unchanged
- Rapid push/pop cycles near capacity boundary

### Program size extremes
- Program at size cap (2^15 - 1) -> `appendAdj` fails with Flag=1
- Program at size 1 -> `del` fails with Flag=1
- Program at size 1 -> maintenance that would destroy the instruction: what happens? (program death)

### Concurrent extremes
- All programs on grid target the same cell with exclusive actions -> exactly one wins
- 100 programs all `giveE` to same cell -> all succeed, energies sum correctly
- Every cell emits a packet in the same direction -> verify propagation and collision handling

### Control flow edge cases
- `for` with `next` at index 0, `for` at last index (wrap-around scan)
- `jmp` to self (infinite loop within budget) -> budget exhausted, execution stops
- Nested `for`/`next` (the spec says LC is a single register — verify nesting behavior is just overwrite)
- `for` with LC = i16::MAX -> loop runs budget times then stops

### Directed radiation edge cases
- Packet emitted into cell with `listen`-er -> captured same tick? (No — packet is queued, propagated in Pass 3, captured in Pass 3 step 2. So it moves 1 cell first.)
- Two packets collide in empty cell -> convert to free energy in that cell
- Packet wraps around grid and returns to emitter -> verify correct tick count for full traversal

---

## 8. Regression Test Pattern

When a bug is found (manually or via fuzzing):

1. Capture the minimal reproducing grid state as a `WorldBuilder` setup
2. Add it as a named test with a comment describing the bug
3. Tag with `#[test]` and a descriptive name: `test_bug_move_swap_should_fail`

For fuzz-found bugs, `cargo-fuzz` can minimize the input. Convert the minimized input into a readable test.

---

## 9. Test Execution Strategy

### CI pipeline order
1. `cargo test` — all unit + integration tests
2. `cargo test --features rayon` — parallelism correctness (the golden determinism test)
3. Property tests (proptest runs ~256 cases by default, increase to 1000+ in CI)
4. Fuzz targets — run for a time budget (e.g., 60s each) in CI, longer runs nightly

### Development workflow
- Run single-opcode tests while implementing each opcode
- Run pass-level integration tests after completing each pass
- Run determinism + conservation after Pass 3 is complete
- Run seed replicator smoke test as the final "it works" gate before moving to parallelism
- After adding Rayon: run the golden single-threaded==multi-threaded test

---

## 10. Key Recommendation Summary

1. **Build `WorldBuilder` and `ProgramBuilder` first** — every test needs them, and they dramatically reduce test authoring friction
2. **Conservation tests with zero stochastic rates** are the single highest-value property test — they catch off-by-one resource accounting bugs that unit tests miss
3. **Fuzz the VM** with random programs early — this finds panics and edge cases faster than hand-authored tests
4. **The Rayon golden test** (same seed -> identical grids) is the only parallelism test you really need, but it must run across multiple configs
5. **Pass boundary tests** (section 5) are the subtlest correctness concern — snapshot isolation bugs will cause nondeterminism that's hard to debug later
6. **Don't skip the `diff_grids` helper** — when a determinism test fails, you need to know WHICH cell diverged and HOW, not just "assertion failed"
