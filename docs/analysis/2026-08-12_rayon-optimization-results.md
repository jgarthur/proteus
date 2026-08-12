# RAYON-PERFORMANCE results

Date: 2026-08-12

This note closes the `RAYON-PERFORMANCE` implementation plan in
`2026-08-12_rayon-optimization-plan.md`. Simulator semantics remain governed by
`SPEC.md` and `SPEC-COMPANION.md`; the measurements here are machine-specific
engineering evidence, not performance requirements.

## Outcome

Pass 2 no longer clones the grid. The Rayon build now fuses compatible ambient
and tick-tail cell traversals, while the serial loop shapes remain separate.
Packet resolution chooses between stable sorting for sparse traffic and direct
bucketing for dense traffic. Absorption skips all allocation when no program
absorbed and retains source-ordered bucketing when absorption is active.

On the supplied growing 256x256 web scenario, a four-thread Rayon build improved
from 76.98 ms/tick to 9.19 ms/tick in a window beginning at 80.2% occupancy
(8.38x). The serial build improved from 92.23 ms/tick to 21.04 ms/tick (4.38x).
The timed window produced identical event totals and final state summaries before
and after the implementation.

## Exact web scenario

`tick_bench` now includes `web-256x256-single`, constructed through
`Simulation::new` and `apply_bootstrap` rather than a synthetic grid. It exactly
uses the supplied seed `6846702536457205`, 256x256 dimensions, resource rates,
maintenance and mutation settings, and the 12-byte lithotroph at `(32, 32)` with
20 free energy and 12 free mass. This means tick-zero background resources and
seed-register initialization follow the production web path.

The optional fourth benchmark argument replays an unobserved checkpoint before
timing. The checkpoint is built once and cloned for every repetition, so replay
cost is excluded and each repetition begins from the same complete simulation
state.

```bash
# First 1,000 ticks, three repetitions
cargo run --release --example tick_bench -- 1000 3 web-256x256-single
RAYON_NUM_THREADS=4 cargo run --release --features rayon --example tick_bench -- \
  1000 3 web-256x256-single

# 100-tick window beginning after tick 6,500
RAYON_NUM_THREADS=4 cargo run --release --features rayon --example tick_bench -- \
  100 3 web-256x256-single 6500
```

### Growing startup window

Mean wall time over ticks 0-999, three repetitions:

| Build | Before (ms/tick) | After (ms/tick) | Speedup |
| --- | ---: | ---: | ---: |
| Serial | 10.232 | 8.434 | 1.21x |
| Rayon, 4 threads | 5.774 | 3.408 | 1.69x |

Every repetition ended with 1,635 programs, 1,561 live programs, 1,673 births,
120 deaths, 525 mutations, and 76 packets. Occupancy was still only 2.5%, so this
window mostly measures full-grid ambient work plus early growth.

### Dense growth window

After 6,500 replayed ticks the checkpoint contained 52,560 programs, 52,289 of
them live: 80.2% occupancy. The timed interval was ticks 6,500-6,599, with three
repetitions:

| Phase | Serial before | Serial after | Rayon-4 before | Rayon-4 after |
| --- | ---: | ---: | ---: | ---: |
| prepare | 0.118 | 0.116 | 0.076 | 0.071 |
| Pass 1 | 17.652 | 7.374 | 7.767 | 2.699 |
| Pass 2 | 61.140 | 0.574 | 62.889 | 0.699 |
| packets | 0.329 | 0.110 | 0.385 | 0.120 |
| ambient | 8.363 | 8.256 | 4.324 | 4.200 |
| tail | 4.415 | 4.404 | 1.380 | 1.241 |
| mutation | 0.141 | 0.140 | 0.109 | 0.112 |
| newborn clear | 0.068 | 0.068 | 0.048 | 0.046 |
| **total** | **92.226** | **21.043** | **76.977** | **9.187** |

Times are milliseconds per tick. Each repetition produced 1,609 births, 171
deaths, and 2,812 mutations and ended with 2,166 packets, 54,027 programs, and
53,730 live programs (82.4% occupancy). The matching activity and final counts
are a benchmark sanity check; full-state equality is enforced separately by the
parity digest.

The large dense win comes primarily from removing three deep grid clones from
Pass 2. The growing programs contain heap-backed code and stacks, so clone cost
increases much faster than cell occupancy alone suggests.

Pass 1 itself was not changed, despite its dense serial timing falling from
17.65 to 7.37 ms/tick. The likely explanation is a secondary allocator and cache
effect: the old Pass 2 repeatedly cloned and freed tens of thousands of
heap-backed programs and kept three extra grids live around each tick. Treat the
Pass 1 change as a measured whole-process effect, not a direct Pass 1 speedup.

## Broader fixture sweep

These quiet-session measurements used 300 ticks and three repetitions on an
Apple M2. They compare preserved pre-change binaries against the final code from
the same session. Values are whole-tick microseconds.

