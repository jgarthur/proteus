# Seed-organism ladder and library variants for Proteus v0.4.0

Date: 2026-08-23. This analysis targets the current `docs/SPEC.md` and Rust
engine. The v0.2 sketches in `2026-03-15_claude_seeds.md` supplied ideas only.
Every organism labeled verified below was re-derived and run; recommendation
ranges and Retrychain are explicitly analytical or hypothetical.
The workflow was hand budget and Pass trace first, then a release build,
cadence observation, and finally five-seed horizon runs; failed seeds remain in
the result tables and artifacts.

## 1. Summary table

Each verification run bootstrapped one live program at `(32,32)` on a 64x64
grid. “Seeds” means simulation RNG seeds 1–5. At tick 20,000, a seed passes iff
`births > deaths` and runner `population >= 4`; `p_spawn_log2=null`, so all
births are constructed boot births. “Verified regime” is an executed point, not
an empirically bracketed parameter minimum.

| Organism | Size | Niche | Minimal regime: analytic floor → verified point | Viable? |
|---|---:|---|---|---:|
| **Radchain** (`radchain`) | 7 | stationary chain; collect mass, pay copy bases directly from radiation | at `d*=7`, solitary mean `rE>=.802`, `rM>=.809` → verified `2/1.25`; `cap=4,maintenance_log2=10,alpha=.60,beta=1,n=1,mut=63/63` | **3/5** |
| Sunchain (`sunchain`) | 8 | captured-energy stationary chain | at `d*=7`, conservative `rE>=.908`, `rM>=.899` → verified `1.25/1.25` with root preload `16E/16M`; `cap=4,maintenance_log2=10,alpha=.55,beta=1,n=1,mut=63/63` | **3/5** |
| **Freesynth** (`freesynth`) | 7 | no ambient mass; background-paid, just-in-time synthesis | at `d*=7`, mean `rE>=1.605`, `rM=0`, with `n=0,maintenance=null` → verified `rE=2`; `cap=4,alpha=.75,beta=1,mut=63/63` | **3/5** |
| Frontforge (`frontforge`) | 9 | own+front capture; `n_synth=1` zero-mass builder | at `d*=7`, conservative `rE>=1.401`, `rM=0` → verified `2/0`; `cap=4,maintenance_log2=10,alpha=.65,beta=1,n=1,mut=63/63` | **3/5** |
| Mixedforge (`mixedforge`) | 10 | Frontforge plus collected-mass reserve | at `d*=7`, conservative `rE>=1.445`, `rM=0` → verified `2/0` with root preload `32E`; otherwise Frontforge regime | **4/5** |
| **Squarestep** (`squarestep`) | 9 | clockwise mobile collector on a four-cell orbit | at `d*=7`, established-orbit means `rE>=.249`, `rM>=.224` → verified `.5/.3`; `cap=4,maintenance_log2=14,alpha=.55,beta=1,n=1,mut=63/63` | **5/5** |
| Wildstep (`wildstep`) | 11 | captured-energy random sparse colonizer | at `d*=7`, conservative fresh-cell `rE>=.105`, `rM>=.0965` → verified `.25/.2` with root preload `16E/16M`; `cap=4,maintenance_log2=14,alpha=.5,beta=1,n=1,mut=63/63` | **5/5** |

The smallest exact counted-loop architecture derived and tested here has a
six-byte copy/boot core: length source, counted loop, self-read, adjacent append,
loop-back, boot. Rung 1 adds one mass acquisition; rung 2 substitutes one
synthesis instruction; repeatable movement adds a direction change and `move`.
This is not an exhaustive proof against every exotic cyclic/stack construction.

### Seed-library cards

These names and sentences are intended to be usable directly in a picker. The
four comparison variants are preserved run artifacts, not extra ladder rungs.

| Display name | Slug | Picker sentence | Evidence status |
|---|---|---|---|
| Radchain | `radchain` | A seven-byte stationary chain replicator that collects ambient mass and pays copy energy straight from background radiation. | verified 3/5; exact genome retained |
| Sunchain | `sunchain` | An eight-byte Radchain that absorbs its own-cell radiation into an energy buffer before copying. | verified 3/5; exact genome retained |
| Freesynth | `freesynth` | A seven-byte copier for massless worlds that synthesizes each offspring byte just in time with zero synthesis surcharge. | verified 3/5; requires disabled maintenance in this minimal form |
| Frontforge | `frontforge` | A nine-byte zero-mass builder that absorbs its own and front cells, then converts captured energy into each offspring byte. | verified 3/5; genome-unstable long-term |
| Mixedforge | `mixedforge` | A ten-byte Frontforge that also collects ambient mass into a surplus buffer when both mass pathways exist. | verified 4/5; genome-unstable long-term |
| Squarestep | `squarestep` | A nine-byte mobile collector that births forward, turns clockwise, and forages a repeatable four-cell orbit. | verified 5/5 colonizer; genome-unstable when dense |
| Wildstep | `wildstep` | An eleven-byte absorber/collector that chooses a random heading after each birth to colonize fresh cells. | verified 5/5 colonizer; genome-unstable when dense |

Direct-background and captured-energy forms are meaningfully different. Direct
payment saves an `absorb`, but can pay base costs only and invokes the
per-consumed-quantum background mutation trigger. Captured free energy can pay
additional costs and normally uses the baseline mutation path, but acquisition
resolves in Pass 3 and footprints compete. Validation used mutation exponents
63, so the ecological mutation difference remains untested.

These are functional analogues, not controlled A/B pairs: synthesis forms also
change `n_synth` and footprint, while mobile forms change direction policy.

| Family | Direct-background form | Captured-energy functional analogue |
|---|---|---|
| Chain | Radchain `[83,66,48,85,95,49,100]` | Sunchain `[81,83,66,48,85,95,49,100]` |
| Synthesis | Freesynth `[66,48,88,85,95,49,100]`, necessarily `n_synth=0` | Frontforge `[81,81,66,48,88,85,95,49,100]`, tested `n_synth=1`; Mixedforge additionally buffers collected mass |
| Mobile | Squarestep `[83,66,48,85,95,49,100,64,99]` | Wildstep `[81,83,66,48,85,95,49,100,20,73,99]` (also changes orbit policy) |

### Executed horizon matrix

Cells are `population/births/deaths`; ✓ applies the formal criterion. Every row
used five fresh current-schema manifests. The three primary rows and Frontforge
start the root at `0E/0M`; preserved Sunchain uses `16E/16M`, Mixedforge `32E/0M`,
and Wildstep `16E/16M`. Constructed descendants always start with zero free
resources unless the target cell already held some.

| Organism | Seed 1 | Seed 2 | Seed 3 | Seed 4 | Seed 5 | Passes |
|---|---:|---:|---:|---:|---:|---:|
| Radchain | 7/6/0 ✓ | 4/3/0 ✓ | 6/5/0 ✓ | 3/2/0 | 2/1/0 | **3/5** |
| Sunchain | 3/2/0 | 6/5/0 ✓ | 4/3/0 ✓ | 11/10/0 ✓ | 2/1/0 | **3/5** |
| Freesynth | 7/6/0 ✓ | 4/3/0 ✓ | 6/5/0 ✓ | 3/2/0 | 2/1/0 | **3/5** |
| Frontforge | 3/2/0 | 4/3/0 ✓ | 7/6/0 ✓ | 8/7/0 ✓ | 2/1/0 | **3/5** |
| Mixedforge | 8/7/0 ✓ | 11/10/0 ✓ | 9/8/0 ✓ | 5/4/0 ✓ | 2/1/0 | **4/5** |
| Squarestep | 4096/4131/36 ✓ | 4095/4141/47 ✓ | 4096/4136/42 ✓ | 4096/4142/47 ✓ | 4096/4143/48 ✓ | **5/5** |
| Wildstep | 4096/4131/36 ✓ | 4095/4143/49 ✓ | 4095/4142/48 ✓ | 4095/4169/76 ✓ | 4095/4141/47 ✓ | **5/5** |

