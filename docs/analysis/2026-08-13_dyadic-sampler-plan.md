# Dyadic sampler implementation plan

Date: 2026-08-13

This plan specifies the next RAYON-HOTSPOTS performance step: replacing the
per-draw `rand_distr` binomial/Poisson machinery with exact bit-level samplers
for power-of-two probabilities and a precomputed-constant Poisson inversion.
It is written to be executed by an agent without prior context. It follows the
line-profile finding in `2026-08-12_rayon-optimization-results.md` that ambient
and tail sampling (binomial, Poisson, `log_gamma`, floating-point power)
dominate the optimized tick.

This is analysis material, not authority. The changes below include a
deliberate, user-approved spec adjustment (decay probabilities become exact
powers of two). **This document intentionally does not draft the spec text**:
the implementing agent must edit `SPEC.md` (and check `SPEC-COMPANION.md`)
per `docs/CLAUDE.md`, log the change in `SPEC-CHANGELOG.md`, and keep the edit
minimal. Read `docs/BACKEND-GUIDELINES.md` and `rust/AGENTS.md` first.

## Why this works

Joey's observation: every probability on the per-cell hot path is (or is being
made) `p = 2^-k` for integer `k`. A Bernoulli(2^-k) trial is "k fair coin
flips all heads", so binomial and Bernoulli draws collapse to integer bit
operations with **zero floating-point transcendentals** and are **exact** —
unlike the current f64 pipeline. Poisson rates are not dyadic and need no bit
trick; hoisting `exp(-λ)` out of the per-draw path is enough.

Current cost per ambient cell per tick (`rust/src/random.rs`): two
`cell_rng` constructions plus two `Binomial::new` + sample (the BINV path
computes `q^n` — a `powf` — per call) plus two `Poisson::new` + sample (an
`exp` per call). Measured ~126 ns/cell serial; the samplers below bring the
sampling portion to ~20-30 ns/cell.

## Parameter inventory (verify against the tree)

| Config field | Current value | Dyadic? | Action |
| --- | --- | --- | --- |
| `mutation_base_log2` | 16 | yes, stored as integer k | no config change; use exact sampler |
| `mutation_background_log2` | 8 | yes, stored as integer k | no config change; use exact sampler |
| `maintenance_rate` | 0.0078125 = 2^-7 (default 1/128) | yes by convention | validate dyadic |
| `p_spawn` | 0.0 (fixtures use 0.25, 0.5) | yes | validate dyadic |
| `d_energy`, `d_mass` | 0.01 | **no** | **spec change**: new defaults 2^-7 = 0.0078125 (or 2^-6 = 0.015625 — Joey decides), validate dyadic |
| `r_energy`, `r_mass` | 0.25, 1.0 | n/a (Poisson rates) | stay free-form non-negative f64 |

Ecology note for the spec decision: steady-state background is `r/d`. With
`r_energy = 0.25`, moving `d` from 0.01 to 2^-7 raises mean background
radiation from 25 to 32; 2^-6 lowers it to 16. Rebalancing `r` to compensate
is allowed and orthogonal (r stays free-form).

## Phase 0 (independent, land first): exponent-1.0 `powf` fast paths

Stream-preserving — no digests change, no version bump, can ship alone.
IEEE `powf(x, 1.0)` returns exactly `x`, so these are pure speed:

- `rust/src/pass3.rs` `resolve_maintenance_cell`: `q = size.powf(beta)` →
  `if beta == 1.0 { f64::from(size) } else { powf }`.
- `rust/src/pass1.rs` `local_action_budget`: same guard for
  `local_action_exponent == 1.0` (result `size.max(1)` unchanged).

Verify with the full gate set (below); digests must be byte-identical, so the
parity script doubles as proof these are value-preserving.

## Phase 1: dyadic samplers and Poisson inversion

Everything in this phase changes RNG draw sequences. That is accepted:
serial-vs-Rayon parity is unaffected (both builds share this code), but all
golden/expected values derived from sampled runs change, and the crate/spec
version must be bumped (see "Migration").

### 1a. Config validation

Keep the f64 config fields (no API churn). In `SimConfig::validate`
(`rust/src/config.rs`), require `d_energy`, `d_mass`, `maintenance_rate`, and
`p_spawn` to be exactly `0.0`, `1.0`, or `2^-k` for integer `k` in `1..=63`.
Detection without float loops:

