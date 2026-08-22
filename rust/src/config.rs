//! Defines the simulator configuration surface and its validation rules.

use std::error::Error;
use std::fmt;

use serde::{Deserialize, Serialize};

/// Tracks the spec version this backend is aligned to.
pub const SPEC_VERSION: &str = "0.4.0";
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
    #[serde(default = "default_d_energy_log2")]
    pub d_energy_log2: Option<u32>,
    #[serde(default = "default_d_mass_log2")]
    pub d_mass_log2: Option<u32>,
    pub t_cap: f64,
    #[serde(default = "default_maintenance_rate_log2")]
    pub maintenance_rate_log2: Option<u32>,
    pub maintenance_exponent: f64,
    pub local_action_exponent: f64,
    pub n_synth: u32,
    pub inert_grace_ticks: u32,
    #[serde(default = "default_p_spawn_log2")]
    pub p_spawn_log2: Option<u32>,
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
            d_energy_log2: default_d_energy_log2(),
            d_mass_log2: default_d_mass_log2(),
            t_cap: 4.0,
            maintenance_rate_log2: default_maintenance_rate_log2(),
            maintenance_exponent: 1.0,
            local_action_exponent: 1.0,
            n_synth: 1,
            inert_grace_ticks: 10,
            p_spawn_log2: default_p_spawn_log2(),
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
        self.check_exponent("d_energy_log2", self.d_energy_log2)?;
        self.check_exponent("d_mass_log2", self.d_mass_log2)?;
        self.check_exponent("maintenance_rate_log2", self.maintenance_rate_log2)?;
        self.check_exponent("p_spawn_log2", self.p_spawn_log2)?;
        self.check_exponent("mutation_base_log2", Some(self.mutation_base_log2))?;
        self.check_exponent(
            "mutation_background_log2",
            Some(self.mutation_background_log2),
        )?;

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

    /// Checks that an optional dyadic exponent fits the exact sampler domain.
    fn check_exponent(&self, field: &'static str, value: Option<u32>) -> Result<(), ConfigError> {
        if let Some(value @ 64..) = value {
            return Err(ConfigError::ExponentOutOfRange { field, value });
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

pub(crate) const fn default_d_energy_log2() -> Option<u32> {
    Some(7)
}

pub(crate) const fn default_d_mass_log2() -> Option<u32> {
    Some(7)
}

pub(crate) const fn default_maintenance_rate_log2() -> Option<u32> {
    Some(7)
}

pub(crate) const fn default_p_spawn_log2() -> Option<u32> {
    None
}

/// Describes why a simulation config is invalid.
#[derive(Clone, Debug, PartialEq)]
pub enum ConfigError {
    ZeroWidth,
    ZeroHeight,
    GridTooLarge { width: u32, height: u32 },
    ExponentOutOfRange { field: &'static str, value: u32 },
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
            Self::ExponentOutOfRange { field, value } => {
                write!(
                    f,
                    "{field} must be an integer exponent in 0..=63, got {value}"
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

    #[test]
    fn optional_exponent_fields_accept_never_and_supported_exponents() {
        for value in [None, Some(0), Some(7), Some(63)] {
            let config = SimConfig {
                d_energy_log2: value,
                d_mass_log2: value,
                maintenance_rate_log2: value,
                p_spawn_log2: value,
                ..SimConfig::default()
            };
            assert_eq!(config.validate(), Ok(()));
        }
    }

    #[test]
    fn exponents_above_63_are_rejected_with_the_field_name() {
        for (field, config) in [
            (
                "d_energy_log2",
                SimConfig {
                    d_energy_log2: Some(64),
                    ..SimConfig::default()
                },
            ),
            (
                "d_mass_log2",
                SimConfig {
                    d_mass_log2: Some(64),
                    ..SimConfig::default()
                },
            ),
            (
                "maintenance_rate_log2",
                SimConfig {
                    maintenance_rate_log2: Some(64),
                    ..SimConfig::default()
                },
            ),
            (
                "p_spawn_log2",
                SimConfig {
                    p_spawn_log2: Some(64),
                    ..SimConfig::default()
                },
            ),
            (
                "mutation_base_log2",
                SimConfig {
                    mutation_base_log2: 64,
                    ..SimConfig::default()
                },
            ),
            (
                "mutation_background_log2",
                SimConfig {
                    mutation_background_log2: 64,
                    ..SimConfig::default()
                },
            ),
        ] {
            assert_eq!(
                config.validate(),
                Err(ConfigError::ExponentOutOfRange { field, value: 64 })
            );
        }
    }

    #[test]
    fn serde_distinguishes_absent_null_and_value_for_optional_exponents() {
        for field in ["d_energy_log2", "d_mass_log2", "maintenance_rate_log2"] {
            let mut absent = serde_json::to_value(SimConfig::default()).unwrap();
            absent.as_object_mut().unwrap().remove(field);
            let parsed: SimConfig = serde_json::from_value(absent).unwrap();
            assert_eq!(field_value(&parsed, field), Some(7), "absent {field}");

            let mut null = serde_json::to_value(SimConfig::default()).unwrap();
            null[field] = serde_json::Value::Null;
            let parsed: SimConfig = serde_json::from_value(null).unwrap();
            assert_eq!(field_value(&parsed, field), None, "null {field}");

            let mut value = serde_json::to_value(SimConfig::default()).unwrap();
            value[field] = serde_json::json!(12);
            let parsed: SimConfig = serde_json::from_value(value).unwrap();
            assert_eq!(field_value(&parsed, field), Some(12), "value {field}");
        }

        let mut absent = serde_json::to_value(SimConfig::default()).unwrap();
        absent.as_object_mut().unwrap().remove("p_spawn_log2");
        let parsed: SimConfig = serde_json::from_value(absent).unwrap();
        assert_eq!(parsed.p_spawn_log2, None);

        let mut null = serde_json::to_value(SimConfig::default()).unwrap();
        null["p_spawn_log2"] = serde_json::Value::Null;
        let parsed: SimConfig = serde_json::from_value(null).unwrap();
        assert_eq!(parsed.p_spawn_log2, None);

        let mut value = serde_json::to_value(SimConfig::default()).unwrap();
        value["p_spawn_log2"] = serde_json::json!(9);
        let parsed: SimConfig = serde_json::from_value(value).unwrap();
        assert_eq!(parsed.p_spawn_log2, Some(9));
    }

    #[test]
    fn none_serializes_as_explicit_null() {
        let serialized = serde_json::to_value(SimConfig::default()).unwrap();
        assert_eq!(serialized["p_spawn_log2"], serde_json::Value::Null);
    }

    fn field_value(config: &SimConfig, field: &str) -> Option<u32> {
        match field {
            "d_energy_log2" => config.d_energy_log2,
            "d_mass_log2" => config.d_mass_log2,
            "maintenance_rate_log2" => config.maintenance_rate_log2,
            other => panic!("unexpected field {other}"),
        }
    }
}
