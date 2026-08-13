# Effective Mutation Rate, Saturation Dynamics, and the Emergence Milestones — August 13, 2026

**Context**: Non-authoritative working notes. **Written by Claude (Opus 5)** from Joey's unstructured
observation notes and follow-up questions; the observations in §1 are his, the derivations,
measurements, and milestone vocabulary are the model's. Structural claims cite `SPEC.md` v0.2.1;
numeric claims come from the headless-runner measurements in §8. Nothing here changes the spec.

**Terminology warning**: an earlier draft of this note called §6 "the abiogenesis threshold." That
was wrong. Nothing observed here self-replicates. §2 defines the milestone ladder this note uses
instead, and reserves *abiogenesis* for the rung that actually deserves it.

## Summary

1. **`mutation_base_log2` is nearly a dead knob in any world containing real replicators.** Measured
   mutation in a seeded 30k-tick run was **35.7× the configured `2^-16` baseline**. The operative
   rate is the background-radiation-stressed one, governed by `mutation_background_log2` and by how
   often programs are broke — which, in a working ecology, is always. → §3
2. **Mutations per program lifetime separates the two regimes** — ≈ 1–1.9 in the sparse world,
   spiking to ≈ 25 in the saturated one. But what drives it is a **collapsed birth rate, not a fast
   mutation clock**: turnover is 11,000–27,500 ticks against the seed replicator's 14-tick design
   cycle, while the mutation rate is only 4.6–33.5× the `2^-16` baseline. Whether this equals
   mutations per generation depends on how uniform reproduction is, which is untested — the bounds
   span three orders of magnitude. → §4
3. **The long slow decline is not a steady state; it is an overshoot unwinding.** Confirmed over a
   200k-tick run: mean genome size converges from 72.4 to **40.5** and mutations-per-lifetime from
   12.7 to **≈ 1.8**, both settling by tick 130k. Density is a slower variable and had not settled.
   No explicit senescence is needed — the substrate already has three aging mechanisms. → §4, §5
4. **`move` is load-bearing because sessile replication is arithmetically impossible.** At default
   rates there is *no* program size for which a stationary own-cell absorber can fund its own
   replication cycle. A `move` is worth `1/D_energy` ticks of sessile income, so gliders are
   strip-mining, not wandering. → §5
5. **The size-1 ceiling is a rate limit, not a structural impossibility — but crossing it is not
   abiogenesis.** Spontaneous growth past size 1 is controlled by **mutations × occupancy**, not by
   `mutation_base_log2` alone: a 62-run sweep collapses `k`, `p_spawn`, and `r_mass` onto that single
   variable, with onset at ≈ 10⁴. `p_spawn` buys more attempts, `r_mass` buys bigger builds (31–32
   instructions at `r_mass = 4`), and the spec's default `r_mass = 0.05` produces dead soup. At
   `k = 6` accreted structures get booted into life with no seed program anywhere in the run.
   **Nothing observed here self-replicates** — see the ladder in §2. → §6

## 1. Source Observations

The raw observations this note was built from, reworded but not reinterpreted. Pointers indicate
where each is addressed.

- **Default settings produce a huge population explosion followed by a long, very slow decline.**
  → §4, §5.
- **Raising `d_energy` reduces the explosion.** → §5, which argues the dominant reason is not the
  one you would expect.
- **On seed 1 there is no explosion.** Instead there is one interesting mobile program running
  around and dropping single-instruction offspring that quickly decay. Genome and analysis below.
- **Seed 5013216203605585: explosion, then stable.** Population reaches ~16k by 25k ticks, then
  declines very slowly — roughly 0.04 programs/tick of excess deaths. → §4, §5. Reproduced here at
  0.021 programs/tick over ticks 6,000–30,000.
- **Seed 4120295913392002: lots of gliders, no real explosion.** Not re-run here; §5 explains why
  glider-dominated worlds are the energetically natural outcome, and predicts these worlds carry a
  low mutational load rather than overshooting into saturation. Worth confirming directly.
- **Open question: do we need senescence?** Preferably not explicitly — perhaps a higher mutation
  rate suffices. Expected mutations per generation should be computed. → §3, §4. Short answer: the
  realized rate is already ~35× higher than the configured one, three senescence mechanisms already
  exist, and no explicit one is needed.
- **Removing the `move` (99) instructions from the seed program makes behavior much less
  interesting.** → §5. There is a hard arithmetic reason for this.
- **Open question: abiogenesis.** The current `p_spawn` mechanism and surrounding mechanics look
  insufficient — no program has ever been seen to exceed size 1 that way, and the minimal viable
  self-replicator is much larger. → §2, §6. The size-1 ceiling is real at default settings and has a
  measurable onset condition (mutations × occupancy ≈ 10⁴); but crossing it is not abiogenesis.

