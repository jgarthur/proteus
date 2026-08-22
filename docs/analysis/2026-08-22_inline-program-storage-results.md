# Inline program storage (RAYON-HOTSPOTS Tier 2b): negative result

Date: 2026-08-22. Branch `task/inline-code` (`0f43b74`, fixes `149cb80`), not merged.

## What was tried

`Program.code` moved from a heap `Vec<u8>` to a fixed-capacity inline
`ProgramCode` (`[u8; CAP]` + length, zeroed tail, slice equality), per
`2026-08-13_performance-roadmap.md` §2b. `PROGRAM_SIZE_CAP` was split into
`PROGRAM_CODE_CAP` and `STACK_DEPTH_CAP` (stacks stay heap `Vec`s; the census
shows p50 depth 759 / max 32 767, so no inline cap exists). Over-cap
`appendAdj` is refused through the existing flag-set path; no draw stream
changes. The branch is correct — parity digests are byte-identical to `main`
across all six fixtures and threads 1/2/4/8, structural tests pass
unmodified, and the cross-family review approved it.

## Benchmark

`tick_bench`, release, quiet machine (load1 ≈ 2.5), two interleaved rounds,
min of five reps, 1000 ticks; identical `final_programs` on every side unless
noted. `Cell` is 128 B on `main`, 176 B at cap 64, 624 B at cap 512.

| Fixture | main (ms) | cap 512 | cap 256 | cap 64 |
| --- | ---: | ---: | ---: | ---: |
| empty-256x256, serial | 5045 | +7.1% | +4.2% | +4.0% |
| empty-256x256, Rayon-4 | 1707 | +17.9% | +11.2% | +3.8% |
| empty-64x64, serial | 309 | +4.4% | +3.4% | — |
| dense-additive-128x128, serial | 1708 | +2.4% | +2.0% | +1.7% |
| dense-additive-128x128, Rayon-4 | 825 | +1.9% | +2.3% | — |
| dense-emit-64x64, serial / Rayon-4 | 560 / 442 | +3.6% / +1.0% | +3.5% / +2.6% | — |
| frontend-64x64, serial | 485 | +1.2% | +0.4% | — |
| web-256x256 grown @6500, Rayon-4 | 4248 | +10.8% | +5.8% ‡ | — |

‡ cap 256 binds in the grown world (31 417 final programs vs 31 818 on
`main` and cap 512), so that cell compares a different ecology. A 256
instruction cap is therefore an ecological change, not just a storage
choice; 512 is the floor the data supports.

## Why the roadmap's prediction missed

The roadmap argued that sweeps ignoring code already fetch one cache line
per cell, so a wider `Cell` would only cost TLB reach, and that removing
allocator traffic and the code pointer-chase would pay on dense grids.
Neither held:

- The empty-grid rows isolate the stride cost: with no programs at all, the
  ambient/tail/prepare sweeps slow 7% serial and 18% under Rayon at 624 B,
  and still 4% at 176 B. Rayon amplifies it because four threads now share
  memory bandwidth over a 4.9x larger working set.
- The dense rows are where the allocator and pointer-chase savings should
  appear, and they are net negative at every cap, including cap 64 where
  the stride increase is near minimal (+1.7%). The savings exist but are
  smaller than the cost of touching more bytes per cell.

Conclusion: inline-in-`Cell` code storage loses at any cap on this
workload. A separate code arena in `Grid` (keeping `Cell` at 128 B) would
remove the stride cost but, per the cap-64 dense result, would at best
break even — the allocator-traffic premise is too weak to justify it. Tier
2b is shelved. The same data argues for Tier 2a (SoA hot-field split):
shrinking what the code-free sweeps touch is where the empty-grid time goes.

## Gates and review

`cargo fmt`, `clippy --all-targets --all-features -D warnings`, `cargo test`
(default, `rayon`, `web`), `scripts/check-rayon-parity.sh 1000 1 2 4 8`
identical to `main`. Review findings (1 medium, 4 low) all closed in
`149cb80`, including a test that pins `STACK_DEPTH_CAP` from below after the
reviewer showed a stack silently capped at 512 left the whole suite green.
