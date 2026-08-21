//! Provides deterministic RNG helpers used across the simulator.

use rand_core::{RngCore, SeedableRng};
use rand_distr::{Distribution, Poisson};
use rand_xoshiro::SplitMix64;

pub const POISSON_INVERSION_MAX_RATE: f64 = 64.0;

/// Wraps the chosen RNG implementation behind a small simulator-facing API.
#[derive(Clone, Debug)]
pub struct WyRand {
    inner: fastrand::Rng,
}

impl WyRand {
    /// Seeds a new RNG instance from a single `u64`.
    pub fn with_seed(seed: u64) -> Self {
        Self {
            inner: fastrand::Rng::with_seed(seed),
        }
    }

    /// Draws the next random `u64`.
    pub fn next_u64(&mut self) -> u64 {
        self.inner.u64(..)
    }

    /// Draws the next random `u32`.
    pub fn next_u32(&mut self) -> u32 {
        self.inner.u32(..)
    }

    /// Draws a uniform floating-point value in `[0, 1)`.
    pub fn f64(&mut self) -> f64 {
        self.inner.f64()
    }

    /// Draws a Bernoulli event with a probability clamp at the extremes.
    pub fn bernoulli(&mut self, probability: f64) -> bool {
        match probability {
            p if p <= 0.0 => false,
            p if p >= 1.0 => true,
            p => self.f64() < p,
        }
    }
}

impl RngCore for WyRand {
    /// Draws the next random `u32` for `rand_core` consumers.
    fn next_u32(&mut self) -> u32 {
        self.inner.u32(..)
    }

    /// Draws the next random `u64` for `rand_core` consumers.
    fn next_u64(&mut self) -> u64 {
        self.inner.u64(..)
    }

    /// Fills a byte slice using repeated 64-bit draws.
    fn fill_bytes(&mut self, dest: &mut [u8]) {
        let mut remaining = dest;
        while remaining.len() >= 8 {
            let bytes = self.inner.u64(..).to_le_bytes();
            remaining[..8].copy_from_slice(&bytes);
            remaining = &mut remaining[8..];
        }
        if !remaining.is_empty() {
            let bytes = self.inner.u64(..).to_le_bytes();
            remaining.copy_from_slice(&bytes[..remaining.len()]);
        }
    }

    /// Fills a byte slice and reports success to `rand_core`.
    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), rand_core::Error> {
        self.fill_bytes(dest);
        Ok(())
    }
}

/// Mixes a seed value with splitmix64 for stable avalanche behavior.
pub fn splitmix64(value: u64) -> u64 {
    let mut rng = SplitMix64::from_seed(value.to_le_bytes());
    rng.next_u64()
}

/// Derives a reproducible per-cell RNG from the master seed, tick, and cell index.
pub fn cell_rng(master_seed: u64, tick: u64, cell_index: u64) -> WyRand {
    let mixed = splitmix64(
        master_seed
            .wrapping_add(tick.wrapping_mul(0x517c_c1b7_2722_0a95))
            .wrapping_add(cell_index.wrapping_mul(0x6c62_272e_07bb_0142)),
    );
    WyRand::with_seed(mixed)
}

/// Draws an exact Bernoulli event with probability `2^-k` for `k` in `0..=63`.
///
/// `k == 0` is probability 1 and returns without consuming a draw. Exponents
/// above 63 have no exact 64-bit representation and panic: `SimConfig::validate`
/// rejects them as configuration errors, so reaching this assertion means a
/// caller bypassed validation. The check is deliberately active in release
/// builds because the masked draw below would otherwise wrap `1 << k` to
/// `1 << 0` and silently invert the sampler into probability 1.
pub fn bernoulli_pow2(rng: &mut WyRand, k: u32) -> bool {
    assert!(k <= 63, "bernoulli_pow2 supports k in 0..=63, got {k}");
    if k == 0 {
        return true;
    }

    rng.next_u64() & ((1_u64 << k) - 1) == 0
}

/// Draws an exact Bernoulli event with probability `min(x / 2^k, 1)`.
///
/// Panics for `k > 63` on the same grounds as [`bernoulli_pow2`]. `k == 0` is
/// still handled correctly (every `x >= 1` saturates to probability 1), but
/// callers are expected to short-circuit that case themselves.
pub fn bernoulli_ratio_pow2(rng: &mut WyRand, x: u32, k: u32) -> bool {
    assert!(
        k <= 63,
        "bernoulli_ratio_pow2 supports k in 1..=63, got {k}"
    );
    debug_assert!((1..=63).contains(&k));
    if x == 0 {
        return false;
    }
    let denominator = 1_u64 << k;
    if u64::from(x) >= denominator {
        return true;
    }

    (rng.next_u64() & (denominator - 1)) < u64::from(x)
}

/// Counts successes among a requested number of independent fair coin flips.
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