```rust
/// Returns Some(k) when p == 2^-k for k in 0..=63, None otherwise.
/// p == 1.0 returns Some(0). Callers handle p == 0.0 separately.
fn dyadic_exponent(p: f64) -> Option<u32> {
    let bits = p.to_bits();
    let mantissa = bits & ((1_u64 << 52) - 1);
    let biased_exp = (bits >> 52) & 0x7ff;
    if mantissa != 0 || biased_exp == 0 {
        return None; // not a power of two, or zero/subnormal
    }
    let exp = biased_exp as i64 - 1023; // p == 2^exp
    (-63..=0).contains(&exp).then(|| (-exp) as u32)
}
```

Add a `ConfigError` variant (e.g. `NotDyadicProbability { field, value }`)
with a display message that names the nearest valid values. Update the two
non-dyadic defaults (`SimConfig::default`, `frontend/src/constants.ts`, the
runner example manifests `rust/example-run.json` and `rust/example-batch/`,
and any web-layer defaults) to the chosen `2^-k`. Derive and store nothing in
`SimConfig` itself — it is `Serialize`/`PartialEq`; compute `k` at use sites
via `dyadic_exponent(...)` (cheap) or pass it down locally.

Also validate `mutation_base_log2` and `mutation_background_log2` `<= 63`
so the bit samplers below are total; document that larger values previously
meant "effectively never" and now mean exactly never (config validation error
is also acceptable — pick one and state it in the spec changelog).

### 1b. Exact Bernoulli(2^-k) and Bernoulli(x / 2^k)

In `rust/src/random.rs`:

```rust
/// Draws an exact Bernoulli(2^-k) event for k in 0..=63.
/// k == 0 means probability 1. Always consumes exactly one u64.
pub fn bernoulli_pow2(rng: &mut WyRand, k: u32) -> bool {
    debug_assert!(k <= 63);
    if k == 0 {
        return true; // still no draw; probability 1 short-circuits like bernoulli(1.0)
    }
    rng.next_u64() & ((1_u64 << k) - 1) == 0
}

/// Draws an exact Bernoulli(min(x / 2^k, 1)) event for k in 1..=63.
pub fn bernoulli_ratio_pow2(rng: &mut WyRand, x: u32, k: u32) -> bool {
    debug_assert!((1..=63).contains(&k));
    if u64::from(x) >= (1_u64 << k) {
        return true; // saturated at probability 1, matching min(x/2^k, 1); no draw
    }
    (rng.next_u64() & ((1_u64 << k) - 1)) < u64::from(x)
}
```

Convention to keep consistent everywhere: probability-0 and probability-1
branches consume **no** draws (this matches the existing
`WyRand::bernoulli` clamps).

### 1c. Exact Binomial(n, 2^-k): halving cascade

Binomial(n, 2^-k) is n trials thinned k times by 1/2, and thinning m
survivors by 1/2 is a popcount of m random bits:

```rust
/// Counts successes among `bits` fair coin flips.
fn popcount_random_bits(rng: &mut WyRand, mut bits: u32) -> u32 {
    let mut count = 0;
    while bits >= 64 {
        count += rng.next_u64().count_ones();
        bits -= 64;
    }
    if bits > 0 {
        count += (rng.next_u64() & ((1_u64 << bits) - 1)).count_ones();
    }
    count
}

/// Draws an exact Binomial(n, 2^-k) for k in 0..=63.
pub fn binomial_pow2(rng: &mut WyRand, n: u32, k: u32) -> u32 {
    debug_assert!(k <= 63);
    if n == 0 {
        return 0; // no draws — the dominant case on decay paths
    }
    if k == 0 {
        return n; // probability 1, no draws
    }
    let mut survivors = n;
    for _ in 0..k {
        if survivors == 0 {
            return 0; // early exit consumes no further draws
        }
        survivors = popcount_random_bits(rng, survivors);
    }
    survivors
}
```

Properties worth a comment in the code: exact for any `n` (large `n` just
chunks the popcount); expected bit consumption `< 2n` regardless of `k`
because survivors halve each round; the early exit makes the expected round
count `~min(k, log2(n) + 2)`. Do **not** add a shared bit buffer across
rounds in the first version — the per-round draw above is deterministic,
simple, and already removes all transcendentals. A buffered variant is a
follow-up only if a fresh profile blames `next_u64` volume.

