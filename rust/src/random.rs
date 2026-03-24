//! Provides deterministic RNG helpers used across the simulator.

use rand_core::{RngCore, SeedableRng};
use rand_distr::{Binomial, Distribution, Poisson};
use rand_xoshiro::SplitMix64;

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

/// Draws a binomial count while handling the probability edge cases cheaply.
pub fn binomial(rng: &mut WyRand, n: u32, probability: f64) -> u32 {
    if probability <= 0.0 {
        return 0;
    }
    if probability >= 1.0 {
        return n;
    }

    let distr = Binomial::new(n.into(), probability).unwrap();
    distr.sample(rng) as u32
}

/// Draws a Poisson count while handling the zero-rate case cheaply.
pub fn poisson(rng: &mut WyRand, rate: f64) -> u32 {
    if rate <= 0.0 {
        return 0;
    }

    let capped_rate = rate.min(f64::from(u32::MAX));
    let distr = Poisson::new(capped_rate).unwrap();
    let draw = distr.sample(rng);
    draw.min(f64::from(u32::MAX)).floor() as u32
}

#[cfg(test)]
mod tests {
    use super::{binomial, cell_rng, poisson, splitmix64};

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
    fn binomial_respects_probability_extremes() {
        let mut rng = cell_rng(1, 2, 3);
        assert_eq!(binomial(&mut rng, 5, 0.0), 0);
        assert_eq!(binomial(&mut rng, 5, 1.0), 5);
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
}