### The seed-1 program

The evolved 39-instruction genome observed running around and shedding offspring:

```
 0 absorb    81      13 next      49      26 absorb    81
 1 cw        64      14 cw        64      27 absorb    81
 2 push 0     0      15 next      49      28 next      49
 3 getSize   66      16 absorb    81      29 cw        64
 4 collect   83      17 boot     100      30 read      85
 5 for       48      18 absorb    81      31 getSize   66
 6 push 0     0      19 move      99      32 appendAdj 95
 7 for       48      20 absorb    81      33 for       48
 8 absorb    81      21 move      99      34 getSize   66
 9 read      85      22 absorb    81      35 read      85
10 push 0     0      23 absorb    81      36 for       48
11 readAdj   93      24 absorb    81      37 appendAdj 95
12 absorb    81      25 absorb    81      38 absorb    81
```

Structurally: **11 of 39 instructions are `absorb`**, which saturates `absorb_count` at its cap of 4
and buys the 5-cell footprint that §5 shows is required for any viability at all. Two `move`s. The
copy machinery is at 30–32 and 34–37.

**Why the offspring are single-instruction and short-lived** — hand trace of the first cycle, *not
verified against a run*:

```
tick 1:  absorb(0) cw(1) push0(2) getSize(3)->39 collect(4)
         for(5) pops 39 -> LC=39
         push0(6) for(7) pops 0 -> LC=0 -> scan fwd to next(13), skip to 14
         cw(14) next(15) LC->-1, falls through
         absorb(16) boot(17)  <- NONLOCAL, tick ends
tick 2:  absorb(18) move(19)              tick 3: absorb(20) move(21)
tick 4:  absorb(22..27) next(28) cw(29) read(30) getSize(31)->39 appendAdj(32)
                                          ^ pops 39, not the read byte
```

Two things fall out. First, `boot` sits at index 17, **before** the copy machinery at 30–37 in
circular execution order, so each cycle animates the neighbor before writing the body into it.
Second, `appendAdj` at 32 pops the top of stack, which `getSize` just overwrote with 39 — so the
appended byte is the literal `39` (`0x27` = `and`), not the byte that `read` fetched. The copy loop
is misaligned. The predicted offspring is therefore a single `and` instruction, booted live, with no
income instruction and no resources.

Such an offspring is a live size-1 program that dies when maintenance takes its only instruction:
`Binomial(1, M)` per tick with `M = 1/128`, so a mean of **128 ticks** (median ~89). An offspring
left inert instead gets `inert_grace_ticks` first, for ~138 ticks. Either way: babies that quickly
decay, exactly as observed. Both the misaligned pop and the boot-before-copy ordering are testable
by inspecting a live offspring's code at boot time.

## 2. Milestone ladder

"Abiogenesis" is doing too much work. It suggests self-replication emerged, and it has not. These
are distinct events with distinct evidence requirements, and it is worth naming them separately.

| | milestone | what it means | mechanism | status |
|---|---|---|---|---|
| **M0** | **Nucleation** | a size-1 `nop` appears from background mass | `p_spawn` | works by construction |
| **M1** | **Persistence** | a size-1 program acquires an income instruction (`absorb`/`collect`) and stops dying | 1 bit flip from `nop` | **predicted, not confirmed** — needs an opcode census |
| **M2** | **Accretion** | a structure larger than size 1 is built by a neighbor's `appendAdj` | mutation walk to `appendAdj` + a persisted stack | **observed**, `k ≤ 12` |
| **M3** | **Animation** | a live program of size > 1 exists that no ancestor designed | accretion onto a live `nop` spawn, or `boot` of an accreted inert | **observed**, `k ≤ 12` (accretion route); `k = 6` (boot route) |
| **M4** | **Autonomy** | such a program pays its own way and persists rather than dying to maintenance | must accrete an income instruction | **not observed** |
| **M5** | **Replication** | an emerged program copies itself — *this* is abiogenesis | must accrete a working copy loop, ~12 instructions | **not observed** |
| **M6** | **Lineage** | the emerged replicator sustains a population across generations, so M5 was not a one-off | selection on emerged variants | **not observed** |

Everything measured in §6 is M2 and M3. That is real — those are structures and living programs
that nothing designed, assembled from random bit flips — but it is construction, not reproduction.
The gap from M3 to M5 is the whole problem, and this note does not close it.

