use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::config::{SimConfig, PROGRAM_SIZE_CAP};
use crate::observe::MetricsSnapshot;

pub const API_VERSION: &str = "0.1.0";
pub const API_VERSION_HEADER: &str = "X-Proteus-API-Version";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SimulationLifecycle {
    Created,
    Running,
    Paused,
}

#[derive(Clone, Debug, Deserialize)]
pub struct CreateSimulationRequest {
    pub width: u32,
    pub height: u32,
    pub seed: u64,
    #[serde(default)]
    pub r_energy: Option<f64>,
    #[serde(default)]
    pub r_mass: Option<f64>,
    #[serde(default)]
    pub d_energy: Option<f64>,
    #[serde(default)]
    pub d_mass: Option<f64>,
    #[serde(default)]
    pub t_cap: Option<f64>,
    #[serde(default)]
    pub maintenance_rate: Option<f64>,
    #[serde(default)]
    pub maintenance_exponent: Option<f64>,
    #[serde(default)]
    pub local_action_exponent: Option<f64>,
    #[serde(default)]
    pub n_synth: Option<u32>,
    #[serde(default)]
    pub inert_grace_ticks: Option<u32>,
    #[serde(default)]
    pub p_spawn: Option<f64>,
    #[serde(default)]
    pub mutation_base_log2: Option<u32>,
    #[serde(default)]
    pub mutation_background_log2: Option<u32>,
    #[serde(default)]
    pub seed_programs: Vec<SeedProgramConfig>,
}

impl CreateSimulationRequest {
    pub fn resolve(self) -> Result<SimulationConfig, String> {
        let defaults = SimConfig::default();
        let config = SimulationConfig {
            width: self.width,
            height: self.height,
            seed: self.seed,
            r_energy: self.r_energy.unwrap_or(defaults.r_energy),
            r_mass: self.r_mass.unwrap_or(defaults.r_mass),
            d_energy: self.d_energy.unwrap_or(defaults.d_energy),
            d_mass: self.d_mass.unwrap_or(defaults.d_mass),
            t_cap: self.t_cap.unwrap_or(defaults.t_cap),
            maintenance_rate: self.maintenance_rate.unwrap_or(defaults.maintenance_rate),
            maintenance_exponent: self
                .maintenance_exponent
                .unwrap_or(defaults.maintenance_exponent),
            local_action_exponent: self
                .local_action_exponent
                .unwrap_or(defaults.local_action_exponent),
            n_synth: self.n_synth.unwrap_or(defaults.n_synth),
            inert_grace_ticks: self.inert_grace_ticks.unwrap_or(defaults.inert_grace_ticks),
            p_spawn: self.p_spawn.unwrap_or(defaults.p_spawn),
            mutation_base_log2: self
                .mutation_base_log2
                .unwrap_or(defaults.mutation_base_log2),
            mutation_background_log2: self
                .mutation_background_log2
                .unwrap_or(defaults.mutation_background_log2),
            seed_programs: self.seed_programs,
        };

        config.validate()?;
        Ok(config)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct SimulationConfig {
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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub seed_programs: Vec<SeedProgramConfig>,
}

impl SimulationConfig {
    pub fn validate(&self) -> Result<(), String> {
        self.to_engine_config()
            .validate()
            .map_err(|err| err.to_string())?;

        let mut occupied = HashSet::new();
        for seed_program in &self.seed_programs {
            if seed_program.x >= self.width || seed_program.y >= self.height {
                return Err(format!(
                    "seed program at ({}, {}) is outside the {}x{} grid",
                    seed_program.x, seed_program.y, self.width, self.height
                ));
            }
            if seed_program.code.is_empty() {
                return Err(format!(
                    "seed program at ({}, {}) must contain at least one instruction",
                    seed_program.x, seed_program.y
                ));
            }
            if seed_program.code.len() > usize::from(PROGRAM_SIZE_CAP) {
                return Err(format!(
                    "seed program at ({}, {}) exceeds the program size cap",
                    seed_program.x, seed_program.y
                ));
            }
            if !occupied.insert((seed_program.x, seed_program.y)) {
                return Err(format!(
                    "multiple seed programs target the same cell ({}, {})",
                    seed_program.x, seed_program.y
                ));
            }
        }

        Ok(())
    }

    pub fn to_engine_config(&self) -> SimConfig {
        SimConfig {
            width: self.width,
            height: self.height,
            seed: self.seed,
            r_energy: self.r_energy,
            r_mass: self.r_mass,
            d_energy: self.d_energy,
            d_mass: self.d_mass,
            t_cap: self.t_cap,
            maintenance_rate: self.maintenance_rate,
            maintenance_exponent: self.maintenance_exponent,
            local_action_exponent: self.local_action_exponent,
            n_synth: self.n_synth,
            inert_grace_ticks: self.inert_grace_ticks,
            p_spawn: self.p_spawn,
            mutation_base_log2: self.mutation_base_log2,
            mutation_background_log2: self.mutation_background_log2,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeedProgramConfig {
    pub x: u32,
    pub y: u32,
    pub code: Vec<u8>,
    pub free_energy: u32,
    pub free_mass: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CreateSimulationResponse {
    pub status: SimulationLifecycle,
    pub tick: u64,
    pub grid_width: u32,
    pub grid_height: u32,
    pub config: SimulationConfig,
}

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

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ErrorEnvelope {
    pub error: ErrorBody,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ErrorBody {
    pub code: &'static str,
    pub message: String,
    pub status: u16,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct StepQuery {
    pub count: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct CellQuery {
    pub x: u32,
    pub y: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct CellRegionQuery {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct WsHelloMessage {
    #[serde(rename = "type")]
    pub kind: &'static str,
    pub api_version: &'static str,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct WsMetricsMessage {
    #[serde(rename = "type")]
    pub kind: &'static str,
    #[serde(flatten)]
    pub metrics: MetricsSnapshot,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct WsErrorMessage {
    #[serde(rename = "type")]
    pub kind: &'static str,
    pub code: &'static str,
    pub message: String,
}

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
