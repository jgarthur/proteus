use rand_core::{RngCore, SeedableRng};
use rand_distr::{Binomial, Distribution};
use rand_xoshiro::SplitMix64;

#[derive(Clone, Debug)]
pub struct WyRand {
    inner: fastrand::Rng,
}

impl WyRand {
    pub fn with_seed(seed: u64) -> Self {
        Self {
            inner: fastrand::Rng::with_seed(seed),
        }
    }

    pub fn next_u64(&mut self) -> u64 {
        self.inner.u64(..)
    }

    pub fn next_u32(&mut self) -> u32 {
        self.inner.u32(..)
    }

    pub fn f64(&mut self) -> f64 {
        self.inner.f64()
    }

    pub fn bernoulli(&mut self, probability: f64) -> bool {
        match probability {
            p if p <= 0.0 => false,
            p if p >= 1.0 => true,
            p => self.f64() < p,
        }
    }
}

impl RngCore for WyRand {
    fn next_u32(&mut self) -> u32 {
        self.inner.u32(..)
    }

    fn next_u64(&mut self) -> u64 {
        self.inner.u64(..)
    }

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

    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), rand_core::Error> {
        self.fill_bytes(dest);
        Ok(())
    }
}

pub fn splitmix64(value: u64) -> u64 {
    let mut rng = SplitMix64::from_seed(value.to_le_bytes());
    rng.next_u64()
}

pub fn cell_rng(master_seed: u64, tick: u64, cell_index: u64) -> WyRand {
    let mixed = splitmix64(
        master_seed
            .wrapping_add(tick.wrapping_mul(0x517c_c1b7_2722_0a95))
            .wrapping_add(cell_index.wrapping_mul(0x6c62_272e_07bb_0142)),
    );
    WyRand::with_seed(mixed)
}

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

#[cfg(test)]
mod tests {
    use super::{binomial, cell_rng, splitmix64};

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
}