Provenance is explicit in each completion record. Eighteen earlier preserved
variant/cadence artifacts record commit `9c78d30`; 31 primary, trace, and
rotation artifacts record current commit `b962834`. The intervening changes are
documentation, frontend, and observation encoding—not simulation dynamics—and
the release build succeeded again at `b962834`. Examples: [earlier completion](../../rust/runs/seed-organisms/outputs/rung1-min8-seed-1/completion.json),
[current completion](../../rust/runs/seed-organisms/outputs/rung1-min7-seed-1/completion.json).

### Shared grounding ledger

Budget and trace tables cite these current rules. Quoted cost text is from the
SPEC instruction rows.

| Key | Current rule and implementation |
|---|---|
| G0 | SPEC rows quote `synthesize` “Base Cost 1” (`docs/SPEC.md:425`), `appendAdj` “Base Cost 1” (`:444`), `move` “Base Cost 1” (`:448`), and `boot` “Base Cost 0” (`:449`); `collect`, `read`, and the remaining primary locals are cost 0 (`:383-425`). Rust's complete map is `rust/src/opcode.rs:150-173`. |
| G1 | Local budget is `max(1,floor(size^alpha))`; execution stops at budget exhaustion or the first nonlocal (`docs/SPEC.md`, “Local Throughput”; `rust/src/pass1.rs:134-185`). |
| G2 | Base cost pays free energy first, then background radiation; background can never pay an additional cost (`docs/SPEC.md`, “Energy Payment”; `rust/src/pass1.rs:681-751,787-802`). |
| G3 | `absorb` and `collect` rows say “Base Cost 0.” They mark flags in Pass 1, then resolve after Pass 2 in Pass 3. Absorb counts 1/2/3/4 cover own / own+front / own+front+sides / all five cells; collect converts own-cell background mass (`rust/src/pass1.rs:532-559`; `rust/src/pass3.rs:262-328,602-610`). |
| G4 | `appendAdj`: “Base Cost 1”, “Add'l Cost 1 mass.” Its byte is captured in Pass 1; a winning append pays mass and tail-appends or creates a one-byte inert body in Pass 2 (`rust/src/pass1.rs:681-751`; `rust/src/pass2.rs:288-320,392-420,502-517`). |
| G5 | `synthesize`: “Base Cost 1”, “E = N_synth.” It immediately spends additional free energy and creates one free mass in Pass 1 (`rust/src/pass1.rs:617-627`). |
| G6 | `move`: “Base Cost 1.” It succeeds only into an empty pre-Pass-2 target and moves the program plus free pools; background stays (`rust/src/pass2.rs:295-306,447-500`). |
| G7 | `boot`: “Base Cost 0.” It accepts any inert body, sets live, age 0, `IP=0`, and newborn, but does not validate length or contents (`rust/src/pass2.rs:295-306,454-478`). First append randomizes child `Dir/ID`; `IP/Src/Dst/Flag/Msg/LC=0`, stack empty (`rust/src/model.rs:64-75,342-361`; `rust/src/pass2.rs:502-517`). |
| G8 | Expected maintenance over `T` ticks is `T*S^beta*2^-k`; payment order is free E, free M, then tail truncation. Free E/M decay only above `t_cap*S` (`docs/SPEC.md`, “Maintenance” and “Decay”; `rust/src/pass3.rs:461-486,648-714`). |
| G9 | Define the dimensionless refill sum `F(T,D)=(1-(1-D)^T)/D`, with `F(T,0)=T`; expected refill is `r*F`. For `D>0`, untouched backgrounds initialize at stationary mean `r/D`; with null decay they initialize at zero (`docs/SPEC.md`, resource model/reference budget; `rust/src/simulation.rs:515-579`). |
| G10 | Runner `population` counts occupied live plus inert cells. Horizon births/deaths are cumulative `summary.final_metrics.event_totals`; top-level per-row values are one-tick deltas. Bootstrap and first-append inert creation are not births; boot/spawn transitions are (`docs/RUNNER-SPEC.md`, observation/bootstrap sections; `rust/src/observe.rs:342-380`; `rust/src/runner/single.rs:82-106`; `rust/src/simulation.rs:361-367`). |
| G11 | Mutation applies only to programs live at tick start; maintenance checks tick-start existence, newborn state, and inert grace; age increments only for programs live at tick start (`rust/src/pass3.rs:553-570,632-670,717-730`). Thus a just-created inert body and a just-booted newborn are exempt on those respective ticks. |

## 2. Organism ladder

### 2.1 Rung 1 — Radchain

#### (a) Commented assembly

Six bytes perform copying and boot. `collect` is the seventh and only foraging
instruction; append base energy is paid directly from background [G2]. Removing
`collect` leaves descendants with no source of the seven required free-mass
quanta, so this is the smallest counted-loop rung-1 design derived and tested.

```text
; === Radchain: 7 instructions; B=floor(7^.60)=3 ===
collect         ; cycle tick 1: request own-cell mass collection for Pass 3
getSize         ;               push 7
for             ;               LC=7; local budget ends, then collection resolves
  read          ; tick 2: push self[Src mod 7], Src++
  appendAdj     ;         pay 1 base E; Pass 2 spends 1M and appends byte 0
next            ; ticks 3-8: next/read/append bytes 1..6
boot            ; tick 9: activate the completed inert child
                ; tick 10: next cycle; child executes for the first time
```

Seven successful reads add seven to raw `Src`, restoring byte-zero alignment
modulo size. This shortcut is valid only while size remains seven; unlike the
SPEC seed, Radchain does not reset `Src` explicitly (`read` row;
`rust/src/pass1.rs:575-584`).

#### (b) Decimal byte array

```json
[83, 66, 48, 85, 95, 49, 100]
```

This is ready for `seed_programs[].code` (API-SPEC §8) and runner
`bootstrap.programs[].code`.

#### (c) Per-cycle energy and mass budget

Executed point: `S=7`, `T=9`, `D*=2^-7`, `M=2^-10`, `beta=1`,
`t_cap=4`. `F(9,1/128)=8.7238173`; the free-resource threshold is 28.
The direct-background row is conservative: the surviving contents of a fully
drained pool are a lower bound on distributed base-cost supply because spending
earlier also prevents later decay [G2,G9].

| Pool | Per-cycle derivation | Expected quanta |
|---|---|---:|
| Background energy available | `rE*F=2*8.7238173` [G2,G9] | +17.4476 |
| Append bases | `7*1E` [G0,G4] | -7.0000 |
| **Energy balance** | no absorbed/free E; maintenance falls through to mass | **+10.4476** |
| Collected mass | `rM*F=1.25*8.7238173`, own cell [G3,G9] | +10.9048 |
| Append mass | `7*1M` [G4] | -7.0000 |
| Maintenance mass | `9*7/1024` [G8] | -0.0615 |
| **Mass balance before cap loss** | `10.9048-7-.0615` | **+3.8432** |

The 28-unit cap exceeds the full `7.0615` expected mass bill. Once storage is
capped, decay can remove accumulated surplus but cannot touch the protected
bill; clean-cycle free-energy decay is zero because none is captured [G8].

#### (d) First-cycle Pass 1/2/3 trace

Cycle tick 1 is engine tick 0 and completed metrics row 1; the engine increments
its tick only after all passes (`rust/src/simulation.rs:315-370`).

| Cycle tick | Pass 1 | Pass 2 | Pass 3 / state after tick |
|---:|---|---|---|
| 1 | `collect; getSize; for`; stack empty after `for`, `LC=7`, `IP=read` | none | collect converts own background mass; maintenance then above-cap decay [G1,G3,G8] |
| 2 | `read` pushes byte 83 and makes parent `Src=1`; `appendAdj` base 1E pays from background, pops the byte, advances IP | winner spends 1 source mass and creates inert `[83]` with random `Dir/ID` [G2,G4,G7] | incoming write resets inert timer; new inert is exempt [G11] |
| 3–8 | `next` makes `LC=6..1`; `read`; pay append base and queue bytes 1..6 | one successful append per tick, ending at exact size 7 | writes keep the inert child inside grace; parent maintenance |
| 9 | `next` makes `LC=0`; queue zero-cost `boot` | child becomes live, `IP=0`, age 0, newborn [G7] | boot birth; child skips maintenance, mutation, and age this tick [G11] |
| 10 | parent and child begin `collect; getSize; for` | none | child's first collection resolves |