### 1d. Poisson by inversion with precomputed `exp(-λ)`

Replace the per-draw `Poisson::new` with Knuth-style inversion using a
cumulative recurrence; the only transcendental is one `exp(-λ)` computed
**once per call site invocation** (per tick, not per cell):

```rust
/// Precomputed constants for repeated Poisson(rate) draws.
#[derive(Clone, Copy)]
pub struct PoissonInverter {
    rate: f64,
    exp_neg_rate: f64, // exp(-rate), computed once
}

impl PoissonInverter {
    pub fn new(rate: f64) -> Self { ... }

    /// Draws by sequential inversion; exact-in-distribution up to f64 rounding.
    pub fn sample(&self, rng: &mut WyRand) -> u32 {
        let u = rng.f64();
        let mut i = 0_u32;
        let mut p = self.exp_neg_rate;
        let mut cumulative = p;
        while u >= cumulative {
            i += 1;
            p *= self.rate / f64::from(i);
            cumulative += p;
            // The loop terminates: cumulative -> 1 and u < 1. Guard anyway:
            if i > 10_000 { break; }
        }
        i
    }
}
```

Use inversion when `rate <= 64.0`; for larger rates keep the existing
`rand_distr::Poisson` fallback (per-tick ambient rates are ~0.25-1.0 in every
real config; only `initialize_background_steady_state`, whose mean is `r/d`,
can exceed 64, and it runs once at simulation creation). Expected work per
draw at λ ≤ 1 is one uniform plus one or two multiply-adds.

Plumbing: **no API changes.** Build the (at most two) `PoissonInverter`
values at the top of `pass3_ambient` (and one inside
`initialize_background_steady_state` / anywhere else `poisson` is called with
a loop around it) each call — construction is one `exp`, amortized over the
whole grid. The free function `poisson(rng, rate)` in `random.rs` can remain
for arbitrary one-off draws, reimplemented on the same inversion.

### 1e. Call-site conversion map

Keep each cell's logical draw *order* unchanged (first decay, then arrival,
etc.); only the underlying draw counts change. Every site derives `k` via
`dyadic_exponent` from the config field, treating `None` as unreachable after
validation (`expect`), and handles `p == 0.0` with the existing zero-branch.

- `resolve_background_radiation_cell` (pass3): `binomial(bg, d_energy)` →
  `binomial_pow2(rng, bg, k_d_energy)`; `poisson(r_energy)` → inverter sample.
- `resolve_background_mass_cell`: same with `d_mass` / `r_mass`.
- `resolve_free_resource_decay_cell`: two `binomial_pow2` calls with the
  respective `k`; the `n == 0` early-out now skips all drawing, which is the
  common case for cells at or below their threshold.
- `resolve_maintenance_cell`: `binomial(whole, rate)` →
  `binomial_pow2(whole, k_maintenance)`. The fractional term
  `Bernoulli((q - floor(q)) * rate)` is only nonzero when
  `maintenance_exponent != 1.0`; its probability is not dyadic in general, so
  keep the existing f64 `rng.bernoulli` for that term and say so in a comment.
- `resolve_spontaneous_creation_cell`: `rng.bernoulli(p_spawn)` →
  `bernoulli_pow2(rng, k_spawn)` (zero-probability branch unchanged);
  direction and id draws stay in the same order afterward.
- `mutate_end_of_tick_cell` / `mutation_probability` (pass3): replace the
  f64 probability computation and `bernoulli` with:
  background-stressed (`bg_radiation_consumed > 0`) →
  `bernoulli_ratio_pow2(rng, consumed, mutation_background_log2)`;
  otherwise → `bernoulli_pow2(rng, mutation_base_log2)`. The subsequent
  instruction-index and bit-index draws keep their current order.
- `initialize_background_steady_state` (simulation.rs): Poisson with mean
  `r/d` — inverter when mean ≤ 64, `rand_distr` fallback above.

Sites that must NOT change: listen capture, exclusive tie-break,
append-create direction/id, spawn direction/id, mutation index/bit — all raw
`next_u32`/`next_u64` uses.

### 1f. Sampler unit tests

In `random.rs` tests (fixed seeds, no statistical flakiness):

