# Proteus Spec Changelog

## Recent Changes

### 2026-08-21 (v0.4.0)

- Replaced the floating `d_energy`, `d_mass`, `maintenance_rate`, and `p_spawn` config fields with optional integer exponent fields ending in `_log2`. `Some(k)` means probability `2^-k` for `k` in `0..=63`; JSON `null` means never. Absent decay and maintenance fields default to 7, while absent spawn defaults to null.
  - Why: the config now carries the exact representation consumed by the samplers, removes invalid dyadic floats and exponent derivation, and makes “never” explicit.
- Changed background-stressed mutation from `min(x / 2^k, 1)` to one mutation if any of `x` independent `Bernoulli(2^-k)` triggers fires, with probability `1 - (1 - 2^-k)^x`.
  - Why: independent per-quantum risk removes the hard saturation cliff while keeping the existing single-mutation-per-tick model.
- Bumped the runner manifest schema `0.1.0` -> `0.2.0` (the exponent-field rename changes the canonical manifest serialization, which the runner spec ties to its schema version). Pre-0.4.0 manifests and run trees are incompatible: manifests are rejected by validation and completed run trees cannot resume. See the migration note in `RUNNER-SPEC.md`.
  - Why: the field rename already invalidated old manifests; the schema bump labels that break honestly instead of leaving the version string stale.

### 2026-08-21 (v0.3.0)

- Constrained `D_energy`, `D_mass`, `M`, and `P_spawn` to exactly 0, 1, or `2^-k` for integer `k` in `1..=63`, while preserving their existing Bernoulli/binomial semantics.
  - Why: exact power-of-two probabilities admit exact bit-level sampling and remove floating-point distribution construction from the per-cell hot path.
- Changed the suggested and implementation-default `D_energy` and `D_mass` values from 0.01 to `2^-7 = 0.0078125`. At `R_energy = 0.25`, this raises stationary mean background radiation from 25 to 32.
  - Why: 0.01 is not dyadic; `2^-7` is the approved nearby value and its ecology shift is accepted.
- Restricted `mutation_base_log2` and `mutation_background_log2` to `0..=63`. Values above 63, which were previously accepted and behaved as extremely rare or effectively never in the floating-point sampler, are now configuration errors.
  - Why: the exact mutation samplers operate on one 64-bit draw, and rejecting unsupported exponents makes the boundary explicit instead of silently changing their meaning.

### 2026-03-23 (v0.2.1)

- Reinterpreted `R_energy` and `R_mass` as Poisson arrival means per cell per tick rather than Bernoulli probabilities capped at one arriving unit. `D_energy` and `D_mass` remain per-quantum decay probabilities with binomial thinning semantics.
  - Why: this lets ambient resource rates exceed 1 without inventing ad hoc clamping or multi-draw hacks, while preserving the simple discrete-time decay model.
- Defined fresh simulation initialization for ambient background pools using the stationary law of the arrival/decay process: each cell starts from `Poisson(R / D)` when `D > 0`, and from 0 when `D = 0` because no finite steady state exists.
  - Why: previously, the initial seed programs were starving. Fresh simulations now begin at ambient equilibrium instead of an artificially empty world, without breaking the mathematically correct no-steady-state case. 

### 2026-03-15

- Clarified that local additional-cost failure (e.g. `synthesize` lacking energy for `N_synth`) has identical consequences to base-cost failure: `Flag = 1`, `IP` does not advance, cell is marked open, remaining local action budget is forfeit. The already-paid base cost is not refunded.
  - Why: the spec's "if any required payment fails, halt and mark the cell open" already implied this, but did not explicitly restate `Flag` and `IP` behavior for the additional-cost case, creating ambiguity for implementers.

### 2026-03-14 (v0.2.0)

- Replaced the old immediate-vs-1-tick execution split with a local/nonlocal model where each tick gives a program `max(1, floor(size^alpha))` local actions, local instructions consume one local action each, and each program may queue at most one nonlocal action.
  - Why: this is the core anti-minimal-replicator change. Larger organisms can now buy more internal computation and metabolism without breaking the one-cell-per-tick interaction limit.
- Separated the scaling of capability and upkeep by introducing explicit `alpha` and `beta` exponents for local action budget and maintenance burden.
  - Why: this makes complexity a tunable ecological question instead of baking in one fixed size tradeoff.
- Promoted adjacent sensing into the local phase and defined it against a Pass-0 snapshot. Empty-cell sensing is now explicitly disambiguated: `senseSize`, `senseE`, and `senseM` treat emptiness as a valid 0-valued read, while `senseID` uses `Flag` to signal ambiguity with `ID = 0`.
  - Why: organisms can do more local perception each tick without creating iteration-order artifacts, and implementers now have precise empty-neighbor semantics.
