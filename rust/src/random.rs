#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WyRand {
    state: u64,
}

impl WyRand {
    pub fn with_seed(seed: u64) -> Self {
        Self { state: seed }
    }

    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0xa076_1d64_78bd_642f);
        let mixed = (self.state as u128).wrapping_mul((self.state ^ 0xe703_7ed1_a0b4_28db) as u128);
        ((mixed >> 64) ^ mixed) as u64
    }

    pub fn next_u32(&mut self) -> u32 {
        (self.next_u64() >> 32) as u32
    }

    pub fn f64(&mut self) -> f64 {
        const SCALE: f64 = 1.0 / ((1_u64 << 53) as f64);
        ((self.next_u64() >> 11) as f64) * SCALE
    }

    pub fn bernoulli(&mut self, probability: f64) -> bool {
        match probability {
            p if p <= 0.0 => false,
            p if p >= 1.0 => true,
            p => self.f64() < p,
        }
    }
}

pub fn splitmix64(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9e37_79b9_7f4a_7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
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

    (0..n).map(|_| u32::from(rng.bernoulli(probability))).sum()
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
