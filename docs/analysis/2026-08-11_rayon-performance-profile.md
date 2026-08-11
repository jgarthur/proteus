# Rayon performance profile

Date: 2026-08-11

This is a point-in-time, non-authoritative performance note for the Rayon branch. It records phase-level profiling and temporary A/B experiments intended to identify remaining performance headroom. No profiling hooks or experimental implementations were retained in the source tree.

## Summary

The modest frontend-fixture speedup is not caused by the expensive parallel work failing to scale. Background radiation and background mass scale well. The current ceiling comes from two structural costs:

1. Pass 2 is serial and performs three deep grid clones per tick, including program-owned vectors.
2. Pass 3 launches many separate Rayon traversals whose per-cell bodies are too small to amortize scheduling and synchronization on a 64×64 grid.

The highest-value implementation order is:

1. Remove the redundant exclusive-resolution clone and skip exclusive setup when there are no exclusive actions.
2. Replace the remaining full pre-Pass-2 clone with compact staged action data or a purpose-built snapshot, and reuse Pass 2 scratch buffers.
3. Fuse compatible cell-local Pass 3 stages into Rayon-specific traversals while retaining the existing serial loop shape.
4. Re-profile before considering parallel packet or absorb resolution.

Temporary A/B experiments found about 17% additional improvement on the frontend-style 64×64 Rayon workload from the first Pass 2 cleanup plus Pass 3 fusion. The same combination improved an all-additive dense 128×128 workload by about 43%.

## Method

Temporary timing hooks measured the tick driver, Pass 2 internals, and individual Pass 3 substeps with `Instant`. The harness was built in release mode, initialized the relevant code paths and Rayon pool before measurement, and accumulated timings across repeated simulations built from identical initial state.

Fixtures:

- Frontend-style 64×64: the four default lithotroph seeds and frontend simulation rates; 1,000 ticks per repetition, normally five repetitions.
- Empty 64×64 and 256×256: isolates full-grid ambient work and fixed scheduling costs.
- Dense additive 64×64 and 128×128: every cell executes `GIVE_E`, producing one additive action per cell per tick and no exclusive actions.
- Dense emit 64×64: every cell emits a packet, exposing the serial packet phase.

The temporary A/B implementations were checked by deterministic workload checksums. After all profiling code was removed, `rust/scripts/check-rayon-parity.sh` passed for the serial cross-process control and Rayon at 1, 2, 4, and 8 threads across the sparse, frontend, dense, and moving-program fixtures.

These are wall-clock measurements on one development machine, not stable benchmark thresholds. Small differences between runs should not be overinterpreted; the phase proportions and A/B deltas were repeatable enough to guide implementation.

## Frontend-style 64×64 baseline

Representative release measurements:

| Phase | Serial | Rayon, 4 threads | Interpretation |
| --- | ---: | ---: | --- |
| Whole tick | about 842 µs | about 688–719 µs | roughly 1.2× overall |
| Prepare tick | 8 µs | 15–17 µs | Rayon overhead exceeds useful work |
| Pass 1 | 55 µs | 62–66 µs | too little live-program work at this size |
| Pass 2 | 220 µs | 231–235 µs | serial and slightly noisier in the Rayon process |
| Packets | 4.5 µs | 4.5 µs | serial, negligible in this fixture |
| Absorb | 23 µs | 24 µs | serial, small in this fixture |
| Background radiation | 169 µs | 90–95 µs | about 1.8–1.9× faster |
| Collect | 4 µs | 23–24 µs | separate Rayon launch is a net loss |
| Background mass | 260 µs | 97–102 µs | about 2.6–2.7× faster |
| Inert lifecycle | 4 µs | 18–19 µs | separate Rayon launch is a net loss |
| Maintenance | 33 µs | 27–28 µs | modest benefit |
| Free-resource decay | 46 µs | 35–36 µs | modest benefit |
| Age update | 4 µs | 16–17 µs | separate Rayon launch is a net loss |
| Spawn check | less than 1 µs | 17–19 µs | separate Rayon launch is a large net loss |
| Mutation | 5 µs | 15–17 µs | separate Rayon launch is a net loss |
| Newborn cleanup | 4 µs | 12–14 µs | separate Rayon launch is a net loss |

Two and four Rayon threads were effectively tied on this workload. Eight threads regressed to about 981 µs per tick, slower than the serial build, because the grid was too small to amortize the additional scheduling.

## Pass 2: deep-copy overhead

`pass2_nonlocal` clones the grid to freeze pre-Pass-2 state. `resolve_exclusive` then clones the post-read/additive grid into `exclusive_base` and clones that again into `working`. At peak, Pass 2 holds the live grid plus three full clones.

