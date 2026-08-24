//! Defines the HTTP and WebSocket payload types for the web API.

use serde::{Deserialize, Deserializer, Serialize};

use crate::bootstrap::BootstrapConfig;
pub use crate::bootstrap::{
    EnvironmentPreload as SeedEnvironmentConfig, SeedProgram as SeedProgramConfig,
};
use crate::config::SimConfig;
use crate::observe::MetricsSnapshot;

/// Declares the current version string for the HTTP/WebSocket API.
pub const API_VERSION: &str = "0.3.1";
/// Names the response header that reports the API version.
pub const API_VERSION_HEADER: &str = "X-Proteus-API-Version";

/// Represents the simulation lifecycle states exposed by the API.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SimulationLifecycle {
    Created,
    Running,
    Paused,
}

/// Accepts a simulation-creation request from the HTTP API.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateSimulationRequest {
    pub width: u32,
    pub height: u32,
    pub seed: u64,
    #[serde(default)]
    pub r_energy: Option<f64>,
    #[serde(default)]
    pub r_mass: Option<f64>,
    #[serde(default, deserialize_with = "deserialize_optional_exponent")]
    pub d_energy_log2: Option<Option<u32>>,
    #[serde(default, deserialize_with = "deserialize_optional_exponent")]
    pub d_mass_log2: Option<Option<u32>>,
    #[serde(default)]
    pub t_cap: Option<f64>,
    #[serde(default, deserialize_with = "deserialize_optional_exponent")]
    pub maintenance_rate_log2: Option<Option<u32>>,
    #[serde(default)]
    pub maintenance_exponent: Option<f64>,
    #[serde(default)]
    pub local_action_exponent: Option<f64>,
    #[serde(default)]
    pub n_synth: Option<u32>,
    #[serde(default)]
    pub inert_grace_ticks: Option<u32>,
    #[serde(default, deserialize_with = "deserialize_optional_exponent")]
    pub p_spawn_log2: Option<Option<u32>>,
    #[serde(default)]
    pub mutation_base_log2: Option<u32>,
    #[serde(default)]
    pub mutation_background_log2: Option<u32>,
    #[serde(default)]
    pub seed_programs: Vec<SeedProgramConfig>,
    #[serde(default)]
    pub seed_environment: Vec<SeedEnvironmentConfig>,
}

fn deserialize_optional_exponent<'de, D>(deserializer: D) -> Result<Option<Option<u32>>, D::Error>
where
    D: Deserializer<'de>,
{
    Option::<u32>::deserialize(deserializer).map(Some)
}

impl CreateSimulationRequest {
    /// Fills defaults and validates a request into a resolved API config.
    pub fn resolve(self) -> Result<SimulationConfig, String> {
        let defaults = SimConfig::default();
        let config = SimulationConfig {
            width: self.width,
            height: self.height,
            seed: self.seed,
            r_energy: self.r_energy.unwrap_or(defaults.r_energy),
            r_mass: self.r_mass.unwrap_or(defaults.r_mass),
            d_energy_log2: self.d_energy_log2.unwrap_or(defaults.d_energy_log2),
            d_mass_log2: self.d_mass_log2.unwrap_or(defaults.d_mass_log2),
            t_cap: self.t_cap.unwrap_or(defaults.t_cap),
            maintenance_rate_log2: self
                .maintenance_rate_log2
                .unwrap_or(defaults.maintenance_rate_log2),
            maintenance_exponent: self
                .maintenance_exponent
                .unwrap_or(defaults.maintenance_exponent),
            local_action_exponent: self
                .local_action_exponent
                .unwrap_or(defaults.local_action_exponent),
            n_synth: self.n_synth.unwrap_or(defaults.n_synth),
            inert_grace_ticks: self.inert_grace_ticks.unwrap_or(defaults.inert_grace_ticks),
            p_spawn_log2: self.p_spawn_log2.unwrap_or(defaults.p_spawn_log2),
            mutation_base_log2: self
                .mutation_base_log2
                .unwrap_or(defaults.mutation_base_log2),
            mutation_background_log2: self
                .mutation_background_log2
                .unwrap_or(defaults.mutation_background_log2),
            seed_programs: self.seed_programs,
            seed_environment: self.seed_environment,
        };

        config.validate()?;
        Ok(config)
    }
}