At creation the child has `IP=Src=Dst=Flag=Msg=LC=0`, empty stack, age 0,
random `Dir/ID`; boot preserves those fields except resetting `IP` and age [G7].
The cadence-1 run observed inert sizes 1 through 7 in rows 2–8 and its first
boot birth in row 9: [manifest](../../rust/runs/seed-organisms/trace-min7-seed-1.json),
[metrics](../../rust/runs/seed-organisms/outputs/trace-min7-seed-1/metrics.jsonl).

#### (e) Verified viable regime, analytic floor, and sensitivity

| Parameter | Executed value | Bound / sensitivity |
|---|---:|---|
| `r_energy` | 2.0 | Mean solitary floor `7/F9=.8024`; direct payment has timing/Poisson risk, so the run uses a wide margin. |
| `r_mass` | 1.25 | Mean floor including maintenance `.8095`; ambient mass is essential. |
| `d_energy_log2`, `d_mass_log2` | 7, 7 | Floors use `D=1/128`; faster decay raises them. |
| `t_cap` | 4.0 | Threshold 28 protects more than a full mass bill. Direct background has no cap. |
| `maintenance_rate_log2` | 10 | Expected mass fallback is included. Lower `k` raises the mass floor and tail-loss risk. |
| `maintenance_exponent` | 1.0 | Cost is `S^beta`; `beta>1` raises it superlinearly. |
| `n_synth` | 1 | Unused. |
| `local_action_exponent` | .60 | Gives clean `B=3`. With zero initial mass, fidelity needs `B<=4`; at `B>=5`, append queues before Pass-3 collection and loses byte 0. This is an ordering bound, not a claim that every lower alpha is equally fit. |
| `mutation_base_log2`, `mutation_background_log2` | 63, 63 | Every background-paid append contributes one stressed quantum. Lower background exponent directly loads this genome. |

There is no turn: after a birth, the parent keeps pointing at its protected
child. Each program can normally make one child, and the child's independently
random `Dir` continues a chain. Immediate reversal is 1/4; later intersection
also terminates it. This is the dominant non-resource sensitivity.

#### (f) Executed manifest and empirical check

```sh
cd rust
cargo build --release
cargo run --release --bin proteus-batch -- \
  --manifest runs/seed-organisms/rung1-min7-batch.json
```

Exact [batch manifest](../../rust/runs/seed-organisms/rung1-min7-batch.json) and
five expanded manifests produced the Radchain row in the horizon matrix:
**3/5 viable**. Seeds 4 (`3/2/0`) and 5 (`2/1/0`) failed the population
threshold and are reported as failures. All five had zero mutation, zero death,
one unique genome, mean/max size 7; that exact-genome and lineage-depth evidence
supports, but does not cell-inspect, the random-chain explanation.

#### Preserved captured-energy counterpart — Sunchain

##### (a) Commented assembly

```text
; === Sunchain: 8 instructions; B=floor(8^.55)=3 ===
absorb          ; cycle tick 1: request own-cell energy capture for Pass 3
collect         ;               request own-cell mass collection for Pass 3
getSize         ;               push 8; budget ends before `for`
for             ; tick 2: LC=8
  read          ;         push byte 0 and advance Src
  appendAdj     ;         pay 1E; Pass 2 spends 1M and appends
next            ; ticks 3-9: next/read/append bytes 1..7
boot            ; tick 10: boot child; both start new cycles on tick 11
```

##### (b) Decimal byte array

```json
[81, 83, 66, 48, 85, 95, 49, 100]
```

##### (c) Per-cycle budget

At `S=8`, `T=10`, `D*=2^-7`, `M=2^-10`, and cap 32, `F10=9.6556625`.
One-cell absorb and collect each yield `1.25*F10=12.0696` [G3,G9].

| Pool | Per-cycle derivation | Expected quanta |
|---|---|---:|
| Captured energy | `1.25*F10` | +12.0696 |
| Append bases | `8*1E` [G4] | -8.0000 |
| Maintenance | `10*8/1024` [G8] | -0.0781 |
| Conservative cap-loss bound | `10*(1/128)*12.0696` [G8] | -0.9429 |
| **Energy balance** | sufficient lower bound | **+3.0485** |
| Collected mass | `1.25*F10` | +12.0696 |
| Append mass | `8*1M` [G4] | -8.0000 |
| Conservative cap-loss bound | `10*(1/128)*12.0696` | -0.9429 |
| **Mass balance** | maintenance paid from energy | **+3.1266** |

The decay terms are deliberately sufficient upper bounds, not expected loss:
one clean capture is below cap 32, so clean-path loss is zero unless surplus
has accumulated.

##### (d) First-cycle Pass trace and boot state

| Cycle tick | Pass 1 | Pass 2 | Pass 3 / state after tick |
|---:|---|---|---|
| 1 | `absorb; collect; getSize`; `IP=for` | none | own-cell E/M acquisition resolves, then maintenance/decay [G3,G8] |
| 2 | `for; read`; append base 1E is paid and byte 0 captured | spend 1M; create one-byte inert child [G4,G7] | incoming write resets grace; new inert exempt [G11] |
| 3–9 | `next; read`; pay/queue one append for bytes 1–7 | one append per tick | construction remains active |
| 10 | `next` sets `LC=0`; queue `boot` | child becomes live, `IP=0`, age 0, newborn | birth; newborn exemptions apply [G7,G11] |
| 11 | parent and child restart at `absorb` | none | child's first acquisition resolves |

At first append the child has random `Dir/ID`, all registers including `Src` and
`Dst` zero, and an empty stack; boot preserves that state except the documented
live/age/IP/newborn changes [G7]. Cadence evidence:
[manifest](../../rust/runs/seed-organisms/trace-rung1-seed-1.json),
[metrics](../../rust/runs/seed-organisms/outputs/trace-rung1-seed-1/metrics.jsonl);
the first boot birth is observed in row 10.

##### (e) Verified regime and sensitivity

| Parameter | Executed value | Bound / sensitivity |
|---|---:|---|
| `r_energy`, `r_mass` | 1.25, 1.25 | Conservative sufficient floors are about `.908/.899`; stochastic timing needs margin. |
| `d_energy_log2`, `d_mass_log2` | 7, 7 | Faster decay raises both refill floors. |
| `t_cap` | 4.0 | Per-program threshold 32 exceeds either single acquisition. |
| `maintenance_rate_log2`, `maintenance_exponent` | 10, 1.0 | Included in the energy ledger; harsher maintenance can spill into mass. |
| `n_synth` | 1 | Unused. |
| `local_action_exponent` | .55 | `B=3` forces harvest before byte-zero append; zero-preload children need `B<=5` for that ordering. |
| `mutation_base_log2`, `mutation_background_log2` | 63, 63 | Captured energy normally avoids stressed background mutation; lower values were not tested. |

Its fixed direction retains Radchain's one-child-chain limit. The one-call
absorb footprint is own-cell only; the tradeoff is buffer/cap timing rather
than a wider contested footprint [G3].

##### (f) Executed manifest and empirical check

The empirical root was preloaded with `16E/16M`; constructed children were not.
Executed [batch](../../rust/runs/seed-organisms/rung1-min8-batch.json): seed
results `3/2/0` fail, `6/5/0` pass, `4/3/0` pass, `11/10/0` pass, `2/1/0`
fail, hence **3/5**. All genomes remained exact size 8 with zero mutations.
Sunchain costs one byte but generally pays append from free energy, avoiding the
stressed mutation path when its buffer is sufficient.

### 2.2 Rung 2 — Freesynth

#### (a) Commented assembly