- Reworked Pass 2 so benign additive/read-only actions can all succeed, while hostile structure-changing and occupancy-changing actions use conflict resolution.
  - Why: this removes an artificial bottleneck where harmless reads or transfers would block each other just for sharing a target cell, while still preserving exclusivity for genuinely incompatible actions.
- Standardized Pass-2 side effects so base costs are paid and operand capture is attempted at the first nonlocal boundary in Pass 1; successful capture creates the queued action, failed capture still advances `IP` and ends Pass 1 for that program, and cursor increments plus additional costs happen only on successful execution.
  - Why: failure behavior is cleaner, easier to test, and avoids weird partial-success edge cases.
- Added an explicit three-way `Flag` outcome: clear, set, or neutral. `listen` is now also explicitly specified as Pass-1 `Flag`-neutral, with any later packet capture setting `Flag` only in Pass 3.
  - Why: harmless soft no-ops no longer overwrite useful internal signal state, and deferred instructions now have unambiguous Pass-1 semantics.
- Split `absorb` into a cleaner metabolism instruction and a separate `listen` instruction for directed-radiation capture and messaging. The first `absorb` call in a tick now explicitly establishes a footprint of size 1 and locks `absorb_dir`; later calls only expand that footprint up to the cap.
  - Why: energy harvesting and communication are no longer entangled in one overloaded opcode, and the core harvesting behavior no longer depends on an implied first-call convention.
- Fixed directed-radiation accounting so each packet carries exactly one unit of energy plus an independent 16-bit message, and simultaneous `listen` captures choose a packet uniformly for `Msg`/`Dir`.
  - Why: signaling no longer mints or destroys energy through message payloads, and simultaneous arrivals have deterministic semantics modulo explicit randomness.
- Introduced background mass as a first-class ambient pool parallel to background radiation, plus `collect` as the active way to crystallize it into free mass.
  - Why: the mass economy is now more symmetric with the energy economy, and foraging mass becomes an explicit behavioral choice.
- Narrowed background-radiation emergency payment so it can cover only base instruction costs, not surcharges such as the extra energy required by `synthesize`.
  - Why: ambient energy remains a last-ditch execution aid rather than a general substitute for stored working energy.
- Removed direct siphoning instructions `takeE` and `takeM`, leaving predation primarily destructive rather than directly extractive.
  - Why: ecological interaction is harsher and more legible; attackers must usually damage or dismantle neighbors instead of vacuuming out resources through open-state exploits.
- Clarified intentional asymmetries in world interaction: `giveE` has base cost 0 while `giveM` has base cost 1, and local `del` holds `Dst` fixed while successful remote `delAdj` advances it.
  - Why: these asymmetries are part of the machine design rather than accidental inconsistencies, and making them explicit reduces implementer second-guessing.
- Tightened program lifecycle semantics around live vs inert states, active construction grace periods, abandonment, booting, and uniform newborn exemptions. A booted program is newborn even when revived from an abandoned inert state, so it skips maintenance on its boot tick.
  - Why: partially built organisms now have a much clearer ontological status and a more principled path from offspring fragment to active organism.
- Moved spontaneous creation to the end of the tick, with newborns entering the next tick at age 0, and added one-time crystallization of ambient background resources at birth.
  - Why: this removes birth-tick ambiguities around maintenance, execution, mutation, and aging.
- Clarified protection/open-state rules so inert programs are always open, and openness comes from explicit states or events such as `nop`, `listen`, or failed energy payment rather than from quirks of the old immediate-execution cap.
  - Why: vulnerability remains an important ecological tradeoff, but it is now tied to intelligible mechanics instead of scheduler artifacts.
- Tightened code-editing semantics: deletion cannot remove the last instruction, `appendAdj` fails cleanly at the size cap, and deletion is interpreted as “continue with the next surviving instruction.”
  - Why: self-modification is still powerful, but the machine now has much fewer ambiguous edge cases around cursor motion, liveness, and execution continuity.
- Clarified maintenance destruction semantics and other operational details, including decay-then-arrival ordering, end-of-tick spontaneous birth, and the distinction between recycling by explicit deletion versus permanent removal by maintenance.
  - Why: the spec is much closer to implementation-ready and less likely to produce divergent simulators from innocent interpretation differences.
- Corrected the world-interaction opcode count to match the actual encoding range and total-opcode summary.
  - Why: this removes an editorial inconsistency that could otherwise make the instruction inventory look underspecified.
- Expanded the seed/budget analysis and overall operational framing so the reference organism and economy are defined against the new execution model rather than inherited from the old one.
  - Why: the spec now better matches the actual machine it describes, which should make early experiments easier to interpret.
