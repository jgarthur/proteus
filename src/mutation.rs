#[derive(Clone, Debug)]
pub struct MutationRules {
    pub mut_rate_log2: usize,     // -log2(prob)
    pub rad_mut_rate_log2: usize, // -log2(prob)
}

impl Default for MutationRules {
    fn default() -> Self {
        Self {
            mut_rate_log2: 16,
            rad_mut_rate_log2: 8,
        }
    }
}

impl MutationRules {}
