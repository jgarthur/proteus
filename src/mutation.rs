#[derive(Clone, Debug)]
pub struct MutationParams {
    base_mutation_rate_log2: usize,      // -log2(prob)
    radiation_mutation_rate_log2: usize, // -log2(prob)
}

impl Default for MutationParams {
    fn default() -> Self {
        Self {
            base_mutation_rate_log2: 16,
            radiation_mutation_rate_log2: 8,
        }
    }
}