Freesynth replaces Radchain's `collect` with `synthesize`. It is not literally
energy-free: each synth still has its 1E base cost. The name denotes
`n_synth=0`, so there is no *additional* free-energy cost and background can pay
the whole synthesis cost [G2,G5]. This is the simplest mass-independent design.

```text
; === Freesynth: 7 instructions; B=floor(7^.75)=4 ===
getSize         ; cycle tick 1: push 7
for             ;               LC=7
  synthesize    ;               background pays 1E base; n_synth=0; make 1M
  read          ;               push byte 0, Src=1; budget ends with byte retained
  appendAdj     ; tick 2: background pays 1E; Pass 2 spends the staged mass
next            ; ticks 3-8: next/synthesize/read/append bytes 1..6
boot            ; tick 9: activate child; next cycle begins tick 10
```

Ordering `synthesize` before `read` is essential. If synth cannot pay, it stalls
before `Src` advances; ordering it after `read/append` would create no mass in
time for Pass 2 [G2,G4,G5].

#### (b) Decimal byte array

```json
[66, 48, 88, 85, 95, 49, 100]
```

#### (c) Per-cycle energy and mass budget

Executed point: `S=7`, `T=9`, `rE=2`, `rM=0`, `D*=2^-7`,
`maintenance_rate_log2=null`, `beta=1`, `n_synth=0`, cap 28.

| Pool | Per-cycle derivation | Expected quanta |
|---|---|---:|
| Background energy available | `rE*F9=2*8.7238173` [G2,G9] | +17.4476 |
| Seven synth bases | `7*1E` [G0,G5] | -7.0000 |
| Seven append bases | `7*1E` [G0,G4] | -7.0000 |
| Maintenance / free-energy decay | disabled / no captured free E | 0.0000 |
| **Energy balance** | `17.4476-14` | **+3.4476** |
| Synthesized mass | `7*1M` [G5] | +7.0000 |
| Append mass | `7*1M` [G4] | -7.0000 |
| Maintenance / mass decay | disabled; at most one staged M, below cap 28 | 0.0000 |
| **Mass balance** | just-in-time conversion | **0.0000** |

This exact mass balance is why the minimal regime disables maintenance. A
positive maintenance draw can consume the one mass staged between cycle ticks
1 and 2, or truncate code after later exact spends; a robust finite-maintenance
Freesynth needs an extra synthesis surplus and is no longer seven bytes.

#### (d) First-cycle Pass 1/2/3 trace

| Cycle tick | Pass 1 | Pass 2 | Pass 3 / state after tick |
|---:|---|---|---|
| 1 | `getSize; for`; synth base 1E pays from background; with `n=0` synth creates 1M; `read` pushes byte 66 and sets `Src=1` | none | no harvest; staged 1M is below cap and maintenance is disabled |
| 2 | append base 1E pays from background; operand is popped and IP advances | append spends staged 1M, creates inert `[66]` [G4,G7] | inert creation is exempt [G11] |
| 3–8 | `next; synthesize; read`; append base also pays in Pass 1 | Pass 2 spends the mass just created and appends bytes 1..6 | active construction continues |
| 9 | `next` makes `LC=0`; queue `boot` | child becomes live at `IP=0`, age 0, newborn [G7] | first boot birth; newborn exemptions apply [G11] |
| 10 | parent and child start at `getSize` | none | next cycle |

Child registers and stack start exactly as in Radchain [G7]. The cadence-1 run
observed inert sizes 1–7 in rows 2–8 and boot in row 9:
[manifest](../../rust/runs/seed-organisms/trace-synth7-bg-seed-1.json),
[metrics](../../rust/runs/seed-organisms/outputs/trace-synth7-bg-seed-1/metrics.jsonl).

#### (e) Verified viable regime, analytic floor, and sensitivity

| Parameter | Executed value | Bound / sensitivity |
|---|---:|---|
| `r_energy` | 2.0 | Mean solitary floor `14/F9=1.6048`; two background quanta are consumed on most copy ticks. |
| `r_mass` | 0.0 | Exact minimum; no background mass is used. |
| `d_energy_log2`, `d_mass_log2` | 7, 7 | Energy floor uses `D=1/128`; `d_mass` is inert on the clean path. |
| `t_cap` | 4.0 | Clean path stores at most 1M, below threshold 28. |
| `maintenance_rate_log2` | null | Structural requirement of this exact-balance seven-byte form, not a default recommendation. |
| `maintenance_exponent` | 1.0 | Inert while maintenance is null; relevant again in a surplus variant. |
| `n_synth` | 0 | Hard boundary. At `n>=1`, additional energy must be free energy; background alone cannot pay it [G2,G5]. |
| `local_action_exponent` | .75 | Gives traced `B=4`. Other budgets rephase the first append; synth-before-read keeps resource failure cursor-safe. |
| `mutation_base_log2`, `mutation_background_log2` | 63, 63 | Fourteen background quanta per clean cycle feed the stressed trigger; this design is highly sensitive to a lower background exponent. |

Freesynth retains Radchain's one-child random-chain topology.

#### (f) Executed manifest and empirical check

```sh
cd rust
cargo run --release --bin proteus-batch -- \
  --manifest runs/seed-organisms/rung2-synth7-bg-batch.json
```

Exact [batch](../../rust/runs/seed-organisms/rung2-synth7-bg-batch.json): seeds
1–5 produced `7/6/0`, `4/3/0`, `6/5/0`, `3/2/0`, `2/1/0`, exactly matching
Radchain seed-for-seed. Result: **3/5 viable**; seeds 4 and 5 are explicit
failures. Every run had zero mutation, zero death, one unique exact seven-byte
genome. Matching cycle timing and lineage outcomes strongly support the shared
topology explanation.

#### Preserved captured-energy counterparts — Frontforge and Mixedforge

##### Frontforge

###### (a) Commented assembly

```text
; === Frontforge: 9 instructions; B=floor(9^.65)=4 ===
absorb          ; cycle tick 1: latch own cell
absorb          ;               expand footprint to own+front
getSize         ;               push 9
for             ;               LC=9; harvest resolves after Pass 2
  synthesize    ; tick 2: pay 1E base + 1 free E; create 1M
  read          ;         read byte 0 only after synthesis succeeds
  appendAdj     ;         pay 1E base; Pass 2 spends the new mass
next            ; ticks 3-10: synthesize/read/append bytes 1..8
boot            ; tick 11: boot child; restart tick 12
```

This is the smallest `n_synth=1`, zero-ambient-mass form tested. Two absorbs in
one tick expand a single latched footprint; they do not point independently
[G3].

###### (b) Decimal byte array

```json
[81, 81, 66, 48, 88, 85, 95, 49, 100]
```

###### (c) Per-cycle budget

At `S=9`, `T=11`, `rE=2`, `rM=0`, `D*=2^-7`, `M=2^-10`, cap 36,
and `n_synth=1`, `F11=10.5802`.

| Pool | Per-cycle derivation | Expected quanta |
|---|---|---:|
| Captured energy, own+front | `2 cells * 2 * F11` [G3,G9] | +42.3209 |
| Synth bases + additional | `9*(1+1)E` [G5] | -18.0000 |
| Append bases | `9*1E` [G4] | -9.0000 |
| Maintenance | `11*9/1024` [G8] | -0.0967 |
| Conservative cap-loss bound | `11*(1/128)*42.3209` | -3.6370 |
| **Energy balance** | sufficient lower bound | **+11.5872** |
| Synthesized / appended mass | `+9M-9M` [G4,G5] | **0.0000** |

The cap term is an upper bound, not expected clean loss. Synthesis pays
maintenance from energy before mass needs a surplus.

###### (d) First-cycle Pass trace and boot state

| Cycle tick | Pass 1 | Pass 2 | Pass 3 / state after tick |
|---:|---|---|---|
| 1 | `absorb; absorb; getSize; for`, `LC=9` | none | capture own+front energy, then maintenance/decay [G3,G8] |
| 2 | synth spends base+additional 2E, creates 1M; `read`; append pays 1E and captures byte 0 | spend 1M; create inert child | new inert exempt [G4,G5,G7,G11] |
| 3–10 | `next; synthesize; read; appendAdj` | append one byte per tick | active writes reset inert grace |
| 11 | `next` sets `LC=0`; queue `boot` | child live at `IP=0`, age 0, newborn | birth; newborn exempt [G7,G11] |
| 12 | parent and child restart at first `absorb` | none | their first new-cycle footprints resolve later this tick |