- Exactness edges: `binomial_pow2(rng, 0, k) == 0` and consumes no draws
  (compare rng state via a following `next_u64` against a twin rng);
  `binomial_pow2(rng, n, 0) == n` likewise; same no-draw checks for the
  Bernoulli 0/1 branches and the `bernoulli_ratio_pow2` saturation branch.
- Distribution sanity at scale (deterministic seed, large sample): mean of
  `binomial_pow2(_, 64, 3)` within ±3σ of `64/8`; variance within ~10%;
  same for `bernoulli_pow2` at k=4 and the inverter at λ=0.25 and λ=8
  (mean≈λ, variance≈λ).
- Cross-check small cases exhaustively against binomial probabilities:
  sample `binomial_pow2(_, 2, 1)` many times and check the 1/4-1/2-1/4 split
  within tolerance.
- Determinism: identical seeds produce identical sequences.

## Migration and verification

Draw streams change, so every sampled expectation moves:

1. Bump the crate version minor (`rust/Cargo.toml`) and update `SPEC_VERSION`
   only as part of the actual spec edit (dyadic constraint + new `d` defaults +
   changelog entry). Update the top-level `README.md` per repo versioning
   rules if `y` changes.
2. Expect failures in integration tests whose literals encode sampled
   outcomes (`tick_driver.rs`, `multi_tick_scenarios.rs`,
   `multi_tick_regressions.rs`, `pass3_ambient.rs`, conservation totals,
   possibly web/runner digests). For each failure: confirm the delta is
   plausible (a changed random outcome, not a structural bug), then update
   the literal. Structural invariants (conservation sums, determinism,
   property tests, shuffled-order equivalence) must pass **unmodified** — if
   one of those fails, the sampler is wrong; stop and fix.
3. Fixture configs with non-dyadic values must be updated: the parity and
   bench fixtures use `d_*` values of 0.01, 0.05, 0.15, and 0.2 and
   `maintenance_rate` values of 0.25, 0.02, and 0.007_812_5, plus
   `p_spawn = 0.25`/`0.5`. Map each probability to the nearest `2^-k`
   (0.5, 0.25, and 0.0078125 are already exact; 0.2 → 2^-2 or 2^-3;
   0.15 → 2^-3; 0.05 → 2^-4 or 2^-5; 0.02 → 2^-6; 0.01 → 2^-7 or 2^-6).
   The new config validation rejects any missed value loudly, so run the
   test suite to find stragglers. `r_*` values (0.35, 0.45, 0.6, ...) are
   Poisson rates and stay as-is. `frontend_config` in `tick_bench.rs` and
   `parity_digest.rs` must stay identical to each other.
4. Full gates: `cargo fmt`, `cargo clippy --all-targets --all-features`,
   `cargo test`, `cargo test --features rayon`,
   `./scripts/check-rayon-parity.sh 1000 1 2 4 8` (the serial control run in
   that script also guards cross-process determinism of the new samplers).
5. Re-benchmark on a quiet machine (`uptime` first): `tick_bench 1000 5`
   serial and Rayon-4 across all fixtures, plus the dense web window
   (`100 3 web-256x256-single 6500`). Expected effect: ambient and tail drop
   substantially (ambient sampling ~126 → ~20-30 ns/cell serial); whole-tick
   dense web Rayon-4 from ~9.2 ms toward ~5-6 ms. Record results in a dated
   analysis note and update `STATUS.md`/`docs/BACKLOG.md`.

## Explicitly out of scope here

- Pass 1 dispatch cost and any data-layout work (SoA hot-field split, inline
  program storage behind code/stack caps). Those are the remaining
  RAYON-HOTSPOTS threads; capping the stack alone buys nothing today, and
  caps only pay off together with inline storage. A prerequisite metric —
  program-size and stack-depth histograms from a dense run — should be added
  before any cap is chosen.
- Bit-buffered binomial rounds, SIMD batching, GPU offload.
- Changing the arrival distribution itself (e.g., negative binomial as a sum
  of geometrics) — considered and rejected as a performance lever. After the
  inversion rewrite, Poisson draws at production rates cost one uniform plus
  at most a couple of multiply-adds, and inversion-by-recurrence would make
  any small-mean count distribution (including NB, whose pmf has the same
  one-step recurrence) O(1 + mean) anyway. Distribution choice is purely a
  modeling/spec decision.
