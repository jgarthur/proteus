use rand::Rng;

/// Samples from a geometric distribution with success probability p = 1 / 2^k.
///
/// Convention used is the total number of trials until a success, including the
/// successful trial.
pub fn geometric_pow2<R: Rng + ?Sized>(rng: &mut R, k: usize) -> u64 {
    assert!(k > 0 && k <= 64, "k must be a positive integer <= 64",);
    let mut num_trials: u64 = 1;
    let mut bits: u64 = 0;
    let mut bits_remaining: usize = 0;

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

/// Samples from a binomial distribution with probability p = 1 / 2^k
pub fn binomial_pow2<R: Rng + Sized>(rng: &mut R, n: u64, k: usize) -> u64 {
    assert!(k > 0 && k <= 64, "k must be a positive integer <= 64",);
    let mut acc = 0;
    let mut bits: u64 = rng.gen();
    let mut bits_remaining: usize = 0;

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

#[cfg(test)]
mod tests {
    use super::*;

    fn test_geometric_pow2(k: usize, num_samples: usize) {
        let mut rng = rand::thread_rng();
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

    fn test_binomial_pow2(k: usize, n: u64, num_samples: u64) {
        let mut rng = rand::thread_rng();
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
    fn test_binomial_sweep_k() {
        for k in 1..=16 {
            println!("Testing for k = {}", k);
            test_binomial_pow2(k, 100, 100_000);
        }
    }
}