The child begins with `Src=Dst=Flag=Msg=LC=0`, empty stack, and random
`Dir/ID` [G7]. If synthesis lacks free energy, it stalls before `read`, so the
copy cursor is preserved. It is not metabolically retry-safe: acquisition flags
reset at the next Pass 1 (`rust/src/pass1.rs:77-92`; `rust/src/model.rs:101-113`), and
an IP stalled at synth never returns to absorb. The initial capture therefore
must cover an uninterrupted loop. Because base payment consumes free E first,
the clean minimum is `3S-1=26E` plus actual maintenance; the final append base
alone may fall through to background [G2,G5]. Cadence evidence:
[manifest](../../rust/runs/seed-organisms/trace-synth9-seed-1.json),
[metrics](../../rust/runs/seed-organisms/outputs/trace-synth9-seed-1/metrics.jsonl);
the first boot birth is observed in row 11.

###### (e) Verified regime and sensitivity

| Parameter | Executed value | Bound / sensitivity |
|---|---:|---|
| `r_energy`, `r_mass` | 2.0, 0.0 | Conservative sufficient energy floor about 1.401; ambient mass is unnecessary. |
| `d_energy_log2`, `d_mass_log2` | 7, 7 | Energy refill uses `D=1/128`; mass decay is inactive on the exact path. |
| `t_cap` | 4.0 | Threshold 36; wider footprints can transiently exceed it. |
| `maintenance_rate_log2`, `maintenance_exponent` | 10, 1.0 | Included in energy; harsher maintenance threatens exact mass staging if energy empties. |
| `n_synth` | 1 | Each byte needs one additional *free* E, making prior absorb essential [G2,G5]. |
| `local_action_exponent` | .65 | `B=4` completes acquisition setup on tick 1. Synth-before-read preserves the cursor at other budgets but does not guarantee reharvest after a later stall. |
| `mutation_base_log2`, `mutation_background_log2` | 63, 63 | Both were made negligible for validation and observed mutation was zero; lower values were not tested. |

The own+front footprint overlaps a just-built child cell on later parent cycles,
and the fixed-direction topology repeatedly targets that child. Both effects
make the solitary sufficient budget a poor dense-state fidelity predictor.

###### (f) Executed manifest and empirical check

Executed [batch](../../rust/runs/seed-organisms/rung2-synth9-batch.json):
`3/2/0` fail, `4/3/0` pass, `7/6/0` pass, `8/7/0` pass, `2/1/0` fail,
so **3/5**. Mutation was zero, but final sizes were not stable: for example
seed 2 ended mean 419.5, max 849, three unique genomes.

##### Mixedforge

###### (a) Commented assembly

```text
; === Mixedforge: 10 instructions; B=floor(10^.65)=4 ===
absorb          ; cycle tick 1: latch own cell
absorb          ;               expand energy footprint to own+front
collect         ;               request own-cell ambient mass
getSize         ;               push 10; budget ends before `for`
for             ; tick 2: LC=10
  synthesize    ;         spend base+additional energy; make 1M
  read          ;         read byte 0 after mass exists
  appendAdj     ;         pay base; Pass 2 spends 1M
next            ; ticks 3-11: synthesize/read/append bytes 1..9
boot            ; tick 12: boot child; restart tick 13
```

The `collect` byte makes this a mass-buffer variant, not the simplest design at
its executed `r_mass=0`.

###### (b) Decimal byte array

```json
[81, 81, 83, 66, 48, 88, 85, 95, 49, 100]
```

###### (c) Per-cycle budget

At `S=10`, `T=12`, the remaining regime matches Frontforge and `F12=11.4976`.

| Pool | Per-cycle derivation | Expected quanta |
|---|---|---:|
| Captured energy, own+front | `2*2*F12` [G3,G9] | +45.9903 |
| Synth base+additional and append base | `10*(2+1)E` [G4,G5] | -30.0000 |
| Maintenance | `12*10/1024` [G8] | -0.1172 |
| Conservative cap-loss bound | `12*(1/128)*45.9903` | -4.3116 |
| **Energy balance** | sufficient lower bound | **+11.5615** |
| Synthesis / append | `+10M-10M` | 0.0000 |
| Collection at executed `rM=0` | `0*F12` | 0.0000 |
| **Mass balance** | maintenance paid from energy | **0.0000** |

At `r_mass>0`, collection contributes `rM*F12` of surplus margin. It cannot
replace synthesis: the unconditional `synthesize` stalls before `read` if its
additional energy is unavailable. At the executed point collect has no effect
and therefore cannot support a minimality claim [G3,G5].

###### (d) First-cycle Pass trace and boot state

| Cycle tick | Pass 1 | Pass 2 | Pass 3 / state after tick |
|---:|---|---|---|
| 1 | `absorb; absorb; collect; getSize`; `IP=for` | none | own+front E and own-cell M requests resolve; M yield is zero here [G3] |
| 2 | `for; synthesize; read`; append base pays and byte 0 is captured | spend synthesized 1M; create inert child | new inert exempt [G4,G5,G7,G11] |
| 3–11 | `next; synthesize; read; appendAdj` | append bytes 1–9 | active construction |
| 12 | `next` makes `LC=0`; queue `boot` | child live, `IP=0`, age 0, newborn | birth; newborn exempt [G7,G11] |
| 13 | parent and child restart acquisition | none | new-cycle harvest resolves |

The child otherwise starts with all registers including `Src/Dst` zero, empty
stack, and random `Dir/ID` [G7]. Cadence evidence:
[manifest](../../rust/runs/seed-organisms/trace-rung2-seed-1.json),
[metrics](../../rust/runs/seed-organisms/outputs/trace-rung2-seed-1/metrics.jsonl);
the first boot birth is observed in row 12.

###### (e) Verified regime and sensitivity

| Parameter | Executed value | Bound / sensitivity |
|---|---:|---|
| `r_energy`, `r_mass` | 2.0, 0.0 | Conservative sufficient E floor about 1.445; collect becomes useful only above zero mass radiation. |
| `d_energy_log2`, `d_mass_log2` | 7, 7 | Both pathways use `D=1/128` if enabled. |
| `t_cap` | 4.0 | Threshold 40; the two-cell capture can exceed it before spending. |
| `maintenance_rate_log2`, `maintenance_exponent` | 10, 1.0 | Included in energy; lower `k` increases fossilization risk. |
| `n_synth` | 1 | Requires captured free energy; raising it increases cost by 10E per unit. |
| `local_action_exponent` | .65 | `B=4` guarantees a harvest-only first tick; later energy stalls preserve `Src` but cannot reharvest. The captured buffer needs `3S-1=29E` plus maintenance for an uninterrupted loop. |
| `mutation_base_log2`, `mutation_background_log2` | 63, 63 | Both paths were made negligible for validation; observed mutation was zero. |

###### (f) Executed manifest and empirical check

Its empirical root was preloaded with `32E/0M`; constructed children were not.
Executed [batch](../../rust/runs/seed-organisms/rung2-synth10-batch.json):
`8/7/0`, `11/10/0`, `9/8/0`, `5/4/0` pass; `2/1/0` fails, hence **4/5**.
It is also genome-unstable: seed 1 ended mean size 429.4, max 872, five unique
genomes with zero mutations.

The specific long-run gap from solitary budgets is footprint/topology
interaction, not blind parameter tuning. Fixed-direction parents repeatedly
target their live child; parent and child can share the child's radiation cell.
When a payment failure opens the live child, occupied-target append becomes
legal [G2-G4]. Only `appendAdj` can increase size, so zero-mutation growth is
direct evidence of non-mutational appending, although census cannot distinguish
every open-live append from construction interference. Frontforge seed 1 later
converged to three size-4 live programs; census opcode totals match the four-byte
metabolic prefix `[absorb,absorb,getSize,for]`. That exact ordering is an
inference because the runner does not emit byte sequences per cell.