Above M6 sits a separate open-endedness ladder (sustained diversity rather than clonal collapse;
parasites on an *emerged* replicator; use of a function no ancestor had, e.g. `emit`/`listen`;
proto-multicellular specialization via `ID`/`Msg`). Those are a different research program and
should not be conflated with M0–M6 either.

## 3. The realized mutation rate is 5–35× the configured one

Per `SPEC.md` §Mutation Model there are two rates: baseline `2^(-mutation_base_log2)` per live
program per tick, and background-stressed `min(x / 2^(mutation_background_log2), 1)` whenever a
program paid any *base* instruction cost out of background radiation.

Measured over the whole 30k-tick boom-bust run, integrating population over time:

| quantity | value |
|---|---|
| cumulative mutations | 237,996 |
| baseline-expected mutations (∫ pop dt × 2^-16) | 6,665 |
| **realized / baseline** | **35.7×** |

Per-window it never drops below ~29× and peaks at ~48× during the explosion. The sparse seed-1 world
runs at 20–49× as well. Programs in a working ecology are broke essentially all the time, so they are
permanently in the stressed regime.

The multiplier is regime-dependent, not a constant: over the 200k-tick run it falls from 33.5× at
tick 20,000 to **4.6× at tick 200,000** as occupancy drops from 98.5% to 58.5% and programs stop
competing for the same energy. So "~35×" characterizes a saturated world specifically.

The clean control: in the nucleation-only soup (§6), where every program is a single zero-cost
instruction that never needs to pay for anything, the realized rate matches baseline to within 12%
at every `k` tested. So the multiplier is genuinely energy stress, not a measurement artifact.

Two consequences:

- **The crossover is at `k = 8`.** Baseline `2^-k` exceeds the single-unit stressed rate `1/2^8`
  exactly when `k < 8`. Above that, lowering `mutation_base_log2` from 16 toward 9 barely moves the
  realized rate — the stressed term already dominates. So the mutations-per-generation table you
  would write from `2^-16` is off by a factor of ~35, and the knob that actually matters for seeded
  worlds is `mutation_background_log2`.
- **Stress mutagenesis is already an implicit senescence mechanism**, and it activates exactly in
  the decline phase. Combined with maintenance destroying instructions from the program tail under
  scarcity, and with germline = soma (programs `read` their own current code to build offspring, so
  every somatic hit is heritable), the substrate has three aging mechanisms already. No explicit
  senescence needed.

## 4. Mutations per program lifetime

**Read the caveat at the end of this section before using these numbers.** An earlier draft called
this quantity "mutations per generation" and compared it to the quasispecies error threshold. That
comparison rests on an assumption this note cannot test, and the section is written accordingly.

Mutation rate per tick is not by itself ecologically meaningful. The quantity below is
`mutations/tick ÷ births/tick`. In a stationary population, births ≈ deaths, so `population ÷
births/tick` is the mean program lifetime — which makes this ratio exactly **the expected number of
mutations a program accumulates over its life**. Because germline = soma (programs `read` their own
current code to build offspring), those hits are heritable if the program reproduces.

| tick | boom-bust: occupancy | mean size | births/tick | mutations/tick | **mut/lifetime** |
|---|---|---|---|---|---|
| 3,000 | 42.9% | 32.3 | 5.69 | 1.71 | **0.30** |
| 4,000 | 97.4% | 44.1 | 11.36 | 8.38 | **0.74** |
| 6,000 | 99.7% | 60.3 | 0.44 | 11.17 | **25.2** |
| 10,000 | 99.5% | 70.0 | 0.42 | 10.08 | **23.8** |
| 20,000 | 98.5% | 70.1 | 1.02 | 8.22 | **8.0** |
| 30,000 | 96.7% | 63.4 | 1.07 | 6.83 | **6.4** |

The sparse seed-1 world, by contrast, rises to ≈ 1–1.9 by tick 8,000 and stays in that band through
tick 30,000 (population ~780, mean size ~50, births ≈ deaths ≈ 0.2–0.3/tick).

This does separate the two regimes cleanly, and it explains the observed genotypic uniqueness
directly: ~2 accumulated mutations per program lifetime is enough to make most genomes unique
(12,870 unique among 15,836 live programs at tick 30,000) without invoking anything exotic.

### What drives it is the birth rate, not the mutation rate

The mutation rate is *low* in absolute terms and gets lower as the world de-saturates — from 33.5×
baseline at tick 20,000 down to **4.6× baseline** at tick 200,000, i.e. `7.0e-5` per program per tick,
one mutation per program per ~14,000 ticks. What is extreme is turnover:

