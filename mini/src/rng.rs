//! Counter-based randomness: every draw is a pure function of (seed, tick, cell, stream).
//!
//! Nothing here carries state between ticks or between cells, so the order in
//! which cells are visited never matters, and changing one law does not shift
//! the noise another cell or another tick sees.

/// Names the independent uses of randomness in a world.
#[derive(Clone, Copy, Debug)]
pub enum Stream {
    Soup = 1,
    Facing = 2,
    Contest = 3,
    Miscopy = 4,
    Slip = 5,
    Upkeep = 6,
    Decay = 7,
    Arrival = 8,
}

/// A SplitMix64 sequence keyed by where and when it is used.
pub struct Rng(u64);

impl Rng {
    /// Keys a sequence to one stream at one cell on one tick of one world.
    pub fn at(seed: u64, tick: u64, cell: usize, stream: Stream) -> Self {
        let mut key = mix(seed ^ 0x9e37_79b9_7f4a_7c15);
        key = mix(key ^ tick);
        key = mix(key ^ cell as u64);
        Self(mix(key ^ stream as u64))
    }

    /// Draws the next 64 uniform bits.
    pub fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        mix(self.0)
    }

    /// Draws a uniform integer in `0..n` without modulo bias.
    pub fn below(&mut self, n: u64) -> u64 {
        debug_assert!(n > 0);
        let zone = u64::MAX - u64::MAX % n;
        loop {
            let draw = self.next();
            if draw < zone {
                return draw % n;
            }
        }
    }

    /// Reports an event of probability `2^-k`; `None` means never.
    ///
    /// Certain and impossible events consume no draw.
    pub fn chance(&mut self, k: Option<u32>) -> bool {
        match k {
            None => false,
            Some(0) => true,
            Some(k) => self.next() >> (64 - k) == 0,
        }
    }

    /// Counts how many of `n` independent events of probability `2^-k` occur.
    pub fn binomial(&mut self, n: u32, k: Option<u32>) -> u32 {
        let Some(k) = k else { return 0 };
        let mut count = 0;
        let mut left = n;
        while left > 0 {
            let block = left.min(64);
            // ANDing k uniform words leaves each bit set with probability 2^-k.
            let hits = (0..k).fold(u64::MAX, |mask, _| mask & self.next());
            count += (hits & (u64::MAX >> (64 - block))).count_ones();
            left -= block;
        }
        count
    }

    /// Draws from a Poisson distribution, given `exp(-mean)`.
    pub fn poisson(&mut self, exp_neg_mean: f64) -> u32 {
        let mut count = 0;
        let mut product = self.unit();
        while product > exp_neg_mean {
            count += 1;
            product *= self.unit();
        }
        count
    }

    /// Draws a uniform number in `[0, 1)`.
    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// The SplitMix64 finalizer.
fn mix(mut z: u64) -> u64 {
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}
