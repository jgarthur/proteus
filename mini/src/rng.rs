//! Counter-based randomness: every draw is a pure function of (seed, tick, site, stream).
//!
//! Nothing here carries state between ticks or between cells, so changing a rule
//! never shifts the noise another cell or another tick sees.

/// Names the independent uses of randomness in a world.
#[derive(Clone, Copy, Debug)]
pub enum Stream {
    Soup = 1,
    Schedule = 2,
    Mutation = 3,
}

/// A SplitMix64 sequence keyed by where and when it is used.
pub struct Rng(u64);

impl Rng {
    /// Keys a sequence to one stream at one site on one tick of one world.
    pub fn at(seed: u64, tick: u64, site: u64, stream: Stream) -> Self {
        let mut key = mix(seed ^ 0x9e37_79b9_7f4a_7c15);
        key = mix(key ^ tick);
        key = mix(key ^ site);
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
}

/// The SplitMix64 finalizer.
fn mix(mut z: u64) -> u64 {
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}
