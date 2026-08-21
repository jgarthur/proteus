# Dyadic sampler implementation results

Date: 2026-08-21

## Summary

Implemented DYADIC-SAMPLERS from the 2026-08-13 plan. Configured hot-path probabilities now use exact power-of-two sampling, repeated small-rate Poisson draws use inversion with precomputed constants, and the simulator/spec version is v0.3.0.

## Implementation

- Added stream-preserving exponent-1 fast paths for local-action budgets and maintenance scaling.
- Added exact `Bernoulli(2^-k)`, `Bernoulli(min(x / 2^k, 1))`, and `Binomial(n, 2^-k)` helpers with deterministic no-draw behavior at probability endpoints.
- Added a cumulative Poisson inverter for rates up to 64 and retained `rand_distr::Poisson` as the large-rate fallback.
- Hoisted ambient Poisson constants once per Pass 3 ambient call and steady-state constants once per simulation initialization.
- Converted decay, maintenance, spawn, and mutation call sites without changing unrelated raw RNG draws.
- Validated dyadic probability fields and rejected mutation exponents above 63.
- Bumped the crate and simulator spec to v0.3.0 and migrated defaults, examples, fixtures, and runner digest material.

## Draw-stream migration

Phase 0 was byte-identical before and after its fast paths. The six 1000-tick grid digests remained `046aeac015f9f70b`, `3b5e3d060a1230da`, `f06dd9a38e965480`, `1c8e49b5edcb8c3d`, `0c3890443552161d`, and `0d790cf1d7b8a8cf` in fixture order.

Phase 1 intentionally changes sampled draw streams. The runner example manifest digest changed from `sha256:e5053c7b811503c8d875fde5bb1a85d6fbbfb15596d591c300fae351cb11ac03` to `sha256:1ee9860658e963b0fdc49d1660b47c5f37c7d8845aeb5560c5cc593cc6e9953b` because the two decay values changed.

## Verification

Passed the required gates:

```text
cargo fmt
cargo clippy --all-targets --all-features
cargo test
cargo test --features rayon
./scripts/check-rayon-parity.sh 1000 1 2 4 8
```

Structural conservation, determinism, property, and shuffled-order assertions and test logic passed unmodified; only fixture probabilities that the new validation rejects were migrated to approved dyadic values. The parity script also confirmed reproducibility in a second serial process.

Accepted Phase 1 1000-tick grid digests:

- `sparse-8x8`: `780b783c188997fd`
- `frontend-64x64`: `19277d5f3d319a83`
- `dense-32x32`: `fdad63ee74501784`
- `moving-8x1`: `1c8e49b5edcb8c3d`
- `all-opcodes-71x1`: `0c3890443552161d`
- `exclusive-5x9`: `0d790cf1d7b8a8cf`

## Benchmarks (quiet machine, 2026-08-21)

Run after implementation on a quiet window (load1 = 1.91): two interleaved
base-vs-branch rounds, `tick_bench 1000 5` per fixture serial and
`RAYON_NUM_THREADS=4`, plus the dense web window (`100 3 web-256x256-single
6500`). Numbers below are the min across rounds, whole-tick µs/tick, with the
ambient phase in parentheses. Base is `main` @ 5b90499.

| Fixture | Mode | Base | Branch | Δ total | Δ ambient |
| --- | --- | ---: | ---: | ---: | ---: |
| empty-64x64 | serial | 499.4 | 322.1 | -35.5% | -34.3% |
| empty-256x256 | serial | 8003.8 | 5248.2 | -34.4% | -33.4% |
| empty-256x256 | rayon-4 | 2461.8 | 1730.4 | -29.7% | -31.5% |
| frontend-64x64 | serial | 583.1 | 467.8 | -19.8% | -34.0% |
| frontend-64x64 | rayon-4 | 344.3 | 327.0 | -5.0% | -20.4% |
| web-256x256-single (t0) | serial | 8283.6 | 5331.2 | -35.6% | -35.3% |
| web-256x256-single (t0) | rayon-4 | 2594.8 | 1865.9 | -28.1% | -30.8% |
| dense-additive-128x128 | serial | 2402.4 | 1698.6 | -29.3% | -34.7% |
| dense-additive-128x128 | rayon-4 | 1006.4 | 831.4 | -17.4% | -29.4% |
| dense-emit-64x64 | serial | 739.9 | 551.9 | -25.4% | -43.9% |
| dense-emit-64x64 | rayon-4 | 485.5 | 433.8 | -10.6% | -28.5% |

Population comparability was checked per fixture: the empty and synthetic
dense fixtures are identical on both sides (0 and 100% occupancy
respectively), web-from-tick-0 is near-identical (1568 vs 1635 final
programs), and frontend-64x64 is *understated* — the branch world grows to
2201 programs vs 1445 yet still ticks 19.8% faster.

**The tick-6500 dense web window is not a valid A/B and is excluded.** Under
the migrated `d_* = 2^-7` decay values the web scenario's ecology changes
qualitatively: the branch world reaches only 6,404 programs (9.8% occupancy)
at tick 6500 where the base reached 52,560 (80.2%). The raw window deltas
(-67.8% serial, -72.0% rayon-4) therefore mostly measure a sparser world, not
sampler speed; the dense-additive-128x128 rows above are the honest dense-regime
measurement. The ecology shift itself is a consequence of the approved spec
change worth its own follow-up: rebalancing `r_*` against the new decay
defaults is allowed and orthogonal (plan §parameter-inventory note).

