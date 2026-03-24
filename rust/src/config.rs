//! Defines the simulator configuration surface and its validation rules.

use std::error::Error;
use std::fmt;

/// Tracks the spec version this backend is aligned to.
pub const SPEC_VERSION: &str = "0.2.1";
/// Stores the maximum allowed program length from the spec.
pub const PROGRAM_SIZE_CAP: u16 = 0x7fff;

/// Holds the tunable parameters that shape one simulation run.
#[derive(Clone, Debug, PartialEq)]
pub struct SimConfig {
    pub width: u32,
    pub height: u32,
    pub seed: u64,
    pub r_energy: f64,
    pub r_mass: f64,
    pub d_energy: f64,
    pub d_mass: f64,
    pub t_cap: f64,
    pub maintenance_rate: f64,
    pub maintenance_exponent: f64,
    pub local_action_exponent: f64,
    pub n_synth: u32,
    pub inert_grace_ticks: u32,
    pub p_spawn: f64,
    pub mutation_base_log2: u32,
    pub mutation_background_log2: u32,
}

impl Default for SimConfig {
    /// Builds the spec-aligned default simulation configuration.
    fn default() -> Self {
        Self {
            width: 128,
            height: 128,
            seed: 0,
            r_energy: 0.25,
            r_mass: 0.05,
            d_energy: 0.01,
            d_mass: 0.01,
            t_cap: 4.0,
            maintenance_rate: 1.0 / 128.0,
            maintenance_exponent: 1.0,
            local_action_exponent: 1.0,
            n_synth: 1,
            inert_grace_ticks: 10,
            p_spawn: 0.0,
            mutation_base_log2: 16,
            mutation_background_log2: 8,
        }
    }
}

impl SimConfig {
    /// Validates that a config can safely drive a simulation.
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.width == 0 {
            return Err(ConfigError::ZeroWidth);
        }
        if self.height == 0 {
            return Err(ConfigError::ZeroHeight);
        }

        self.check_non_negative("r_energy", self.r_energy)?;
        self.check_non_negative("r_mass", self.r_mass)?;
        self.check_probability("d_energy", self.d_energy)?;
        self.check_probability("d_mass", self.d_mass)?;
        self.check_probability("maintenance_rate", self.maintenance_rate)?;
        self.check_probability("p_spawn", self.p_spawn)?;

        self.check_non_negative("t_cap", self.t_cap)?;
        self.check_non_negative("maintenance_exponent", self.maintenance_exponent)?;
        self.check_non_negative("local_action_exponent", self.local_action_exponent)?;

        self.cell_count()
            .map(|_| ())
            .ok_or(ConfigError::GridTooLarge {
                width: self.width,
                height: self.height,
            })
    }

    /// Returns the total number of cells implied by the grid dimensions.
    pub fn cell_count(&self) -> Option<usize> {
        let width = usize::try_from(self.width).ok()?;
        let height = usize::try_from(self.height).ok()?;
        width.checked_mul(height)
    }

    /// Checks that a floating-point field is a valid unit probability.
    fn check_probability(&self, field: &'static str, value: f64) -> Result<(), ConfigError> {
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err(ConfigError::ProbabilityOutOfRange { field, value });
        }
        Ok(())
    }

    /// Checks that a floating-point field is finite and non-negative.
    fn check_non_negative(&self, field: &'static str, value: f64) -> Result<(), ConfigError> {
        if !value.is_finite() || value < 0.0 {
            return Err(ConfigError::NegativeOrNonFinite { field, value });
        }
        Ok(())
    }
}

/// Describes why a simulation config is invalid.
#[derive(Clone, Debug, PartialEq)]
pub enum ConfigError {
    ZeroWidth,
    ZeroHeight,
    GridTooLarge { width: u32, height: u32 },
    ProbabilityOutOfRange { field: &'static str, value: f64 },
    NegativeOrNonFinite { field: &'static str, value: f64 },
}

impl fmt::Display for ConfigError {
    /// Formats a human-readable config validation error.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroWidth => write!(f, "config width must be greater than zero"),
            Self::ZeroHeight => write!(f, "config height must be greater than zero"),
            Self::GridTooLarge { width, height } => {
                write!(
                    f,
                    "grid dimensions {width}x{height} do not fit in memory indexing"
                )
            }
            Self::ProbabilityOutOfRange { field, value } => {
                write!(
                    f,
                    "{field} must be a finite probability in [0, 1], got {value}"
                )
            }
            Self::NegativeOrNonFinite { field, value } => {
                write!(f, "{field} must be finite and non-negative, got {value}")
            }
        }
    }
}

impl Error for ConfigError {}

#[cfg(test)]
mod tests {
    use super::{ConfigError, SimConfig};

    #[test]
    fn default_config_is_valid() {
        let config = SimConfig::default();
        assert_eq!(config.validate(), Ok(()));
        assert_eq!(config.cell_count(), Some(16_384));
    }

    #[test]
    fn arrival_rate_above_one_is_valid() {
        let config = SimConfig {
            r_energy: 3.5,
            ..SimConfig::default()
        };

        assert_eq!(config.validate(), Ok(()));
    }

    #[test]
    fn negative_arrival_rate_is_rejected() {
        let config = SimConfig {
            r_energy: -0.5,
            ..SimConfig::default()
        };

        assert_eq!(
            config.validate(),
            Err(ConfigError::NegativeOrNonFinite {
                field: "r_energy",
                value: -0.5
            })
        );
    }
}
