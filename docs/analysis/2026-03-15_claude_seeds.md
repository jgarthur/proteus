# Prompt
```
below is my ALIFE spec. read it carefully and propose some seed replicators. do not consider the simulation parameters as fixed. i'm just getting started. currently i think the existing seed organism starves for energy and mass pretty quickly. the seed should be efficient enough (or simulation abundant enough) that most simulations end up with life spreading

[SPEC.md]
```

# Proposed Seed Replicators for Proteus v0.2

## Preliminary Analysis: Why the Current Seed Starves

The existing 12-instruction seed has two fatal deficits at the suggested parameters:

| Resource | Income/cycle | Cost/cycle | Balance |
|----------|-------------|------------|---------|
| Energy | 3.28 (1-cell absorb) | 13.31 | **−10.0** |
| Mass | 0.66 (collect only) | 12.0 | **−11.3** |

Even with 5-cell absorb (which the seed doesn't achieve — it only calls `absorb` once), energy is marginal and mass is hopeless. The parameters need to change, and so does the seed.

## Key Design Insight: `Src` Auto-Wraps — No Reset Needed

The current seed wastes 2 instructions on `push 0; setSrc`. Since `Src` is interpreted `mod size` at each `read`, and each replication cycle reads exactly `size` instructions, `Src` naturally advances by `size` and wraps perfectly:

- Cycle 1: reads indices 0..S−1, Src ends at S
- Cycle 2: reads S mod S = 0, S+1 mod S = 1, ... wraps correctly

This saves 2 instructions (−2 mass cost, −2 maintenance, −2 copy ticks).

---

## Seed A: "Lithotroph" — Minimal, Mass-Rich World

**Strategy**: Simplest possible replicator; compensate with abundant background mass.

```
; === Lithotroph (12 instructions) ===
absorb        ; 0: absorb count 1 (own cell)
absorb        ; 1: count 2 (+ front cell)
absorb        ; 2: count 3 (+ sides)
absorb        ; 3: count 4 (all 5 cells)
collect       ; 4: harvest background mass
cw            ; 5: rotate Dir for next cycle
getSize       ; 6: push 12
for           ; 7: begin copy loop
  read        ; 8: read self[Src mod 12], Src++
  appendAdj   ; 9: (nonlocal) append to neighbor
next          ; 10: loop
boot          ; 11: (nonlocal) activate offspring
;
; Tick 1: instructions 0–8 (9 local, budget=12), appendAdj (nonlocal)
; Ticks 2–12: next, read (2 local), appendAdj (nonlocal)
; Tick 13: next (falls through), boot (nonlocal)
; Cycle: 13 ticks
```

**Budget analysis** (S=12, T=13):

```
refill(13) = R_energy × (1 − 0.99^13) / 0.01 = R_energy × 12.24

E_in       = 5 × R_energy × 12.24                     (5-cell absorb)
E_cost     = 12 (appendAdj) + 13 × 12 × M             (base + maintenance)

M_in       = R_mass × 12.24                            (collect, own cell)
M_cost     = 12                                        (one per appended instruction)
```

**Recommended parameters**:

| Parameter | Value | Rationale |
|-----------|-------|-----------|
| `R_energy` | 0.25 | Default. E_in = 15.3, E_cost = 13.2. Surplus: **+2.1** ✓ |
| `R_mass` | **1.25** | M_in = 15.3, M_cost = 12. Surplus: **+3.3** ✓ |
| `M` | 1/128 | Default works |
| Others | default | |

**Tradeoffs**: Simplest code, easiest to understand, smallest evolutionary search space. Relies on a mass-rich world (steady-state 125 bg mass/cell). No synthesis — evolution must discover it. The energy margin is thin; crowded populations will feel energy pressure quickly, driving competitive evolution.

---

## Seed B: "Autotroph" — Synthesis-in-Loop, Mass Independent

**Strategy**: Put `synthesize` inside the copy loop so each instruction's mass is manufactured just-in-time from energy. Mass arrival rate becomes irrelevant.

```
; === Autotroph (13 instructions) ===
absorb        ; 0: absorb count 1
absorb        ; 1: count 2
absorb        ; 2: count 3
absorb        ; 3: count 4 (all 5 cells)
collect       ; 4: harvest background mass (bonus, not required)
cw            ; 5: rotate Dir
getSize       ; 6: push 13
for           ; 7: begin copy loop
  synthesize  ; 8: convert energy → 1 mass
  read        ; 9: read self[Src mod 13], Src++
  appendAdj   ; 10: (nonlocal) append to neighbor, consuming 1 mass
next          ; 11: loop
boot          ; 12: (nonlocal) activate offspring
;
; Tick 1: instructions 0–9 (10 local, budget=13), appendAdj (nonlocal)
; Ticks 2–13: next, synthesize, read (3 local), appendAdj (nonlocal)
; Tick 14: next (falls through), boot (nonlocal)
; Cycle: 14 ticks
;
; Mass flow per iteration: synthesize +1, appendAdj −1 = net zero.
; The organism is a perpetual motion machine for mass (powered by energy).
```

**Budget analysis** (S=13, T=14):

```
refill(14) = R_energy × (1 − 0.99^14) / 0.01 = R_energy × 13.06

E_in       = 5 × R_energy × 13.06

E_cost     = 13 × 1                                 (appendAdj base)
           + 13 × (1 + N_synth)                     (synthesize: base + additional)
           + 14 × 13 × M                            (maintenance)
           = 13 + 13×(1+N_synth) + 182M

M_in       = 13 (from synthesis) + R_mass × 13.06   (collect bonus)
M_cost     = 13
; Mass is exactly self-sufficient from synthesis alone!
```

**Two parameter options**:

### Option B1: Default synthesis cost, higher energy

| Parameter | Value | Rationale |
|-----------|-------|-----------|
| `R_energy` | **0.75** | E_in = 48.97, E_cost = 13+26+1.42 = 40.42. Surplus: **+8.6** ✓ |
| `R_mass` | 0.05 | Default! Mass comes from synthesis. |
| `N_synth` | 1 | Default |
| `M` | 1/128 | Default |

### Option B2: Free synthesis, moderate energy

| Parameter | Value | Rationale |
|-----------|-------|-----------|
| `R_energy` | **0.50** | E_in = 32.65, E_cost = 13+13+1.42 = 27.42. Surplus: **+5.2** ✓ |
| `R_mass` | 0.05 | Default |
| `N_synth` | **0** | Synthesis costs only 1E base (no additional) |
| `M` | 1/128 | Default |

**Tradeoffs**: Mass-independent — works even on barren grids. The `collect` is pure bonus (small trickle of free mass reduces synthesis pressure). Energy appetite is ~3× higher per cycle than Lithotroph, so the world must be energy-rich. The synthesis-in-loop pattern is elegant: evolution doesn't need to *discover* synthesis, just optimize its ratio. More energy headroom means more room for evolved complexity (sensing, movement, defense) before organisms starve.

---

## Seed C: "Wanderer" — Movement-Based Resource Access

**Strategy**: Instead of sitting and waiting for resources, move to fresh cells between replication cycles. Each virgin cell has ~`R/D` accumulated background mass and radiation.

```
; === Wanderer (14 instructions) ===
absorb        ; 0: absorb count 1
absorb        ; 1: count 2
absorb        ; 2: count 3
absorb        ; 3: count 4
collect       ; 4: harvest background mass from current cell
cw            ; 5: rotate Dir
getSize       ; 6: push 14
for           ; 7: copy loop
  synthesize  ; 8: convert E → M
  read        ; 9: read self[Src mod 14]
  appendAdj   ; 10: (nonlocal) append to neighbor
next          ; 11: loop
boot          ; 12: (nonlocal) activate offspring
move          ; 13: (nonlocal) relocate to cell in Dir
;
; Tick 1: instructions 0–9 (10 local, budget=14), appendAdj (nonlocal)
; Ticks 2–14: next, synthesize, read (3 local), appendAdj (nonlocal)
; Tick 15: next falls through, boot (nonlocal)
; Tick 16: move (nonlocal) — parent leaves, offspring stays
; Cycle: 16 ticks
;
; After boot, cw has rotated Dir. move goes to a fresh cell.
; Parent walks away from its offspring → no resource competition.
; Next cycle starts in a virgin cell with full bg pools.
```

**Budget analysis** (S=14, T=16):

```
refill(16) = R_energy × 14.87

E_in       = 5 × R_energy × 14.87                   (5-cell absorb)
E_cost     = 14 (appendAdj) + 14×(1+N_synth) (synth) + 1 (move) + 16×14×M

; BUT: after move, parent arrives at a VIRGIN cell with full bg pools.
; Crystallization doesn't happen (only for spontaneous creation),
; but next tick's absorb drains the full accumulated bg radiation.
; Virgin cell bg radiation ≈ R_energy/D_energy (steady state).
; This is a one-time bonus each cycle, separate from refill(T).

E_virgin   = R_energy / D_energy                    (bg radiation in virgin cell)
; At R_energy=0.5: 50 energy. At R_energy=0.25: 25 energy.
; This is captured on the FIRST absorb of the next cycle.

M_virgin   = R_mass / D_mass                        (bg mass in virgin cell)
; At R_mass=0.05: 5 mass from collect. Free bonus mass.
```

**Recommended parameters**:

| Parameter | Value | Rationale |
|-----------|-------|-----------|
| `R_energy` | **0.50** | E_in = 37.2 + ~50 virgin bonus ≈ 87. E_cost ≈ 43.8. Huge surplus. |
| `R_mass` | 0.05 | Default. Virgin cells provide 5 mass/cycle. |
| `N_synth` | 1 | Default |
| `M` | 1/128 | Default |

Actually the virgin cell bonus is more nuanced — the absorbed energy arrives at the end of tick 1 of the *next* cycle, not the current one. But the parent gets both: refill from 5 cells during the cycle + a massive virgin-cell dump at the start. In practice, this organism is energy-rich.

**The mass story**: 14 from synthesis + 5 from virgin cell collect = 19 mass, need 14. **Surplus of 5** — robust even at R_mass=0.05!

**Tradeoffs**: Most ecologically interesting of the three. Creates a spatial spreading pattern (parent walks away, offspring stays). Offspring compete with each other, not parents. The `move` instruction costs 1 energy and 1 tick but grants access to virgin resource pools. The `cw` means the parent spirals outward, naturally exploring the grid. However, `move` can fail if the target cell is occupied (protected), which happens in dense populations — driving evolution toward directional sensing.

---

## Comparative Summary

| Property | Lithotroph | Autotroph | Wanderer |
|----------|-----------|-----------|----------|
| Size | 12 | 13 | 14 |
| Cycle (ticks) | 13 | 14 | 16 |
| Energy/cycle | 13.2 | 27–40 | ~44 |
| Mass source | collect only | synthesis + collect | synthesis + virgin collect |
| Key parameter change | R_mass = 1.25 | R_energy = 0.5–0.75 | R_energy = 0.50 |
| R_mass requirement | **1.25** | **0.05** (default!) | **0.05** (default!) |
| Energy margin | thin (+2) | moderate (+5–9) | large (+40+) |
| Ecological pattern | sessile, clumpy | sessile, clumpy | dispersing, spatial |
| Evolutionary runway | must discover synthesis | optimize synth ratio | sensing, cooperation |

## Parameter Recommendations

For the broadest evolutionary potential, I'd start with **Seed C (Wanderer)** and these parameters:

```
R_energy    = 0.50      # Rich energy (steady state 50/cell)
R_mass      = 0.10      # Moderate mass (steady state 10/cell)  
D_energy    = 0.01      # Default
D_mass      = 0.01      # Default
T_cap       = 6         # Slightly generous storage
M           = 1/200     # Slightly lower maintenance than default
N_synth     = 1         # Default
P_spawn     = 0         # No abiogenesis (controlled seeding)
alpha       = 1.0       # Default
beta        = 1.0       # Default
```

This gives the Wanderer a comfortable margin (~2× income over costs) so that mutant offspring with a few junk instructions can still survive. It provides mass from two sources (synthesis + collection) so evolution can explore tradeoffs between them. And the spatial dispersal creates genuine ecological dynamics from tick 1.

If you want to start with the simplest possible seed and see synthesis evolve *de novo*, use **Seed A (Lithotroph)** with R_mass=1.25. If you want a mass-scarce world where energy-to-mass conversion is the core metabolic challenge, use **Seed B (Autotroph)** with R_energy=0.75.