/// Draws an exact `Binomial(n, 2^-k)` value for `k` in `0..=63`.
///
/// Each round halves the survivors with random-bit popcounts. The expected bit
/// consumption is less than `2n` regardless of `k`, and a zero survivor count
/// stops subsequent rounds without consuming more draws.
///
/// Panics for `k > 63` on the same grounds as [`bernoulli_pow2`].
pub fn binomial_pow2(rng: &mut WyRand, n: u32, k: u32) -> u32 {
    assert!(k <= 63, "binomial_pow2 supports k in 0..=63, got {k}");
    if n == 0 {
        return 0;
    }
    if k == 0 {
        return n;
    }

    let mut survivors = n;
    for _ in 0..k {
        if survivors == 0 {
            return 0;
        }
        survivors = popcount_random_bits(rng, survivors);
    }
    survivors
}

/// Precomputes constants for repeated small-rate Poisson inversion draws.
#[derive(Clone, Copy, Debug)]
pub struct PoissonInverter {
    rate: f64,
    exp_neg_rate: f64,
}

impl PoissonInverter {
    /// Builds an inverter for a finite rate in `0..=64`.
    pub fn new(rate: f64) -> Self {
        debug_assert!(rate.is_finite() && (0.0..=POISSON_INVERSION_MAX_RATE).contains(&rate));
        Self {
            rate,
            exp_neg_rate: (-rate).exp(),
        }
    }

    /// Draws by cumulative inversion, exact in distribution up to f64 rounding.
    pub fn sample(&self, rng: &mut WyRand) -> u32 {
        if self.rate <= 0.0 {
            return 0;
        }

        let uniform = rng.f64();
        let mut value = 0_u32;
        let mut probability = self.exp_neg_rate;
        let mut cumulative = probability;
        while uniform >= cumulative {
            value += 1;
            probability *= self.rate / f64::from(value);
            let next_cumulative = cumulative + probability;
            if value > 10_000 || next_cumulative == cumulative {
                break;
            }
            cumulative = next_cumulative;
        }
        value
    }
}

/// Draws a Poisson count while handling the zero-rate case cheaply.
pub fn poisson(rng: &mut WyRand, rate: f64) -> u32 {
    if rate <= 0.0 {
        return 0;
    }
    if rate <= POISSON_INVERSION_MAX_RATE {
        return PoissonInverter::new(rate).sample(rng);
    }

    let capped_rate = rate.min(f64::from(u32::MAX));
    let distr = Poisson::new(capped_rate).unwrap();
    let draw = distr.sample(rng);
    draw.min(f64::from(u32::MAX)).floor() as u32
}

#[cfg(test)]
mod tests {
    use super::{
        bernoulli_pow2, bernoulli_ratio_pow2, binomial_pow2, cell_rng, poisson, splitmix64,
        PoissonInverter,
    };

    #[test]
    fn splitmix64_is_stable_for_known_input() {
        assert_eq!(splitmix64(0), 0xe220_a839_7b1d_cdaf);
    }

    #[test]
    fn cell_rng_is_reproducible_for_same_coordinates() {
        let mut first = cell_rng(7, 11, 13);
        let mut second = cell_rng(7, 11, 13);

        assert_eq!(first.next_u64(), second.next_u64());
        assert_eq!(first.next_u64(), second.next_u64());
    }

    #[test]
    fn nearby_cells_get_distinct_streams() {
        let mut left = cell_rng(7, 11, 13);
        let mut right = cell_rng(7, 11, 14);

        assert_ne!(left.next_u64(), right.next_u64());
    }

    #[test]
    #[should_panic(expected = "bernoulli_pow2 supports k in 0..=63, got 64")]
    fn bernoulli_pow2_rejects_unsupported_exponent() {
        let mut rng = cell_rng(1, 1, 1);
        bernoulli_pow2(&mut rng, 64);
    }

    #[test]
    #[should_panic(expected = "bernoulli_ratio_pow2 supports k in 1..=63, got 64")]
    fn bernoulli_ratio_pow2_rejects_unsupported_exponent() {
        let mut rng = cell_rng(1, 1, 1);
        bernoulli_ratio_pow2(&mut rng, 1, 64);
    }

    #[test]
    #[should_panic(expected = "binomial_pow2 supports k in 0..=63, got 64")]
    fn binomial_pow2_rejects_unsupported_exponent() {
        let mut rng = cell_rng(1, 1, 1);
        binomial_pow2(&mut rng, 8, 64);
    }

    #[test]
    fn highest_supported_exponent_still_samples() {
        let mut rng = cell_rng(2, 2, 2);
        assert!(!bernoulli_pow2(&mut rng, 63));
        assert!(!bernoulli_ratio_pow2(&mut rng, 1, 63));
        assert_eq!(binomial_pow2(&mut rng, 8, 63), 0);
    }