### 2.3 Rung 3 — Squarestep

#### (a) Commented assembly

Moving immediately after boot without changing direction targets the child and
must fail [G6]. The smallest repeatable mobile extension is therefore `cw; move`.
Squarestep is Radchain plus those two bytes.

```text
; === Squarestep: 9 instructions; B=floor(9^.55)=3 ===
collect         ; cycle tick 1: own-cell mass request
getSize         ;               push 9
for             ;               LC=9; harvest resolves after Pass 2
  read          ; tick 2: read byte 0, Src++
  appendAdj     ;         direct-bg base; Pass 2 spends 1M
next            ; ticks 3-10: copy bytes 1..8
boot            ; tick 11: activate child in the forward cell
cw              ; tick 12: turn perpendicular to the child
move            ;          direct-bg base; move parent if target is empty
                ; tick 13: next collection/copy cycle at the new cell
```

Four clockwise moves return the parent to its prior cell after `4*T=48` ticks,
creating a deterministic resource-rest orbit. Prefixes are deliberately useful:
7 bytes are Radchain, 8 bytes are a stationary turning chain, and 9 adds motion.

#### (b) Decimal byte array

```json
[83, 66, 48, 85, 95, 49, 100, 64, 99]
```

#### (c) Per-cycle energy and mass budget

Executed point: `S=9`, `T=12`, orbit revisit 48, `rE=.5`, `rM=.3`,
`D*=2^-7`, `M=2^-14`, `beta=1`, cap 36. A conservative established-orbit
budget uses refill of the revisited cell, not its larger initial stationary
reservoir: `F(48)=40.1564189` [G9].

| Pool | Per-cycle derivation | Expected quanta |
|---|---|---:|
| Background energy at revisited cell | `.5*F48` [G2,G9] | +20.0782 |
| Append bases | `9*1E` [G4] | -9.0000 |
| Move base | `1*1E` [G6] | -1.0000 |
| **Energy balance** | no absorbed/free E; maintenance falls through to mass | **+10.0782** |
| Collected mass at revisited cell | `.3*F48` [G3,G9] | +12.0469 |
| Append mass | `9*1M` [G4] | -9.0000 |
| Maintenance mass | `12*9/16384` [G8] | -0.0066 |
| **Mass balance before cap loss** | `12.0469-9-.0066` | **+3.0403** |

Cap 36 protects a full mass bill; only accumulated surplus is decay-eligible.
Movement is metabolic, not decorative: a stationary 12-tick refill supplies
only `.5*F12=5.7488E` and `.3*F12=3.4493M`, both below the 10E/9M bills.
Initial untouched means are still larger (`r/D=64E,38.4M`) [G9].

#### (d) First-cycle Pass 1/2/3 trace

| Cycle tick | Pass 1 | Pass 2 | Pass 3 / state after tick |
|---:|---|---|---|
| 1 | `collect; getSize; for`, `LC=9` | none | collect own mass; maintenance/decay [G3,G8] |
| 2 | `read` byte 83; append base 1E pays background and operand is captured | spend 1M; create inert `[83]` [G4,G7] | reset inert timer; new inert is exempt [G11] |
| 3–10 | `next; read`; pay/queue append for bytes 1..8 | one append succeeds per tick | active construction |
| 11 | `next` sets `LC=0`; queue `boot` | child live, `IP=0`, age 0, newborn [G7] | boot birth; newborn exemptions apply [G11] |
| 12 | parent executes `cw`; move base 1E pays background; child executes its first `collect; getSize; for` | parent and its free pools move perpendicular if empty; child stays [G6] | child's first collection resolves; background remains at origin/destination |
| 13 | successful parent starts collection at its new site | none | next parent cycle |

The child starts with the shared zeroed register/stack state and random retained
`Dir/ID` [G7]. Cadence-1 evidence shows inert sizes 1–9 in rows 2–10 and first
boot in row 11: [manifest](../../rust/runs/seed-organisms/trace-square9-seed-1.json),
[metrics](../../rust/runs/seed-organisms/outputs/trace-square9-seed-1/metrics.jsonl).

#### (e) Verified viable regime, analytic floor, and sensitivity

| Parameter | Executed value | Bound / sensitivity |
|---|---:|---|
| `r_energy` | .50 | Established-orbit mean floor `10/F48=.2490`; wide margin covers timing and occupied moves. |
| `r_mass` | .30 | Mean floor including maintenance `.2243`; default `.05` is insufficient for this nine-byte copy. |
| `d_energy_log2`, `d_mass_log2` | 7, 7 | Supplies stationary initialization and 48-tick refill. Null decay starts backgrounds at zero, so this zero-preload organism lacks startup resources. |
| `t_cap` | 4.0 | Threshold 36 protects the complete mass bill. |
| `maintenance_rate_log2` | 14 | Included as mass fallback; lower `k` increases tail erosion after failed moves. |
| `maintenance_exponent` | 1.0 | Higher beta penalizes the large composite genomes observed in density. |
| `n_synth` | 1 | Unused. |
| `local_action_exponent` | .55 | Gives `B=3`. With zero initial mass, exact first byte needs `B<=4`; `B>=5` reaches append before Pass-3 collect. |
| `mutation_base_log2`, `mutation_background_log2` | 63, 63 | Ten direct-background quanta per clean cycle feed the stressed trigger; lower settings remain untested. |

Move can fail on occupancy or exclusive conflict. On failure the parent remains
in place with its clockwise direction and enters another copy attempt next
cycle; dense behavior is therefore not described by the solitary orbit budget.

#### (f) Executed manifest and empirical check

```sh
cd rust
cargo run --release --bin proteus-batch -- \
  --manifest runs/seed-organisms/rung3-square9-batch.json
```

Exact [batch](../../rust/runs/seed-organisms/rung3-square9-batch.json): all five
seeds passed, with the exact values in the horizon matrix, so **5/5 viable**.
Seed 1 went from `1/0/0` at tick 0 to population 2,883 / births 2,770 / deaths 0
at tick 500, then 4,096 / 4,131 / 36 at tick 20,000.

It is a colonizer, not a faithful dense-state copier. At tick 500 seed 1 already
had 112 inert bodies, 1,477 unique genomes, mean live size 15.67, max 106; at
the horizon it had zero mutations, 3,893 unique genomes, mean size 803.98, max
4,495. Since only append can grow code, this proves non-mutational construction
mixing. The likely mechanism is the combination of always-open inert bodies,
multiple parents competing at different copy phases, unconditional boot after
failed appends, and appends into live cells opened by payment failure
[G2,G4,G7]. Current census cannot assign a fraction to each cause; that split is
marked as inference. Engine deaths are maintenance removal of the final code
byte [G8], but resource starvation preceding each recorded death is likewise an
inference rather than an action-counter observation.

#### Preserved captured-energy analogue — Wildstep

##### (a) Commented assembly

```text
; === Wildstep: 11 instructions; B=floor(11^.5)=3 ===
absorb          ; cycle tick 1: request own-cell energy capture
collect         ;               request own-cell mass collection
getSize         ;               push 11; budget ends before `for`
for             ; tick 2: LC=11
  read          ;         read byte 0
  appendAdj     ;         pay 1E; Pass 2 spends 1M
next            ; ticks 3-12: next/read/append bytes 1..10
boot            ; tick 13: boot the forward child
rand            ; tick 14: push a random value
setDir          ;          choose a new direction from it
move            ;          pay 1E; Pass 2 moves if that cell is empty
                ; tick 15: acquire at the new site after a successful move
```

##### (b) Decimal byte array

```json
[81, 83, 66, 48, 85, 95, 49, 100, 20, 73, 99]
```

##### (c) Per-cycle budget

At `S=11`, `T=14`, `rE=.25`, `rM=.2`, `D*=2^-7`, `M=2^-14`, and
cap 44, this sparse-world budget assumes each successful move leads to an
effectively untouched cell for the next cycle. Its stationary means are
`rE/D=32E` and `rM/D=25.6M` [G9].