/// Mirrors the API-level simulation config, including seed programs.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct SimulationConfig {
    pub width: u32,
    pub height: u32,
    pub seed: u64,
    pub r_energy: f64,
    pub r_mass: f64,
    pub d_energy_log2: Option<u32>,
    pub d_mass_log2: Option<u32>,
    pub t_cap: f64,
    pub maintenance_rate_log2: Option<u32>,
    pub maintenance_exponent: f64,
    pub local_action_exponent: f64,
    pub n_synth: u32,
    pub inert_grace_ticks: u32,
    pub p_spawn_log2: Option<u32>,
    pub mutation_base_log2: u32,
    pub mutation_background_log2: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub seed_programs: Vec<SeedProgramConfig>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub seed_environment: Vec<SeedEnvironmentConfig>,
}

impl SimulationConfig {
    /// Validates the API config and all requested seed programs.
    pub fn validate(&self) -> Result<(), String> {
        self.to_engine_config()
            .validate()
            .map_err(|err| err.to_string())?;

        self.bootstrap_config()
            .validate(&self.to_engine_config())
            .map_err(|error| error.to_string())?;

        Ok(())
    }

    /// Converts API bootstrap fields into the engine-shared bootstrap model.
    pub fn bootstrap_config(&self) -> BootstrapConfig {
        BootstrapConfig {
            programs: self.seed_programs.clone(),
            environment: self.seed_environment.clone(),
        }
    }

    /// Converts the API config into the engine config type.
    pub fn to_engine_config(&self) -> SimConfig {
        SimConfig {
            width: self.width,
            height: self.height,
            seed: self.seed,
            r_energy: self.r_energy,
            r_mass: self.r_mass,
            d_energy_log2: self.d_energy_log2,
            d_mass_log2: self.d_mass_log2,
            t_cap: self.t_cap,
            maintenance_rate_log2: self.maintenance_rate_log2,
            maintenance_exponent: self.maintenance_exponent,
            local_action_exponent: self.local_action_exponent,
            n_synth: self.n_synth,
            inert_grace_ticks: self.inert_grace_ticks,
            p_spawn_log2: self.p_spawn_log2,
            mutation_base_log2: self.mutation_base_log2,
            mutation_background_log2: self.mutation_background_log2,
        }
    }
}

/// Returns the API payload for a successful create request.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CreateSimulationResponse {
    pub status: SimulationLifecycle,
    pub tick: u64,
    pub grid_width: u32,
    pub grid_height: u32,
    pub config: SimulationConfig,
}

/// Returns the API payload for a simulation status request.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct SimulationStatusResponse {
    pub status: SimulationLifecycle,
    pub tick: u64,
    pub grid_width: u32,
    pub grid_height: u32,
    pub population: u32,
    pub total_energy: u64,
    pub total_mass: u64,
    pub ticks_per_second: f64,
}

/// Wraps API errors in the shared response envelope.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ErrorEnvelope {
    pub error: ErrorBody,
}

/// Stores the structured error body returned by the API.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ErrorBody {
    pub code: &'static str,
    pub message: String,
    pub status: u16,
}

/// Parses the optional tick count used by the step endpoint.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct StepQuery {
    pub count: Option<u64>,
}

/// Parses the optional census opt-in used by the metrics endpoint.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct MetricsQuery {
    #[serde(default, deserialize_with = "deserialize_flag")]
    pub census: Option<bool>,
}