| tick | mut/program/tick | × baseline (`2^-16`) | turnover time `N/B` | mut/lifetime |
|---|---|---|---|---|
| 20,000 | 5.1e-4 | 33.5× | 11,280 ticks | 5.8 |
| 100,000 | 1.9e-4 | 12.1× | 12,390 ticks | 2.3 |
| 200,000 | 7.0e-5 | 4.6× | 27,520 ticks | 1.9 |

**Population turnover is 11,000–27,500 ticks against the seed replicator's designed 14-tick cycle** —
a factor of ~2,000. That is the dominant structural fact about this regime: these worlds are made of
near-immortal, near-sterile programs. Mutation only matters at all because programs live so long.

### Caveat: this is not established to be mutations per generation

Along the line of descent, mutations per generation is `μ × (generation time of lineages that
actually reproduce)`. Mean lifetime equals generation time **only if reproduction is roughly uniform
across individuals** — in a stationary population each individual leaves ~1 surviving offspring, so
the two coincide under uniformity. If instead a small subpopulation does all the reproducing on short
cycles while the rest are long-lived and sterile, the line of descent passes only through the
reproducers, and this metric overestimates.

The two bounds are far apart:

- **uniform reproduction** → mutations/generation = mut/lifetime = 1.9–25, i.e. at or far above the
  error threshold
- **reproduction concentrated in lineages cycling at `T = S + 2 ≈ 42` ticks** → `μ × 42` =
  **0.003–0.037**, three orders of magnitude *below* the threshold

Nothing here distinguishes them. The §1 trace — where even the successful seed-1 organism has a
misaligned copy loop shedding junk offspring — is weak evidence for the heterogeneous case, which
would mean the reproducing lineages are nowhere near error catastrophe and the high numbers above are
an artifact of the sterile majority. Deciding this needs per-lineage tracking (§7); until then, treat
mut/lifetime as a regime diagnostic and an *upper bound* on mutational load, not as Eigen's
parameter.

## 5. The plateau is space saturation; the decline is an overshoot unwinding

At the plateau the grid is **99.7% occupied** (16,331 of 16,384 cells). That matters mechanically:
a birth requires `appendAdj` into an *empty* cell followed by a `boot`, so at full occupancy births
are capped by the rate at which cells empty. Births track deaths from that point on (0.44 vs 0.41 at
tick 6,000; 1.07 vs 1.27 at tick 30,000), and the measured net decline is **0.021 programs/tick** —
the same order as the ~0.04 estimated by hand.

The important part is that the decline is *not* a slow death. Over ticks 14,000 → 30,000:

- mean genome size **peaks at 72.4 and falls to 63.4** (maintenance destruction trimming tails)
- births **rise** 0.59 → 1.07 per tick as cells free up
- mutations per lifetime **fall** 15.6 → 6.4

Every indicator is moving back toward the sparse world's attractor. Extending the same run to
**200,000 ticks** confirms it — this is the difference between "the sim slowly dies" and "the sim has
a stable attractor it overshoots on rich initial conditions," and it is the latter:

| tick | population | occupancy | mean size | mut/lifetime |
|---|---|---|---|---|
| 20,000 | 16,142 | 98.5% | 70.1 | **12.7** |
| 60,000 | 14,431 | 88.1% | 50.0 | **3.2** |
| 100,000 | 12,512 | 76.4% | 42.9 | **2.3** |
| 150,000 | 10,723 | 65.4% | 40.6 | **1.79** |
| 200,000 | 9,581 | 58.5% | 40.5 | **2.03** |

Mutations per lifetime decays monotonically from 12.7 and **flattens at ≈ 1.8 from tick 130,000
onward** (range 1.66–2.03 over the last 70k ticks) — the same band the sparse seed-1 world occupies.
Mean genome size converges to **40.5** (range 40.4–41.1 over the same span) from a peak of 72.4. The
substrate has a genuine attractor and the boom-bust config overshoots it. Note the mutation rate is
*falling* throughout (33.5× → 4.6× baseline), so this convergence is not a mutation effect; see §4.

One caveat the run adds that was not predicted: **the error-threshold ratio and genome size
equilibrate long before density does.** Population is still falling at tick 200,000 (58.5%
occupancy), though the decline is decelerating — 0.044 programs/tick around tick 100,000, 0.023 over
ticks 150k–200k, 0.018 over the last 10k. Births and deaths are both shrinking (0.31 and 0.46 per
tick at the end). So mut/lifetime and mean size are fast variables that settle first; density is a
slow variable that had not settled within 200k ticks. Whether it asymptotes near the sparse world's ~5%
occupancy or somewhere well above it is still open.

