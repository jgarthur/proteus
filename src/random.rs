use rand::Rng;

/// Samples from a geometric distribution with success probability p = 1 / 2^k.
///
/// Convention used is the total number of trials until a success, including the
/// successful trial.
pub fn geometric_pow2_old<R: Rng + ?Sized>(rng: &mut R, k: usize) -> u64 {
    assert!(k > 0 && k <= 64, "k must be a positive integer <= 64",);
    let mut num_trials: u64 = 1;
    let mut bits: u64 = rng.gen();
    let mut bits_remaining: usize = 64;

    let mask = if k < 64 { (1u64 << k) - 1 } else { u64::MAX };

    loop {
        // Refill buffer if needed
        if bits_remaining < k {
            bits = rng.gen();
            bits_remaining = 64;
        }
        // If lowest k bits are zero, than success
        let group = bits & mask;
        if group == 0 {
            return num_trials;
        }
        bits >>= k;
        bits_remaining -= k;
        num_trials += 1;
    }
}

/// Samples from a geometric distribution with success probability p = 1 / 2^k.
///
/// Convention used is the total number of trials until a success, including the
/// successful trial.
pub fn geometric_pow2<R: Rng + ?Sized>(rng: &mut R, k: usize) -> u64 {
    assert!(k > 0 && k <= 64, "k must be a positive integer <= 64",);

    let mut num_trials: u64 = 1;
    // k least significant bits are 1
    let mask = if k < 64 { (1u64 << k) - 1 } else { u64::MAX };
    let groups_per_word = 64 / k;
    let mut bits: u64;

    loop {
        bits = rng.gen::<u64>();
        for _ in 0..groups_per_word {
            if bits & mask == 0 {
                return num_trials;
            }
            num_trials += 1;
            bits >>= k;
        }
    }
}

/// Samples from a binomial distribution with probability p = 1 / 2^k
pub fn binomial_pow2_old<R: Rng + Sized>(rng: &mut R, n: u64, k: usize) -> u64 {
    assert!(k > 0 && k <= 64, "k must be a positive integer <= 64",);
    let mut acc = 0;
    let mut bits: u64 = rng.gen();
    let mut bits_remaining: usize = 64;

    let mask = if k < 64 { (1u64 << k) - 1 } else { u64::MAX };

    for _ in 0..n {
        // Refill buffer if needed
        if bits_remaining < k {
            bits = rng.gen();
            bits_remaining = 64;
        }
        // If lowest k bits are zero, than success
        let group = bits & mask;
        if group == 0 {
            acc += 1;
        }
        bits >>= k;
        bits_remaining -= k;
    }

    return acc;
}

pub fn binomial_pow2<R: Rng + ?Sized>(rng: &mut R, n: u64, k: usize) -> u64 {
    assert!(k > 0 && k <= 64, "k must be a positive integer <= 64");

    let mut acc = 0u64;
    // k least significant bits are 1
    let mask = if k == 64 { u64::MAX } else { (1u64 << k) - 1 };
    let groups_per_word = 64 / k;
    let mut trials_remaining = n;
    let mut bits: u64;

    while trials_remaining >= groups_per_word as u64 {
        bits = rng.gen::<u64>();
        for _ in 0..groups_per_word {
            acc += (bits & mask == 0) as u64;
            bits >>= k;
        }
        trials_remaining -= groups_per_word as u64;
    }
    if trials_remaining > 0 {
        bits = rng.gen::<u64>();
        for _ in 0..trials_remaining {
            acc += (bits & mask == 0) as u64;
            bits >>= k;
        }
    }

    acc
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::rngs::SmallRng;
    use rand::SeedableRng;

    fn test_geometric_pow2(k: usize, num_samples: usize) {
        let mut rng = SmallRng::seed_from_u64(17);
        let mut sum = 0.0;
        let mut sum_of_squares = 0.0;
        let p = 1.0 / (1 << k) as f64;

        for _ in 0..num_samples {
            let sample = geometric_pow2(&mut rng, k);
            sum += sample as f64;
            sum_of_squares += (sample as f64).powi(2);
        }

        let empirical_mean = sum / num_samples as f64;
        let empirical_variance = (sum_of_squares / num_samples as f64) - empirical_mean.powi(2);
        let theoretical_mean = 1.0 / p;
        let theoretical_variance = (1.0 - p) / (p * p);

        println!(
            "Empirical mean: {:.4e}, Theoretical mean: {:.4e}",
            empirical_mean, theoretical_mean
        );
        println!(
            "Empirical variance: {:.4e}, Theoretical variance: {:.4e}",
            empirical_variance, theoretical_variance
        );
    }

    #[test]
    fn test_geom_sweep_k() {
        for k in 1..=8 {
            println!("{}", k);
            test_geometric_pow2(k, 100_000);
        }
    }

    fn test_binomial_pow2(n: u64, k: usize, num_samples: u64) {
        let mut rng = SmallRng::seed_from_u64(17);
        let mut sum = 0.0;
        let mut sum_of_squares = 0.0;
        let p = 1.0 / (1 << k) as f64;

        for _ in 0..num_samples {
            let sample = binomial_pow2(&mut rng, n, k);
            sum += sample as f64;
            sum_of_squares += (sample as f64).powi(2);
        }

        let empirical_mean = sum / num_samples as f64;
        let empirical_variance = (sum_of_squares / num_samples as f64) - empirical_mean.powi(2);
        let theoretical_mean = n as f64 * p;
        let theoretical_variance = n as f64 * p * (1.0 - p);

        println!(
            "Empirical mean: {:.4e}, Theoretical mean: {:.4e}",
            empirical_mean, theoretical_mean
        );
        println!(
            "Empirical variance: {:.4e}, Theoretical variance: {:.4e}",
            empirical_variance, theoretical_variance
        );
    }

    #[test]
    fn test_binomial_implementations_match() {
        let rng = SmallRng::seed_from_u64(17);

        for k in 1..=16 {
            for n in [10, 100, 1000] {
                // Clone RNG to get identical streams
                let mut rng1 = rng.clone();
                let mut rng2 = rng.clone();

                let result1 = binomial_pow2_old(&mut rng1, n, k);
                let result2 = binomial_pow2(&mut rng2, n, k);

                assert_eq!(
                    result1, result2,
                    "Results differ for k={}, n={}: original={}, unrolled={}",
                    k, n, result1, result2
                );
            }
        }
    }

    #[test]
    fn test_geometric_implementations_match() {
        let rng = SmallRng::seed_from_u64(17);

        for k in 1..=16 {
            // Clone RNG to get identical streams
            let mut rng1 = rng.clone();
            let mut rng2 = rng.clone();

            let result1 = geometric_pow2_old(&mut rng1, k);
            let result2 = geometric_pow2(&mut rng2, k);

            assert_eq!(
                result1, result2,
                "Results differ for k={}: original={}, unrolled={}",
                k, result1, result2
            );
        }
    }

    #[test]
    fn test_binomial_sweep_n_k() {
        for n in [10, 100] {
            for k in 1..=8 {
                println!("Testing for k = {}", k);
                test_binomial_pow2(n, k, 100_000);
            }
        }
    }
}