/// Parses a query-string boolean flag.
///
/// `serde_urlencoded` only accepts the exact strings `true` and `false` for a
/// `bool`, but the documented spelling of this flag is `?census=1`. Accept the
/// usual query-string spellings so both forms work.
fn deserialize_flag<'de, D>(deserializer: D) -> Result<Option<bool>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::Error as _;

    let Some(raw) = Option::<String>::deserialize(deserializer)? else {
        return Ok(None);
    };

    match raw.as_str() {
        "1" | "true" | "yes" | "on" => Ok(Some(true)),
        "0" | "false" | "no" | "off" | "" => Ok(Some(false)),
        other => Err(D::Error::custom(format!(
            "expected a boolean flag (1/0, true/false), got {other:?}"
        ))),
    }
}

/// Parses x/y coordinates for single-cell inspection endpoints.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct CellQuery {
    pub x: u32,
    pub y: u32,
}

/// Parses the bounded rectangle requested by the region inspection endpoint.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct CellRegionQuery {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

/// Sends the initial hello frame on a new WebSocket connection.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct WsHelloMessage {
    #[serde(rename = "type")]
    pub kind: &'static str,
    pub api_version: &'static str,
}

/// Sends one metrics update over the WebSocket stream.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct WsMetricsMessage {
    #[serde(rename = "type")]
    pub kind: &'static str,
    #[serde(flatten)]
    pub metrics: MetricsSnapshot,
}

/// Sends one structured WebSocket error message.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct WsErrorMessage {
    #[serde(rename = "type")]
    pub kind: &'static str,
    pub code: &'static str,
    pub message: String,
}

/// Parses client control messages for WebSocket subscriptions.
#[derive(Clone, Debug, Deserialize)]
pub struct WsControlMessage {
    #[serde(default)]
    pub subscribe: Option<String>,
    #[serde(default)]
    pub unsubscribe: Option<String>,
    #[serde(default)]
    pub max_fps: Option<u32>,
    #[serde(default)]
    pub every_n_ticks: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::CreateSimulationRequest;

    #[test]
    fn create_request_distinguishes_absent_null_and_value_exponents() {
        let absent: CreateSimulationRequest =
            serde_json::from_str(r#"{"width":2,"height":2,"seed":1}"#).unwrap();
        assert_eq!(absent.d_energy_log2, None);
        assert_eq!(absent.p_spawn_log2, None);
        let resolved = absent.resolve().unwrap();
        assert_eq!(resolved.d_energy_log2, Some(7));
        assert_eq!(resolved.p_spawn_log2, None);

        let explicit_null: CreateSimulationRequest = serde_json::from_str(
            r#"{"width":2,"height":2,"seed":1,"d_energy_log2":null,"p_spawn_log2":null}"#,
        )
        .unwrap();
        assert_eq!(explicit_null.d_energy_log2, Some(None));
        assert_eq!(explicit_null.p_spawn_log2, Some(None));
        let resolved = explicit_null.resolve().unwrap();
        assert_eq!(resolved.d_energy_log2, None);
        assert_eq!(resolved.p_spawn_log2, None);

        let explicit_value: CreateSimulationRequest = serde_json::from_str(
            r#"{"width":2,"height":2,"seed":1,"d_energy_log2":3,"p_spawn_log2":6}"#,
        )
        .unwrap();
        assert_eq!(explicit_value.d_energy_log2, Some(Some(3)));
        assert_eq!(explicit_value.p_spawn_log2, Some(Some(6)));
        let resolved = explicit_value.resolve().unwrap();
        assert_eq!(resolved.d_energy_log2, Some(3));
        assert_eq!(resolved.p_spawn_log2, Some(6));
    }

    #[test]
    fn create_request_rejects_stale_probability_field_names() {
        let error = serde_json::from_str::<CreateSimulationRequest>(
            r#"{"width":2,"height":2,"seed":1,"d_mass":0.25,"maintenance_rate":0.5,"p_spawn":0.5}"#,
        )
        .expect_err("renamed probability fields must not be silently ignored");
        assert!(error.to_string().contains("unknown field"));
    }
}