    #[test]
    fn exact_sampler_probability_extremes_consume_no_draws() {
        let mut rng = cell_rng(1, 2, 3);
        let mut twin = rng.clone();
        assert!(!rng.bernoulli(0.0));
        assert_eq!(rng.next_u64(), twin.next_u64());

        let mut rng = cell_rng(2, 3, 4);
        let mut twin = rng.clone();
        assert!(rng.bernoulli(1.0));
        assert_eq!(rng.next_u64(), twin.next_u64());

        let mut rng = cell_rng(2, 3, 4);
        let mut twin = rng.clone();
        assert!(bernoulli_pow2(&mut rng, 0));
        assert_eq!(rng.next_u64(), twin.next_u64());

        let mut rng = cell_rng(3, 4, 5);
        let mut twin = rng.clone();
        assert!(!bernoulli_ratio_pow2(&mut rng, 0, 8));
        assert_eq!(rng.next_u64(), twin.next_u64());

        let mut rng = cell_rng(4, 5, 6);
        let mut twin = rng.clone();
        assert!(bernoulli_ratio_pow2(&mut rng, 256, 8));
        assert_eq!(rng.next_u64(), twin.next_u64());

        let mut rng = cell_rng(5, 6, 7);
        let mut twin = rng.clone();
        assert_eq!(binomial_pow2(&mut rng, 0, 7), 0);
        assert_eq!(rng.next_u64(), twin.next_u64());

        let mut rng = cell_rng(6, 7, 8);
        let mut twin = rng.clone();
        assert_eq!(binomial_pow2(&mut rng, 123, 0), 123);
        assert_eq!(rng.next_u64(), twin.next_u64());

        let mut rng = cell_rng(7, 8, 9);
        let mut twin = rng.clone();
        assert_eq!(PoissonInverter::new(0.0).sample(&mut rng), 0);
        assert_eq!(rng.next_u64(), twin.next_u64());
    }

    #[test]
    fn exact_samplers_are_deterministic() {
        let mut first = cell_rng(17, 19, 23);
        let mut second = cell_rng(17, 19, 23);
        let poisson = PoissonInverter::new(8.0);

        for _ in 0..1_000 {
            assert_eq!(
                bernoulli_pow2(&mut first, 4),
                bernoulli_pow2(&mut second, 4)
            );
            assert_eq!(
                bernoulli_ratio_pow2(&mut first, 13, 8),
                bernoulli_ratio_pow2(&mut second, 13, 8)
            );
            assert_eq!(
                binomial_pow2(&mut first, 64, 3),
                binomial_pow2(&mut second, 64, 3)
            );
            assert_eq!(poisson.sample(&mut first), poisson.sample(&mut second));
        }
    }

    #[test]
    fn binomial_pow2_matches_small_exact_distribution() {
        let mut rng = cell_rng(29, 31, 37);
        let draws = 200_000_u32;
        let mut counts = [0_u32; 3];
        for _ in 0..draws {
            counts[binomial_pow2(&mut rng, 2, 1) as usize] += 1;
        }

        for (actual, expected) in counts.into_iter().zip([0.25, 0.5, 0.25]) {
            let frequency = f64::from(actual) / f64::from(draws);
            assert!((frequency - expected).abs() < 0.01);
        }
    }

    #[test]
    fn exact_sampler_moments_match_their_distributions() {
        const DRAWS: u32 = 200_000;

        let mut binomial_rng = cell_rng(41, 43, 47);
        assert_moments(
            DRAWS,
            || f64::from(binomial_pow2(&mut binomial_rng, 64, 3)),
            8.0,
            7.0,
        );

        let mut bernoulli_rng = cell_rng(53, 59, 61);
        assert_moments(
            DRAWS,
            || f64::from(bernoulli_pow2(&mut bernoulli_rng, 4)),
            1.0 / 16.0,
            15.0 / 256.0,
        );

        for (seed, rate) in [(67, 0.25), (71, 8.0)] {
            let mut rng = cell_rng(seed, 73, 79);
            let inverter = PoissonInverter::new(rate);
            assert_moments(DRAWS, || f64::from(inverter.sample(&mut rng)), rate, rate);
        }
    }

    #[test]
    fn poisson_respects_zero_rate() {
        let mut rng = cell_rng(1, 2, 3);
        assert_eq!(poisson(&mut rng, 0.0), 0);
    }

    #[test]
    fn poisson_can_draw_more_than_one_arrival() {
        let mut rng = cell_rng(7, 11, 13);
        assert!(poisson(&mut rng, 100.0) > 1);
    }

    fn assert_moments(
        draws: u32,
        mut sample: impl FnMut() -> f64,
        expected_mean: f64,
        expected_variance: f64,
    ) {
        let values: Vec<f64> = (0..draws).map(|_| sample()).collect();
        let mean = values.iter().sum::<f64>() / f64::from(draws);
        let variance = values
            .iter()
            .map(|value| (value - mean).powi(2))
            .sum::<f64>()
            / f64::from(draws);

        let three_sigma = 3.0 * (expected_variance / f64::from(draws)).sqrt();
        assert!(
            (mean - expected_mean).abs() <= three_sigma,
            "mean {mean} differs from {expected_mean} by more than {three_sigma}"
        );
        assert!((variance - expected_variance).abs() <= expected_variance * 0.1);
    }
}
