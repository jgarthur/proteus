//! Defines the simulator configuration surface and its validation rules.

use std::error::Error;
use std::fmt;

use serde::{Deserialize, Serialize};

/// Tracks the spec version this backend is aligned to.
pub const SPEC_VERSION: &str = "0.3.0";
/// Stores the maximum allowed program length from the spec.
pub const PROGRAM_SIZE_CAP: u16 = 0x7fff;

/// Holds the tunable parameters that shape one simulation run.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
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
            d_energy: 1.0 / 128.0,
            d_mass: 1.0 / 128.0,
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
        self.check_dyadic_probability("d_energy", self.d_energy)?;
        self.check_dyadic_probability("d_mass", self.d_mass)?;
        self.check_dyadic_probability("maintenance_rate", self.maintenance_rate)?;
        self.check_dyadic_probability("p_spawn", self.p_spawn)?;

        if self.mutation_base_log2 > 63 {
            return Err(ConfigError::DyadicExponentTooLarge {
                field: "mutation_base_log2",
                value: self.mutation_base_log2,
            });
        }
        if self.mutation_background_log2 > 63 {
            return Err(ConfigError::DyadicExponentTooLarge {
                field: "mutation_background_log2",
                value: self.mutation_background_log2,
            });
        }

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

    /// Checks that a floating-point field is an exactly representable dyadic probability.
    fn check_dyadic_probability(&self, field: &'static str, value: f64) -> Result<(), ConfigError> {
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err(ConfigError::ProbabilityOutOfRange { field, value });
        }
        if value == 0.0 || dyadic_exponent(value).is_some() {
            return Ok(());
        }

        let (nearest_lower, nearest_upper) = dyadic_probability_neighbors(value);
        Err(ConfigError::NotDyadicProbability {
            field,
            value,
            nearest_lower,
            nearest_upper,
        })
    }

    /// Checks that a floating-point field is finite and non-negative.
    fn check_non_negative(&self, field: &'static str, value: f64) -> Result<(), ConfigError> {
        if !value.is_finite() || value < 0.0 {
            return Err(ConfigError::NegativeOrNonFinite { field, value });
        }
        Ok(())
    }
}

/// Returns `Some(k)` exactly when `p == 2^-k` for `k` in `0..=63`.
///
/// Zero is handled separately by callers because it has no finite exponent.
pub(crate) fn dyadic_exponent(p: f64) -> Option<u32> {
    let bits = p.to_bits();
    let sign = bits >> 63;
    let mantissa = bits & ((1_u64 << 52) - 1);
    let biased_exp = (bits >> 52) & 0x7ff;
    if sign != 0 || mantissa != 0 || biased_exp == 0 {
        return None;
    }

    let exponent = biased_exp as i64 - 1023;
    (-63..=0).contains(&exponent).then(|| (-exponent) as u32)
}

/// Returns the valid dyadic probabilities bracketing an in-range invalid value.
fn dyadic_probability_neighbors(value: f64) -> (f64, f64) {
    let minimum = 2_f64.powi(-63);
    if value < minimum {
        return (0.0, minimum);
    }

    let biased_exp = (value.to_bits() >> 52) & 0x7ff;
    let lower = f64::from_bits(biased_exp << 52);
    let upper = f64::from_bits((biased_exp + 1) << 52);
    (lower, upper)
}

/// Describes why a simulation config is invalid.
#[derive(Clone, Debug, PartialEq)]
pub enum ConfigError {
    ZeroWidth,
    ZeroHeight,
    GridTooLarge {
        width: u32,
        height: u32,
    },
    ProbabilityOutOfRange {
        field: &'static str,
        value: f64,
    },
    NotDyadicProbability {
        field: &'static str,
        value: f64,
        nearest_lower: f64,
        nearest_upper: f64,
    },
    DyadicExponentTooLarge {
        field: &'static str,
        value: u32,
    },
    NegativeOrNonFinite {
        field: &'static str,
        value: f64,
    },
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
            Self::NotDyadicProbability {
                field,
                value,
                nearest_lower,
                nearest_upper,
            } => write!(
                f,
                "{field} must be exactly 0, 1, or 2^-k for k in 1..=63; got {value}, whose nearest bracketing valid values are {nearest_lower} and {nearest_upper}"
            ),
            Self::DyadicExponentTooLarge { field, value } => {
                write!(
                    f,
                    "{field} must be at most 63 for exact dyadic sampling, got {value}"
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
    use super::{dyadic_exponent, ConfigError, SimConfig};

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

    #[test]
    fn dyadic_exponent_recognizes_only_supported_exact_powers_of_two() {
        assert_eq!(dyadic_exponent(1.0), Some(0));
        assert_eq!(dyadic_exponent(0.5), Some(1));
        assert_eq!(dyadic_exponent(2_f64.powi(-63)), Some(63));
        assert_eq!(dyadic_exponent(0.0), None);
        assert_eq!(dyadic_exponent(-0.5), None);
        assert_eq!(dyadic_exponent(0.75), None);
        assert_eq!(dyadic_exponent(2_f64.powi(-64)), None);
    }

    #[test]
    fn dyadic_probability_fields_accept_zero_one_and_supported_powers() {
        for value in [0.0, 1.0, 0.25, 2_f64.powi(-63)] {
            let config = SimConfig {
                d_energy: value,
                d_mass: value,
                maintenance_rate: value,
                p_spawn: value,
                ..SimConfig::default()
            };
            assert_eq!(config.validate(), Ok(()));
        }
    }

    #[test]
    fn non_dyadic_probability_reports_bracketing_valid_values() {
        let config = SimConfig {
            d_energy: 0.01,
            ..SimConfig::default()
        };

        assert_eq!(
            config.validate(),
            Err(ConfigError::NotDyadicProbability {
                field: "d_energy",
                value: 0.01,
                nearest_lower: 0.0078125,
                nearest_upper: 0.015625,
            })
        );
    }

    #[test]
    fn mutation_exponents_above_63_are_rejected() {
        let base = SimConfig {
            mutation_base_log2: 64,
            ..SimConfig::default()
        };
        assert_eq!(
            base.validate(),
            Err(ConfigError::DyadicExponentTooLarge {
                field: "mutation_base_log2",
                value: 64,
            })
        );

        let background = SimConfig {
            mutation_background_log2: u32::MAX,
            ..SimConfig::default()
        };
        assert_eq!(
            background.validate(),
            Err(ConfigError::DyadicExponentTooLarge {
                field: "mutation_background_log2",
                value: u32::MAX,
            })
        );
    }
}