| Pool | Per-cycle derivation | Expected quanta |
|---|---|---:|
| Captured energy at fresh source | `.25/(1/128)` [G3,G9] | +32.0000 |
| Eleven append bases + move | `11E+1E` [G4,G6] | -12.0000 |
| Maintenance | `14*11/16384` [G8] | -0.0094 |
| Conservative cap-loss bound | `14*(1/128)*32` | -3.5000 |
| **Energy balance** | sparse-world sufficient bound | **+16.4906** |
| Collected mass at fresh source | `.2/(1/128)` | +25.6000 |
| Append mass | `11*1M` [G4] | -11.0000 |
| Conservative cap-loss bound | `14*(1/128)*25.6` | -2.8000 |
| **Mass balance** | maintenance paid from energy | **+11.8000** |

An empty cell is not necessarily untouched: revisits or other organisms may
have harvested it. A 14-tick refill gives only `.25*F14=3.3277E` and
`.2*F14=2.6622M`, both far below the bills, so successful sparse movement is a
metabolic requirement for a parent's repeated cycles. Its newly booted child
can still start from the untouched background in its own cell.

##### (d) First-cycle Pass trace and boot state

| Cycle tick | Pass 1 | Pass 2 | Pass 3 / state after tick |
|---:|---|---|---|
| 1 | `absorb; collect; getSize`; `IP=for` | none | acquire own-cell E/M, then maintenance/decay [G3,G8] |
| 2 | `for; read`; append pays 1E and captures byte 0 | spend 1M; create inert child | new inert exempt [G4,G7,G11] |
| 3–12 | `next; read; appendAdj` | append bytes 1–10 | active construction |
| 13 | `next` sets `LC=0`; queue `boot` | child live, `IP=0`, age 0, newborn | birth; newborn exempt [G7,G11] |
| 14 | parent `rand; setDir`; move pays 1E; child begins `absorb; collect; getSize` | parent and free pools move if chosen target was empty [G6] | child's first own-cell acquisition resolves; backgrounds do not move |
| 15 | a successfully moved parent restarts acquisition | none | next parent cycle |

The child begins with zero `Src/Dst/Flag/Msg/LC`, empty stack, and its random
construction-time `Dir/ID`; boot does not reroll those [G7]. Cadence evidence:
[manifest](../../rust/runs/seed-organisms/trace-rung3-seed-1.json),
[metrics](../../rust/runs/seed-organisms/outputs/trace-rung3-seed-1/metrics.jsonl);
the first boot birth is observed in row 13.

##### (e) Verified regime and sensitivity

| Parameter | Executed value | Bound / sensitivity |
|---|---:|---|
| `r_energy`, `r_mass` | .25, .20 | Clean fresh-cell floors are `(12+.0094)/128=.09382` and `11/128=.08594`; the table's conservative loss bound gives sufficient `.10535/.09649`. Revisits need much higher rates. |
| `d_energy_log2`, `d_mass_log2` | 7, 7 | These create the fresh stationary reservoirs. Null decay initializes background at zero and invalidates this startup niche. |
| `t_cap` | 4.0 | Threshold 44 exceeds either mean one-cell capture; clean-path cap loss is normally zero. |
| `maintenance_rate_log2`, `maintenance_exponent` | 14, 1.0 | Small in the clean ledger; lower `k` accelerates suffix erosion after movement stalls. |
| `n_synth` | 1 | Unused. |
| `local_action_exponent` | .50 | `B=3` guarantees Pass-3 acquisition before byte-zero append; zero-preload children retain that ordering through `B=5`. |
| `mutation_base_log2`, `mutation_background_log2` | 63, 63 | Mutation was made negligible and observed at zero; captured append energy usually uses the baseline path. |

A random direction chooses the just-built child one quarter of the time in
sparse space, making move fail; other occupancy and movement conflicts worsen
with density. The next copy attempt then uses that retained direction.

##### (f) Executed manifest and empirical check

Wildstep also changes Squarestep's direction policy, so it is not a controlled
one-opcode energy-mode pair. Its empirical root was preloaded with `16E/16M`;
constructed children were not. Executed
[batch](../../rust/runs/seed-organisms/rung3-forager11-batch.json): all
five passed (**5/5**) with horizon values in the summary matrix. Every run had
zero mutation yet roughly 3,900 unique genomes and mean size 607–625, so the
same dense-state fidelity warning applies. Wildstep's prefix architecture is
less clean than Squarestep's: its length-9 prefix ends in `rand` and leaks stack;
length 10 is a random-turn stationary copier; only length 11 moves.

No optional fourth rung is claimed. Communication would add a genuinely new
niche, but an emit/listen cooperator needs a separately budgeted partner
geometry and packet trace before it meets this report's verification standard.

### 2.4 Instruction ordering, prefix fossils, and rotation

The SPEC itself lists “Prefix-writing bias: failed replication creates whatever
prefix was already copied” under Known Limitations. Current ordering amplifies
that bias in precise ways.

| Failure point | Exact state transition | Consequence |
|---|---|---|
| Synth base/additional payment before `read` | failure leaves IP at synth and `Src` unchanged [G2,G5] | copy-cursor safe, but `n>0` forges can deadlock because acquisition flags reset before the retry |
| Append base payment | `read` has advanced `Src`, but base failure occurs before operand capture; IP remains append and byte remains stacked (`rust/src/pass1.rs:575-584,681-751`) | retry-safe for that byte, though the inert target ages while stalled |
| Append protection/conflict/cap/mass failure in Pass 2 | operand was popped and IP advanced in Pass 1; local `Src` is already incremented [G4] | byte is silently skipped |
| Following `next` | matched `next` clears `Flag` (`rust/src/pass1.rs:374-386`) | current loops forget Pass-2 failure and still reach `boot` |
| Abandoned inert maintenance | only successful writes reset grace; maintenance truncates from the tail (`rust/src/pass3.rs:632-691`) | persisted bodies converge toward prefixes |
| Boot | validates only inert state, not size/content [G7] | short, overgrown, or composite bodies can become live |

Resource-first, boot-last ordering is therefore fail-closed in one sense: tail
erosion removes reproduction/movement first and leaves non-birthing metabolic
fossils. Frontforge seed 1's size-4 endpoint is consistent with exactly this.
Squarestep also makes prefix bias constructive: loss of its mobility suffix
yields the viable Radchain core. The tradeoff is strong evolutionary pressure
toward the simpler prefix.

Raw `Src` is another ordering channel. Omitting `push 0; setSrc` is minimal while
size is invariant. After growth or tail erosion, old `Src mod new_size` can make
a survivor copy a cyclic rotation rather than byte zero first.

#### Cyclic-rotation screen

All seven rotations of Radchain were run for 20,000 ticks on the same world seed
and regime. This is an exploratory one-seed screen, not a viability ensemble.

| Rotation | Byte-zero / initial opcode | Population/births/deaths | Unique genomes | Result |
|---:|---|---:|---:|---|
| r0 | `collect` | 7/6/0 | 1 | canonical |
| r1 | `getSize` | 2/1/0 | 1 | stalled chain |
| r2 | `for` | 2/1/0 | 2 | stalled; non-mutational divergence |
| r3 | `read` | 2/1/0 | 2 | stalled; non-mutational divergence |
| r4 | `appendAdj` | 2/1/0 | 1 | stalled chain |
| r5 | `next` | 6/5/0 | 1 | productive |
| r6 | `boot` | 6/5/0 | 1 | productive |

[Rotation batch](../../rust/runs/seed-organisms/rotation-min7-batch.json). Seeds
and children always start `IP=0`, `Src=0`, `LC=0`, empty-stack [G7], so rotation
changes both the first execution transient and the order of partial writes. It
also changes event ticks and therefore later cell-RNG draws, including child
direction; the population differences are not attributable to entry phase
alone. The two zero-mutation, two-genome outcomes nevertheless directly show
that rotations are not safe to deduplicate as equivalent circular strings.