On the initial explosion: fresh sims initialize background pools at the stationary distribution
(`SPEC.md` §Mass and Energy), so tick 0 holds a standing crop of `R_energy/D_energy` per cell —
81,920 energy grid-wide at `d_energy = 0.05`, and 409,600 at the spec default of 0.01. Measured
`total_energy` drops 82,376 → 58,703 across the explosion and then recovers to 236,732 as biomass
(and therefore storage capacity `T_cap × S`) grows.

### Why `move` is load-bearing

For a seed-style replicator of size `S` copying itself over `T = S + 2` ticks with an absorb
footprint of `f` cells, the energy viability condition (`SPEC.md` §Viability Condition) is

```
f · R_energy · T  >  S  +  T · S · M
```

Solving at `R_energy = 0.25`, `M = 1/128`:

| absorb footprint | viable sizes |
|---|---|
| 1 cell (own cell only) | **none, at any size** |
| 2 cells | **none, at any size** |
| 3 cells | S ≤ 4 |
| 5 cells (`absorb` ×4) | S ≤ 38 |

A stationary program that drains its own cell every tick earns `R_energy = 0.25` energy/tick, but a
replication cycle costs ~1 energy/tick in `appendAdj` base costs alone. It is 4× short regardless of
how efficient the genome is. Full `absorb_count = 4` buys a factor of 5 and makes sizes up to ~38
viable — which is why evolved genomes are stuffed with `absorb` (11 of 39 in the seed-1 program
above), and why mean size stalls in the 30–70 range rather than climbing freely.

Movement is the other half. A `move` costs 1 energy and lands the program on a cell holding the full
standing crop `R_energy/D_energy` — **5 energy at `d_energy = 0.05`, 25 at the default 0.01**. One
move is worth `1/D_energy` ticks of sessile income: 20× or 100×. Gliders are not wandering, they are
strip-mining, and a sessile program sits inside its own depletion halo.

This reframes the `d_energy` knob: raising it does not merely shorten the "oil age," it cuts the
per-move grazing payoff proportionally. That is the dominant effect, and it explains both why higher
`d_energy` tames the explosion and why removing the `move` (99) instructions makes behavior much less
interesting.

## 6. M2/M3 have a measurable threshold — but not in `mutation_base_log2`

### Structural analysis (from the spec)

- **`appendAdj` is the only size-increasing operation in the entire system.** `write` is
  size-preserving, `del`/`delAdj` shrink, `synthesize` makes free mass rather than instructions,
  crystallization makes resources, and nucleation makes exactly one `nop`.
- **No program can grow itself.** The ISA has local `write` and local `del` but no local append.
  Growth only ever arrives from a neighbor.
- **`appendAdj` needs a stack operand.** A size-1 program whose only byte is `appendAdj` has an
  empty stack, and Pass-1 operand capture is atomic, so it queues nothing, every tick, forever —
  while still paying its base cost of 1.

Taken together that reads like a proof that M2 is unreachable: size-1 can never grow, and reaching
size 2 requires a neighbor that is already more than a `nop`. But there is a base case, from two
details:

- **The stack persists across ticks and across mutations.** A size-1 program whose byte is a
  stack-pusher — `read` (0x55), `readAdj` (0x5D), `rand` (0x14), `getSize` (0x42), any `push N`
  (0x00–0x0F) — pushes one value per tick indefinitely. If it *later* mutates into `appendAdj`, it
  spends that accumulated stack building a neighbor, one instruction per tick.
- **`nop` opens its cell.** A freshly nucleated program executes `nop` every tick, so it is
  permanently unprotected and is therefore valid feedstock for a neighbor's `appendAdj` — including
  while it is alive. M3 does not have to route through an inert offspring and a `boot` at all.

The mutation walk is short. There are 24 four-flip paths from `nop` (0x50) to `appendAdj` (0x5F);
8 have every intermediate an assigned instruction, and two of those pass through a stack
accumulator:

```
nop(0x50) → absorb(0x51) → read(0x55) → readAdj(0x5D) → appendAdj(0x5F)
```

Every step is a single bit flip, and the residence times are favorable. A size-1 `absorb` earns
0.25 energy/tick against maintenance of `1 × M` = 0.0078 energy/tick — a **32× surplus, so it is
effectively immortal** (this is M1) and can wait arbitrarily long for the next flip. A size-1
`collect` is likewise immortal on the mass side, since maintenance falls back to free mass. `read`
and `readAdj` are cost-0 accumulators. A resourceless size-1 program with no income survives about
`(4 + 4) × 128 ≈ 1,024` ticks — free pools are pinned near the `T_cap × S = 4` threshold by decay,
then maintenance eats them at `1/128` per tick. Measured mean lifetime in the soup was ~1,190 ticks.