These are deep clones. A populated `Cell` contains a `Program`, and its code and stack vectors allocate and copy. Destruction and replacement of the cloned grids add costs that are not fully represented by the clone call timings themselves.

On the frontend fixture, the three directly timed clones consumed about 54% of Pass 2. Grid replacement and clone destruction accounted for much of the remaining unattributed time. On the dense additive fixture, which has no exclusive candidates, roughly 90% of Pass 2 was clone/allocation/assignment/destruction overhead rather than transfer resolution.

Temporary A/B changes:

- Return from exclusive resolution when no action has an exclusive endpoint.
- Use the existing post-additive grid as the immutable exclusive base and create only the working clone, instead of cloning a base and then cloning it again.

Results:

| Workload | Original Pass 2 | A/B Pass 2 | Improvement |
| --- | ---: | ---: | ---: |
| Frontend 64×64 | about 235 µs | about 169–171 µs | about 28% |
| Empty 64×64 | about 74 µs | about 26 µs | about 65% |
| Dense additive 128×128 | about 1.36 ms | about 0.49 ms | about 64% |

### Recommended design

Do not introduce two permanent full mutable grids as the primary solution. Double buffering would avoid some allocation but would preserve excessive copying and double steady-state grid memory.

Use incremental steps:

1. Remove the redundant `exclusive_base` clone. The live grid is already unchanged while the independent working grid resolves exclusive actions. The few pre-exclusive values needed by deferred `MOVE` commits can be copied into the commit record if borrow structure requires it.
2. Skip exclusive allocation, grouping, cloning, and commits when there are no exclusive actions.
3. Pre-resolve action inputs against an immutable grid before applying mutations. Reads can store the observed value, transfers can store bounded amounts, and exclusive candidates can store validation/strength data. This may eliminate the full `pre_pass2` grid clone without weakening simultaneous-action semantics.
4. Reuse grid-sized transfer arrays, incoming-write flags, target groups, move commits, and creation commits through simulation scratch storage.

Every step must retain class ordering, simultaneous validation against pre-Pass-2 state, deterministic target grouping, tie-breaking, and move semantics.

## Pass 3: traversal overhead and fusion

The expensive stochastic loops benefit from Rayon. The cheap lifecycle loops do not. Switching only the cheap loops to serial did not improve the full tick: the serial gaps allowed Rayon workers to park, so later parallel phases paid wake-up and scheduling costs.

The successful A/B used loop fusion instead:

- Fuse collect immediately before background-mass update in one cell traversal.
- Fuse inert lifecycle, maintenance, free-resource decay, age update, and spontaneous creation in one cell traversal, preserving that order within each cell.

All fused tail operations are cell-local and use cell-indexed deterministic random streams. Nevertheless, implementation should explicitly verify that changing cross-cell execution order cannot violate a spec-visible dependency.

Results:

| Workload | Original Rayon tick | Fused Pass 3 tick | Improvement |
| --- | ---: | ---: | ---: |
| Frontend 64×64 | about 688 µs | about 626 µs | about 9% |
| Dense additive 128×128 | about 3.07 ms | about 2.69 ms | about 12% |

On the frontend fixture, collect plus background mass fell from about 120 µs to about 101 µs. The five-stage tail fell from about 113 µs to about 56 µs.

The fused shape slightly slowed the serial build, so keep the straightforward serial traversals and use fusion only for the Rayon path unless later measurements support a shared implementation.

## Other scalability observations

- Empty 256×256: about 9.17 ms serial versus 4.74 ms with four Rayon threads, about 1.94× overall. Background radiation and mass approached 3× speedup. This confirms that Rayon is effective once the full-grid work is large enough.
- Dense emit 64×64: serial packet propagation/bucketing consumed about 103 µs, roughly 10% of the Rayon tick. Packet work is a later optimization candidate, particularly its per-tick `Vec<Vec<Packet>>` bucket allocation, but Pass 2 and Pass 3 fusion are higher priority.
- Absorb resolution was about 24 µs on the frontend fixture. Its serial footprint is too small to prioritize before the structural work above.

## Completion gates for RAYON-PERFORMANCE

- Add focused parity fixtures for every new Pass 2 fast path, especially no actions, additive-only actions, invalid exclusive actions, successful moves, and conflicting exclusives.
- Keep `rust/scripts/check-rayon-parity.sh` green for serial and Rayon at 1, 2, 4, and 8 threads.
- Benchmark true serial versus Rayon rather than using a one-thread Rayon pool as the serial baseline.
- Measure frontend 64×64, a larger sparse grid, a dense additive grid, and a packet-heavy grid.
- Report absolute tick time and phase proportions, not only relative speedup.
- Re-profile after each structural step; do not assume savings from separate experiments add linearly.