No permutation makes append conflict atomic. A candidate retry-preserving
extension, not yet run, duplicates the byte and captures `Flag` before any
later flag-clearing stack operation:

```text
; Retrychain hypothesis (13 bytes)
collect; getSize; for
  read
retry_dup: dup
  appendAdj
  getFlag; push -5; swap; jmpNZ retry_dup
  drop
next
boot
```

```json
[83, 66, 48, 85, 16, 95, 68, 11, 18, 51, 17, 49, 100]
```

Offset -5 returns from `jmpNZ` to `dup`, which recreates the expendable top copy
while retaining one byte below it. This preserves the parent's byte across a
Pass-2 loss but cannot undo a competing writer's change to the target. It can
also livelock on a permanently occupied/capped target and remains a hypothesis.
A target-size gate plus turn/abandon path is needed for a complete design.

## 3. Spec/code discrepancies found

| Discrepancy | Evidence and consequence |
|---|---|
| SPEC Seed Replicator period is 13, not 14 | The listing/reference gives `T=14` (`docs/SPEC.md:530-540,640-669`), but normative throughput and Rust let `nop` consume one local action and continue to later locals in the same tick (`rust/src/pass1.rs:140-185,523-530`). Boots are 13 ticks apart. This is an internal SPEC example off-by-one, not a code violation of the throughput rule. |
| Maintenance eligibility prose conflicts internally | Pass 0 says only live-at-start programs pay (`docs/SPEC.md:252`), while lifecycle and detailed Pass-3 step 9 say abandoned inert programs pay (`docs/SPEC.md:75,94,323-325`). Rust follows the detailed rule (`rust/src/pass3.rs:648-670`). SPEC-COMPANION has the same broad/detail tension. |
| No direct decay-driven code destruction | SPEC deletion notes mention “maintenance or decay-driven destruction” (`docs/SPEC.md:462`). Rust decay removes only free E/M; only maintenance truncates code (`rust/src/pass3.rs:461-486,694-714`). Decay can contribute only indirectly. |
| Three uniform choices have tiny modulo bias | SPEC requires size-weighted conflict ties, uniform packet choice, and uniform mutation location (`docs/SPEC.md:304,311,483`). Rust uses `%N` (`rust/src/pass2.rs:342-347`; `rust/src/pass3.rs:252-254,579`), exact only when `N` divides `2^64`. |
| Main newborn wording is broader than constructed implementation | SPEC says a program created during a tick is newborn (`docs/SPEC.md:100`); Companion §3.2 applies newborn at boot. Rust append creation does not set newborn, but `existed_at_tick_start=false` independently exempts it; boot sets newborn (`rust/src/pass2.rs:468-478,502-517`). |
| Emit conservation needs a success qualifier | Companion §10.1 says the emit base becomes one packet. Rust pays base before operand capture; empty-stack emit spends energy but emits no packet (`rust/src/pass1.rs:560-573,681-751`). Successful emit conserves as stated. |

No SPEC/Rust disagreement was found for successful ordinary `absorb`, `collect`,
`synthesize`, `appendAdj`, `move`, constructed defaults, or boot used here.

## 4. Parameter recommendations

These are engineering ranges around executed points. Only exact manifest points
were run; analytic floors and unexecuted intervals are explicitly hypotheses.

| Scope | Recommended range | Why each bound exists |
|---|---|---|
| Radchain | at `d*=7`, `rE=1.0–2.0`, `rM=1.0–1.5`; tested `2/1.25` | Mean floors `.802/.809`; lower-tail and payment-timing margin motivates the lower recommendations. Higher rates mainly accumulate/decay surplus. |
| Freesynth | `rE=1.8–2.2`, `rM=0`, `n_synth=0`, `maintenance_rate_log2=null`; tested `2` | Mean energy floor `1.605`. `n>0` is impossible without captured/transferred free E; finite maintenance needs surplus synthesis/code. |
| Squarestep | at `d*=7`, `rE=.35–.60`, `rM=.27–.40`; tested `.5/.3` | Orbit floors `.249/.224`; margin covers Poisson tails and delayed/failed moves. Below the 12-tick stationary bills, movement remains the niche. |
| Wildstep | at `d*=7`, sparse-world `rE=.15–.30`, `rM=.13–.25`; tested `.25/.2` | Conservative fresh-cell floors `.105/.0965`; margin covers random failed moves, while dense revisits need a different regime. |
| Captured chain/forge | Sunchain `rE,rM>=1.1`; Frontforge `rE>=1.6,rM=0`; Mixedforge `rE>=1.7,rM>=0` at tested decay | Conservative analytic floors are about `.908/.899`, `1.401`, and `1.445`; stochastic timing and forge footprint sharing motivate the higher ranges. |
| `d_energy_log2`, `d_mass_log2` | `7–9` for this library generation | Larger `k` retains more background. Null is a different startup regime because fresh backgrounds initialize at zero. |
| `t_cap` | `>=2` for Radchain/Squarestep, `>=4` for captured forges; tested 4 | Protects at least one working bill; direct background itself has no threshold. Freesynth stores at most 1M cleanly. |
| `maintenance_rate_log2` | `>=10` resource-buffered chains; `>=14` mobile; `null` Freesynth-7 | Bounds reflect expected size-scaled maintenance and observed dense-state erosion. Freesynth has no mass surplus. |
| `maintenance_exponent` | `0–1`, tested 1 | For `S>1`, lower beta reduces cost; beta above 1 makes large/composite programs disproportionately fragile. |
| `local_action_exponent` | select by intended `B`, not a global range | Radchain/Squarestep need `B<=4`, and Sunchain/Wildstep `B<=5`, to harvest before byte-zero append from zero preload. Freesynth traces at `B=4`. Frontforge protects `Src` on synth failure but needs a `3S-1` free-E loop buffer to avoid deadlock. Lower budgets can remain viable with re-derived periods. |
| mutation exponents | use `>=32` for engineering validation; only 63 tested | Separates construction from evolution. Direct-background organisms consume 7/14/10 stressed quanta per clean cycle, so restore ecological values only deliberately. |

All executed manifests also use `inert_grace_ticks=10` and
`p_spawn_log2=null`. Successful construction writes at least once per tick, so
the grace timer resets; a stalled abandoned prefix eventually pays maintenance
[G8].

## 5. What to try next

1. Make rotations first-class in the future seed library: store a canonical
   family, exact byte order, rotation/entry metadata, parameter profile, status,
   and evidence manifest. Never necklace-deduplicate rotations. A future
   `initial_ip` field could separate execution entry from serialization/prefix
   order; current bootstrap and boot couple both at zero.
2. Build controlled energy-path twins while preserving all variants: test
   **Sunstep** (`absorb` + Squarestep, retaining `cw`) against Squarestep, and
   **Sunsynth**, a captured `n_synth=0` Freesynth twin. Hold instruction order,
   direction policy, `n_synth`, and stochastic seed fixed so only acquisition
   and the resulting size/timing differ. Current captured forms are functional
   analogues, not those controlled A/Bs.
3. Run Retrychain above, then add a size check before boot and an explicit
   turn/abandon path. Score genome retention and inert-prefix load alongside
   population.
4. Add an explicit `push 0; setSrc` robustness variant. Test size changes and
   cyclic-rotation inheritance rather than only exact parents.
5. Add empty-target scanning (`senseSize`, branch, turn) to the mobile and forge
   families. Pass-0 sensing cannot prevent simultaneous contention, so retain
   conflict tests.
6. Add action counters or cell inspection to distinguish failed moves,
   open-live appends, inert multi-parent splicing, and boot conflicts. Current
   census proves non-mutational divergence but not its exact causal shares.
7. Only then build an emit/listen pair with explicit partner geometry, packet
   propagation/collision trace, listener-opening risk, and two reproduction
   budgets as a genuinely new fourth rung.

All manifests, batches, cadence traces, rotation screen, and outputs are under
`rust/runs/seed-organisms/`. Work for this analysis wrote only there and to this
new document; engine/spec/tests and Git history were not modified.