One caveat worth recording: **size-1 programs cannot be destroyed by other programs.** `delAdj`
fails against a size-1 target, and `absorb` never opens the cell, so an M1 absorber is immortal,
uninvadable, and occupies its cell until it happens to mutate away (~65k ticks at `k = 16`). At any
`p_spawn > 0` these accumulate. Worth watching as a grid-clogging failure mode — and note the
tension it creates: the states that survive (`absorb`, protected) are exactly the states that cannot
be built upon, while the state that can be built upon (`nop`, open) is the one that dies.

### Measured threshold

Nucleation-only soup: 128×128, **no seed programs at all**, `p_spawn = 0.001`, 30,000 ticks,
otherwise the config from §1 (`r_energy 0.25`, `r_mass 1`, `d_energy 0.05`, `d_mass 0.01`,
`maintenance_rate 0.0078125`, `t_cap 4`, `mutation_background_log2 8`, seed 12345).

| `mutation_base_log2` | max **live** size | max inert | first size > 1 | boot-births | distinct bytes | realized/baseline | milestones |
|---|---|---|---|---|---|---|---|
| 16 | 1 | 0 | never | 0 | 22 | 1.12× | M0 |
| 14 | 1 | 0 | never | 0 | 37 | 1.10× | M0 |
| 12 | 5 | 0 | tick 28,250 | 0 | 69 | 1.08× | M0–M3 (one event) |
| 10 | 18 | 3 | tick 1,900 | 0 | 157 | 1.04× | M0–M3 |
| 8 | **45** | 13 | tick 2,150 | 0 | 264 | 1.00× | M0–M3 |
| 6 | 26 | 56 | tick 650 | **23** | 279 | 0.98× | M0–M3, boot route |

All six runs equilibrate at ~7,300–7,700 programs (45% occupancy), so the difference is not
population size — it is purely mutation rate.

- At 16 and 14, nothing ever exceeds size 1. **This reproduces the §1 observation exactly**, and
  explains it: the mechanism is not missing, the walk is ~2 orders of magnitude too slow.
- Growth appears at 12, becomes routine at 10, and by 8 the soup is building **live 45-instruction
  programs** — larger than the 12-instruction minimal replicator — out of nothing but random bit
  flips and `p_spawn`. Note `max_program_size` counts live programs only, so these are alive:
  nucleated `nop`s grown in place by a neighbor, never booted.
- At 6, accreted *inert* structures are additionally booted into life 23 times.

### Replicated threshold

The single-seed sweep above made `k = 12` look marginal (one late event). Five seeds per `k` over the
transition region show it is not marginal at all — the threshold is **sharp between `k = 14` and
`k = 12`**:

| `mutation_base_log2` | replicates reaching M2/M3 | max live size per seed | median first size > 1 | max inert | boot-births |
|---|---|---|---|---|---|
| 16 | **0 / 5** | 1, 1, 1, 1, 1 | — | 0 | 0 |
| 14 | **0 / 5** | 1, 1, 1, 1, 1 | — | 0 | 0 |
| 12 | **5 / 5** | 2, 4, 4, 5, 5 | tick 17,900 | 1 | 0 |
| 10 | **5 / 5** | 14, 18, 26, 30, 31 | tick 2,750 | 4 | 0 |

Equilibrium population is flat across all four (7,281–7,617), so within this sweep nothing is a
density effect. `k = 12` is reliably above threshold but slow and small — structures of 2–5
instructions, first appearing around tick 18k. `k = 10` is a different regime: an order of magnitude
faster onset and sizes of 14–31. Distinct byte values seen in the soup track the same transition
(22 → 36 → 69 → 153).

This looked like a boundary at `k ≈ 13`. The next sweep shows that reading was too narrow.

### `p_spawn` and `r_mass`: the threshold is not a property of `k`

Holding `k` at 14 (previously 0/5) and 12, and moving the other two knobs — 7 parameter points × 2
`k` values × 3 seeds, 30k ticks each:

