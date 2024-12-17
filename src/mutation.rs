#[derive(Clone, Debug)]
pub struct MutationRules {
    pub base_mutation_rate_log2: usize,      // -log2(prob)
    pub radiation_mutation_rate_log2: usize, // -log2(prob)
}

impl Default for MutationRules {
    fn default() -> Self {
        Self {
            base_mutation_rate_log2: 16,
            radiation_mutation_rate_log2: 8,
        }
    }
}

impl MutationRules {}
