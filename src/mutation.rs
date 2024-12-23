use rand::Rng;

use crate::instruction::Instruction;

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

impl MutationRules {
    /// Mutate an instruction.
    /// Currently just generates a random instruction.
    pub fn mutate_instruction<R: Rng + ?Sized>(
        &self,
        rng: &mut R,
        _instruction: Instruction,
    ) -> Instruction {
        rng.gen()
    }

    /// Get amount to decrement mutation counter
    pub fn get_counter_decrement(&self, background_rad: Option<u8>) -> u32 {
        match background_rad {
            None => 1,
            Some(rad) => {
                // radiation_amount * 2^(rad_mut_rate - base_mut_rate)
                (rad as u32) * (2 << (self.mut_rate_log2 - self.rad_mut_rate_log2))
            }
        }
    }
}