| `k` | `p_spawn` | `r_mass` | occupancy | total mutations | **mutations × occupancy** | max live size | M2 |
|---|---|---|---|---|---|---|---|
| 14 | 0.001 | 0.05 | 0.056 | 1,719 | 97 | 1, 1, 1 | 0/3 |
| 14 | 0.0001 | 1 | 0.079 | 2,447 | 193 | 1, 1, 1 | 0/3 |
| 14 | 0.001 | 0.25 | 0.223 | 6,872 | 1,533 | 1, 1, 1 | 0/3 |
| 14 | 0.001 | 1 | 0.462 | 14,658 | 6,773 | 1, 1, 1 | 0/3 |
| 14 | 0.001 | **4** | 0.586 | 18,806 | 11,011 | 1, 1, **31** | **1/3** |
| 14 | **0.01** | 1 | 0.895 | 29,208 | 26,140 | 1, 1, 10 | **1/3** |
| 14 | **0.1** | 1 | 0.988 | 32,421 | 32,026 | 1, 9, 9 | **2/3** |
| 12 | 0.001 | 0.05 | 0.058 | 6,938 | 399 | 1, 1, 1 | 0/3 |
| 12 | 0.0001 | 1 | 0.080 | 9,948 | 792 | 1, 1, 1 | 0/3 |
| 12 | 0.001 | 0.25 | 0.226 | 28,017 | 6,338 | 1, 1, 1 | 0/3 |
| 12 | 0.001 | 1 | 0.467 | 58,795 | 27,450 | 4, 5, 5 | 3/3 |
| 12 | 0.001 | **4** | 0.589 | 75,093 | 44,200 | 5, 5, **32** | 3/3 |
| 12 | 0.01 | 1 | 0.897 | 115,511 | 103,559 | 10, 14, 18 | 3/3 |
| 12 | 0.1 | 1 | 0.988 | 127,843 | 126,272 | 6, 10, 24 | 3/3 |

**`k = 14` is not below any threshold.** Raising either `p_spawn` (to 0.01 or 0.1) or `r_mass` (to 4)
pushes it into M2. So the earlier "`k ≈ 13` boundary" was an artifact of holding
`p_spawn = 0.001, r_mass = 1.0` fixed — it is a contour of a surface, not a property of the mutation
rate.

**The two axes collapse onto one control variable: mutations × occupancy.** Every 0/3 point sits at
≤ 6,773; every point at ≥ 11,011 produces M2. That separation holds across both `k` values and both
knobs, with no overlap. Raw mutation count does *not* collapse them — `k = 12` at `r_mass = 0.25`
has 28,017 mutations and gets nothing, while `k = 14` at `p_spawn = 0.01` has a comparable 29,208 and
succeeds. Occupancy is the difference (0.226 vs 0.895), and it belongs there for a mechanical reason:
**accretion is a two-body event.** An accretor needs an adjacent target, so the rate carries a
density factor that a mutation count alone misses.

**But the axes are not interchangeable — they trade frequency against size.** `r_mass = 4` reaches
M2 with roughly a third of the effective trials of `p_spawn = 0.1`, yet builds the largest structures
in the entire sweep (31 and 32 instructions, versus 9–24 for the high-`p_spawn` points). A plausible
mechanism: `appendAdj` costs 1 free *mass* per append, and a size-1 `collect` program — which sits
one bit flip from `absorb` on the walk — reaches a steady-state free-mass pool of `4 + r_mass/d_mass`,
i.e. 9, 29, 104, 404 across the four levels. That is the accretion budget. So `p_spawn` buys more
attempts; `r_mass` buys bigger builds. Treat this as a hypothesis: three seeds is thin, and the two
low-`r_mass` points are confounded because they also fail the mutations × occupancy threshold.

**The spec default `r_mass = 0.05` is dead soup.** Equilibrium population 924 (5.6% occupancy) and
0/3 at both `k` values. Every result in this note that shows anything happening uses `r_mass = 1`,
20× the suggested default, which is worth remembering when reading the spec's own parameter table.

No boot-births anywhere in this sweep — the boot route still requires `k = 6` (§6 first table).

**None of this is M4 or M5.** The accreted code is not a copy of anything, there is no evidence any
of it survived on its own, and nothing replicated. What the sweep establishes is narrower and still
useful: the size-1 ceiling is a *rate*, not a structural impossibility, and the rate has a
threshold between `k = 14` and `k = 10` that the existing runner can map.

The tension this exposes is the one worth thinking hardest about: the mutation rate that produces
M2/M3 (`k ≤ 12`) is far above the rate a seeded replicator survives (§4 — the sparse world holds at
≈ 1 mutation/birth). Even if a random accretion ever stumbled onto a copy loop at `k = 8`, it would
be carrying a mutational load of ~15 per program lifetime before it could found a lineage — subject
to the §4 caveat, which could soften this by orders of magnitude if reproduction is heterogeneous.
**M5 may nonetheless need a rate that varies in space or time, not a different constant.** Options, roughly in order of spice: a
spatially/temporally varying mutation field (the existing ambient-noise proposal is the natural
vehicle); decoupling the rate for size-1 programs from the rate for larger ones; or making `k` a
function of program size, defensible as "short polymers are chemically unstable," which would give a
genuine ramp rather than a flat knob.

## 7. Instrumentation gaps

