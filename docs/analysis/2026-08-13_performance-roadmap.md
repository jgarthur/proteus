# Post-RAYON-PERFORMANCE roadmap

Date: 2026-08-13

Records the agreed performance strategy after the clone-free Pass 2 and fused
Rayon Pass 3 work (`2026-08-12_rayon-optimization-results.md`). Tiers 0 and 1
are specified in implementable detail in `2026-08-13_dyadic-sampler-plan.md`;
this note records the whole picture, the Tier 2 design directions, and the
expected ceilings so the strategy survives the session. Analysis material,
not authority; measurements are one Apple M2 (4P+4E cores).

## Where the time goes now

Dense web scenario (256x256 at 80% occupancy), Rayon 4 threads, 9.19 ms/tick:
ambient 46%, Pass 1 29%, tail 14%; Pass 2, packets, mutation, prepare, and
newborn-clear are each ≤ 8% combined. Serial: 21.0 ms with the same ordering.
Empty/sparse grids are almost pure ambient+tail, i.e. almost pure sampling.
The line profile attributes ambient/tail time to per-draw
`rand_distr` binomial/Poisson construction (`powf`/`exp`/`log_gamma`) and
Pass 1 time to opcode dispatch, not to Rayon launch overhead.

## Tier 0 — stream-preserving micro-wins (~10%, land anytime)

No digest changes, no version bump; each is provable value-preserving:

- `powf(x, 1.0) == x` exactly, so guard `maintenance_exponent == 1.0` in
  `resolve_maintenance_cell` and `local_action_exponent == 1.0` in
  `local_action_budget` (details in the sampler plan, Phase 0).
- Scratch reuse deferred from the Rayon plan: Pass 1's per-tick
  `Vec<Pass1CellOutput>` and Pass 2's candidate/transfer/commit vectors can
  live in `Simulation`-owned scratch without public API changes. Only worth
  keeping if a profile shows allocation time; the 2026-08-12 profile did not.

## Tier 1 — exact dyadic samplers (~2x whole-tick dense, more on sparse)

Fully specified in `2026-08-13_dyadic-sampler-plan.md`. Summary: every hot
probability is (after an approved spec adjustment to `d_energy`/`d_mass`
defaults) exactly `2^-k`, so Bernoulli and binomial draws become exact
integer bit operations (halving-cascade binomial: expected < 2n bits, no
transcendentals), and Poisson moves to inversion with `exp(-λ)` hoisted out
of the per-cell path. RNG draw streams change: crate/spec version bump,
golden literals regenerate, structural invariant tests must pass unmodified.
Expected: ambient sampling ~126 → ~20-30 ns/cell serial; dense web Rayon-4
~9.2 → ~5-6 ms/tick; empty grids roughly halve.

Considered and rejected: switching arrivals from Poisson to negative
binomial for samplability — inversion already makes any small-mean count
distribution O(1 + mean), so distribution choice stays a modeling decision.

## Tier 2 — data layout (est. 1.5-2.5x more on the grid-sweep phases)

The remaining ambient/tail/prepare cost after Tier 1 is memory traffic:
`Cell` is ~140 bytes holding an inline `Option<Program>` with two heap `Vec`s
(code, stack), while the sweeps touch ~20 bytes per cell plus a pointer-chase
for per-tick flags. Two sub-steps, in order of increasing invasiveness:

**2a. SoA hot-field split (no spec change, digest-identical).** Move the
fields the full-grid sweeps actually touch — `bg_radiation`, `bg_mass`,
`free_energy`, `free_mass`, and a small per-tick flag bitmask (`did_collect`,
liveness/occupancy) — into parallel arrays owned by `Grid`, leaving programs
where they are. Ambient/tail/prepare then stream dense arrays (Rayon zips
over slices) instead of striding cells. Mechanically large (Grid accessors,
observe/web projections, tests) but behavior-identical, so the parity script
proves it. Do this only after a post-Tier-1 profile confirms the sweeps are
memory-bound.

**2b. Inline program storage behind spec caps (spec change).** Cap program
code and stack sizes small enough to store both as fixed-capacity inline
arrays (no heap per program): eliminates allocator traffic on
births/deaths/moves and the code pointer-chase in Pass 1, and improves
multi-thread scaling by removing latency-bound loads. Current spec caps are
2^15 − 1 for both (SPEC.md program size and stack sections) — far beyond
anything the ecology uses, but caps must be chosen from data, not guessed:
**prerequisite is a program-size and stack-depth histogram metric** (observe
layer or `tick_bench` instrumentation) from a long dense web run. Note the
asymmetry: capping the stack *alone* buys almost nothing today (stacks are
lazily allocated `Vec`s that most programs never grow); the payoff exists
only when code and stack are both capped and moved inline. Grid memory
becomes cells × max-program footprint (e.g., 64 B code + 32×2 B stack ≈
0.25 KB/cell ≈ 16 MB at 256x256 — acceptable; revisit if caps grow).

## Pass 1 dispatch (open investigation)

~12 ns per executed instruction serial. No design settled; candidate angles:
flatter decode (byte → handler table), fewer repeated `program(cell)`
re-borrows inside `execute_local_instruction`, reusing the per-tick output
vec (Tier 0 overlap). Bounded expectations (1.3-1.7x on Pass 1); the crate
forbids `unsafe`, so bounds-check elimination must come from iterator shapes.
Tier 2b helps here too (inline code removes a dereference per fetch).

## Scaling ceiling and ordering

| Step | Dense web Rayon-4 (ms/tick, est.) |
| --- | ---: |
| today | 9.2 |
| + Tier 0 | ~8.5 |
| + Tier 1 | ~5-6 |
| + Tier 2 + Pass 1 work | ~2.5-3.5 |

Estimates, not commitments; re-profile after each step (`tick_bench`, quiet
machine, serial + Rayon 2/4/8). More than 4 threads regresses on this M2
(4 P-cores); Tier 2's locality work is what would let wider machines scale
past that. Beyond Tier 2 lies SIMD-batched sampling or GPU offload — poor
effort-to-reward at current sizes, revisit only if grids grow well past
256x256.

Order: Tier 0 → Tier 1 (with its migration steps) → histogram metric →
post-Tier-1 profile → choose 2a and/or 2b → Pass 1 dispatch investigation
whenever a profile shows it at the top.