| Fixture | Serial before | Serial after | Serial speedup | Rayon-4 before | Rayon-4 after | Rayon speedup |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| frontend-64x64 | 617.5 | 520.2 | 1.19x | 518.2 | 307.2 | 1.69x |
| web-256x256-single | 10,042.1 | 8,355.1 | 1.20x | 4,494.1 | 2,759.4 | 1.63x |
| empty-64x64 | 566.4 | 489.3 | 1.16x | 452.2 | 277.3 | 1.63x |
| empty-256x256 | 9,605.2 | 7,854.5 | 1.22x | 4,373.8 | 2,539.6 | 1.72x |
| dense-additive-128x128 | 3,702.1 | 2,421.5 | 1.53x | 2,421.0 | 1,040.6 | 2.33x |
| dense-emit-64x64 | 1,160.8 | 724.2 | 1.60x | 1,001.4 | 488.0 | 2.05x |

The exact web row here covers only ticks 0-299 and should not be confused with
the 80%-occupied checkpoint above.

## Fixture and instruction audit

The original four parity fixtures are deterministic sentinels with distinct jobs:

- `sparse-8x8` stresses tiny Rayon chunks, spontaneous creation, mutation,
  maintenance deaths, packets, and a mixture of local operations.
- `frontend-64x64` is a moderate growing lithotroph ecology with production-like
  rates and boot-based reproduction.
- `dense-32x32` keeps every parallel chunk busy and combines live and inert
  programs, high mutation, spontaneous and boot births, absorption, transfers,
  packets, and contention.
- `moving-8x1` targets program-carried tick-start eligibility after movement. Its
  four programs eventually die, so focused integration tests remain responsible
  for move success, resource transfer, and write-before-deferred-move ordering.

At 1,000 ticks, their observed event totals were:

| Fixture | Births | Deaths | Mutations | Max packets | Final live |
| --- | ---: | ---: | ---: | ---: | ---: |
| sparse-8x8 | 2,089 spawn | 2,054 | 6,389 | 9 | 49 |
| frontend-64x64 | 1,520 boot | 164 | 555 | 60 | 1,378 |
| dense-32x32 | 2,179 (10 boot, 2,169 spawn) | 2,193 | 63,921 | 210 | 1,004 |
| moving-8x1 | 0 | 4 | 19 | 0 | 0 |

Two audit fixtures were added:

- `all-opcodes-71x1` starts one live, resource-rich carrier for every
  spec-defined instruction byte. All 71 remained live through the 1,000-tick
  parity run.
- `exclusive-5x9` asserts before replay that READ_ADJ, WRITE_ADJ, APPEND_ADJ,
  DEL_ADJ, GIVE_E, GIVE_M, MOVE, and BOOT each succeed, and that a mixed
  WRITE/APPEND conflict changes its target.

Long ecological replays cannot by themselves prove instruction frequency:
self-modification, mutation, and death obscure which byte ran. A deterministic
Pass 1 census therefore creates 32 independent carriers for every one of the 71
spec instruction bytes and executes exactly one pass. That guarantees 2,272
intended dispatches with populated stacks and ample resources; focused unit and
integration tests cover success, failure, contention, and ordering branches.
This is reasonable coverage of the optimized surfaces, but it is not a claim of
exhaustive state-space or branch coverage.

## Line profile and measured reversals

A 15-second macOS `sample` profile was captured from the four-thread dense web
run beginning at tick 6,500. The dominant compute stacks were Rayon worker
iteration, binomial and Poisson sampling, opcode decoding, `log_gamma`, and
floating-point power. Mutation plus newborn clearing was only about 0.16 ms/tick
in the bounded benchmark, so the optional API-shuffling fusion was not justified.

The profile also caught a proposed regression: stable sorting of absorption
claims accounted for 755 top-of-stack samples and made the dense four-thread
ambient phase 6.16 ms/tick. Restoring source-ordered buckets when absorption is
active, while retaining a zero-absorption early return, reduced it to 4.20
ms/tick. Similarly, always sorting packets improved sparse traffic but changed
the dense-emitter packet phase from 77.6 to 122.7 microseconds/tick. The final
adaptive resolver uses buckets above 25% packet density and measured 76.0
microseconds/tick in the serial dense-emitter fixture.

The next useful profile work is therefore algorithmic investigation of ambient
random sampling and Pass 1 dispatch, not additional blind Rayon launch fusion.

## Verification

- `cargo test`
- `cargo test --features rayon`
- `cargo clippy --all-targets --all-features`
- `cargo fmt --check`
- `./scripts/check-rayon-parity.sh 1000 1 2 4 8`

The deep parity check compares full grid, report, and packet digests for all six
fixtures, first verifies the serial path is reproducible across processes, and
then verifies bit-identical output at Rayon thread counts 1, 2, 4, and 8.

## Serial-path decision

The separate serial paths remain worthwhile. With the final code, a one-thread
Rayon pool was 3-15% slower than the direct serial build across the six benchmark
fixtures. Four Rayon threads were 1.48-3.09x faster than serial in the tick-zero
fixture sweep and 2.29x faster in the 80%-occupied web window. Keep Rayon as the
parallel build path without replacing direct serial iteration with a one-thread
pool.