- **Mutations are not split by cause.** `event_totals.mutations` merges baseline and
  background-stressed events. Everything in §3 had to be inferred by comparing realized totals to
  the population integral.
- **No size histogram or size-1 opcode census.** This is what blocks confirming M1 and M4 at all:
  there is no way to ask "how many immortal size-1 absorbers exist" or "did any accreted program
  survive on its own."
- **No lineage or genome tracking.** This is the largest gap. It blocks M5 and M6 (`unique_genomes`
  would not distinguish an emerged replicator from drift), *and* it is what leaves §4 with a
  three-order-of-magnitude ambiguity: without knowing which programs actually reproduce and on what
  cycle, mutations-per-lifetime cannot be converted into mutations-per-generation. Tracking parent
  ID and birth tick per program would resolve both.

## 8. Runs

All runs used `proteus-run`/`proteus-batch` per `docs/RUNNER-SPEC.md`, 30,000 ticks unless noted.
The first batch used the debug profile (the crate tunes `dev` for runtime speed); the 200k run and the
replicate sweep used `--release --features rayon`.

| run | grid | seed | seed program | notes |
|---|---|---|---|---|
| boom-bust | 128×128 | 5013216203605585 | 14-instr, at (32,32) | explosion → 99.7% occupancy → slow decline |
| seed1-glider | 128×128 | 1 | same | sparse stable ecology, ~780 programs |
| attractor-200k | 128×128 | 5013216203605585 | same as boom-bust | 200,000 ticks; convergence test for §5 |
| spawn-k{16,14,12,10,8,6} | 128×128 | 12345 | **none**, `p_spawn = 0.001` | M2/M3 threshold sweep |
| em-k{16,14,12,10}-s{0..4} | 128×128 | 12345, 777, 20260813, 98765, 31337 | **none**, `p_spawn = 0.001` | 20-run replicate sweep across the transition |
| ax-p{…}-m{…}-k{14,12}-s{0..2} | 128×128 | 12345, 777, 31337 | **none** | 42-run `p_spawn` × `r_mass` sweep; `p_spawn ∈ {1e-4,1e-3,1e-2,1e-1}`, `r_mass ∈ {0.05,0.25,1,4}` |

Seeded runs used the config from §1: `r_energy 0.25`, `r_mass 1`, `d_energy 0.05`, `d_mass 0.01`,
`t_cap 4`, `maintenance_rate 0.0078125`, `maintenance_exponent 1`, `local_action_exponent 1`,
`n_synth 1`, `inert_grace_ticks 10`, `p_spawn 0`, `mutation_base_log2 16`,
`mutation_background_log2 8`. Seed program `[81,81,81,81,83,64,66,48,85,95,49,100,99,99]` with
`free_energy 20`, `free_mass 12`.

All three sweep axes are now covered: `mutation_base_log2` replicated 5× across
`k ∈ {16, 14, 12, 10}`, and `p_spawn` × `r_mass` replicated 3× at `k ∈ {14, 12}`. Remaining
weaknesses: `k ∈ {8, 6}` — where the largest structures and the only boot-births appeared — is still
single-seed and was not crossed with the other two axes; three seeds is thin for the
frequency-vs-size claim; and the mutations × occupancy collapse is established over roughly two
decades, with only one point between 6,773 and 11,011 to locate the onset.

## 9. Follow-ups

- **MUTATION-CAUSE-SPLIT** — split the mutation metric into baseline and background-stressed
  counters so the stress regime is directly observable.
- **EMERGENCE-CENSUS** — add a program-size histogram, an opcode census, and per-program parent ID
  and birth tick. This is the blocker for deciding M1 and M4 at all, for detecting immortal size-1
  absorber accumulation, and for converting mutations-per-lifetime into mutations-per-generation
  (§4 caveat). Highest-leverage remaining item.
- **EMERGENCE-SWEEP** — all three axes done (§6). M2 onset collapses onto mutations × occupancy
  ≈ 10⁴. Remaining: replicates at `k ∈ {8, 6}` crossed with `p_spawn`/`r_mass` to find the cheapest
  route to the boot route (M3-by-boot) and beyond; and more seeds to firm up the
  frequency-vs-size split between the two knobs.
- ~~**ATTRACTOR-CONVERGENCE**~~ — done (§5). Mutations-per-lifetime and mean genome size converge as
  predicted; density does not settle within 200k ticks. The open follow-on is where occupancy
  asymptotes, which needs a longer run or a smaller grid.
- **SEED1-TRACE** — verify the two §1 predictions about the seed-1 genome (offspring receive the
  literal byte `39`; `boot` fires before the copy loop) by inspecting a live offspring at boot time.
