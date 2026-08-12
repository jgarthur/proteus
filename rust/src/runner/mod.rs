//! Shared manifest, provenance, output, and filesystem contracts for headless runs.

mod batch;
mod single;

use std::error::Error;
use std::fmt;
use std::fmt::Write as _;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::bootstrap::{BootstrapConfig, BOOTSTRAP_RNG_SALT, BOOTSTRAP_RNG_VERSION};
use crate::config::{SimConfig, SPEC_VERSION};
use crate::observe::MetricsSnapshot;

pub use batch::{batch_main, BatchOptions, BatchOutcome};
pub use single::{run_main, RunOptions};

/// Identifies the first stable headless-runner contract.
pub const RUNNER_SCHEMA_VERSION: &str = "0.1.0";

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Exact single-run manifest accepted from callers.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunManifest {
    pub runner_schema_version: String,
    pub run_id: String,
    pub simulation: SimConfig,
    pub bootstrap: BootstrapConfig,
    pub limits: RunLimits,
    pub observation: ObservationConfig,
    pub output_directory: String,
}

/// Fixed tick limit for one run.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunLimits {
    pub ticks: u64,
}

/// Fixed observation cadence for one run.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObservationConfig {
    pub every_n_ticks: u64,
}

/// Exact batch manifest accepted from callers.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BatchManifest {
    pub runner_schema_version: String,
    pub jobs: u32,
    pub runs: Vec<BatchRunReference>,
}

/// References one external single-run manifest from a batch.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BatchRunReference {
    pub manifest: String,
}

/// Fully validated run plus resolved storage paths and canonical identity.
#[derive(Clone, Debug)]
pub struct ResolvedRun {
    pub manifest: RunManifest,
    pub source_manifest: PathBuf,
    pub output_directory: PathBuf,
    pub input_digest: String,
}

impl ResolvedRun {
    /// Returns the normalized input persisted in `manifest.json`.
    pub fn normalized_input(&self) -> Result<NormalizedRunInput, RunnerError> {
        Ok(NormalizedRunInput {
            simulation: self.manifest.simulation.clone(),
            bootstrap: self.manifest.bootstrap.normalized(),
            limits: self.manifest.limits,
            observation: self.manifest.observation,
            output_directory: path_string(&self.output_directory)?,
        })
    }
}

/// Normalized run input stored with provenance.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NormalizedRunInput {
    pub simulation: SimConfig,
    pub bootstrap: BootstrapConfig,
    pub limits: RunLimits,
    pub observation: ObservationConfig,
    pub output_directory: String,
}

/// Rebuild-stable source and compiler identity shared by both binaries.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BuildProvenance {
    pub engine_crate_version: String,
    pub simulator_spec_version: String,
    pub git_commit: Option<String>,
    pub source_dirty: Option<bool>,
    pub source_digest: String,
    pub cargo_profile: String,
    pub cargo_features: Vec<String>,
    pub target_triple: String,
    pub rustc_version: String,
}

/// Machine-readable metadata emitted by `proteus-run --build-info`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BuildInfo {
    pub runner_schema_version: String,
    pub execution_digest: String,
    pub build: BuildProvenance,
    pub rayon_available: bool,
}

/// Execution-specific metadata stored in a run manifest record.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionProvenance {
    pub bootstrap_rng_version: String,
    pub bootstrap_rng_salt: String,
    pub engine_threads: u32,
}

/// Launch metadata known before tick 0.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LaunchProvenance {
    pub executable: String,
    pub source_manifest: String,
    pub started_at: String,
}

/// Immutable normalized input and provenance record.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunManifestRecord {
    pub runner_schema_version: String,
    pub run_id: String,
    pub input_digest: String,
    pub execution_digest: String,
    pub input: NormalizedRunInput,
    pub execution: ExecutionProvenance,
    pub build: BuildProvenance,
    pub launch: LaunchProvenance,
}

/// Self-identifying sampled metrics line.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MetricsEnvelope {
    pub runner_schema_version: String,
    pub run_id: String,
    pub input_digest: String,
    pub execution_digest: String,
    pub metrics: MetricsSnapshot,
}

/// Final child-owned successful-run summary.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunSummary {
    pub runner_schema_version: String,
    pub run_id: String,
    pub input_digest: String,
    pub execution_digest: String,
    pub build: BuildProvenance,
    pub final_tick: u64,
    pub termination_reason: String,
    pub started_at: String,
    pub finished_at: String,
    pub wall_duration_ms: u64,
    pub final_metrics: MetricsSnapshot,
}

/// Supervisor-owned portable outcome category.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompletionCategory {
    Succeeded,
    ChildFailed,
    Signaled,
    LaunchFailed,
    InvalidChildOutput,
    SupervisorInterrupted,
}

/// Absolute artifact paths stored in a completion record.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactPaths {
    pub manifest: String,
    pub metrics: String,
    pub summary: String,
    pub stdout: String,
    pub stderr: String,
}

/// Authoritative supervisor-owned run completion record.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompletionRecord {
    pub runner_schema_version: String,
    pub run_id: String,
    pub input_digest: String,
    pub execution_digest: String,
    pub build: BuildProvenance,
    pub success: bool,
    pub category: CompletionCategory,
    pub child_exit_code: Option<i32>,
    pub child_signal: Option<String>,
    pub valid_summary: bool,
    pub started_at: String,
    pub finished_at: String,
    pub wall_duration_ms: u64,
    pub artifacts: ArtifactPaths,
}

/// Error classification used by the CLI exit-code boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorKind {
    InvalidInput,
    Operational,
}

/// Human-readable runner failure with a stable CLI classification.
#[derive(Debug)]
pub struct RunnerError {
    kind: ErrorKind,
    message: String,
}

impl RunnerError {
    pub fn invalid(message: impl Into<String>) -> Self {
        Self {
            kind: ErrorKind::InvalidInput,
            message: message.into(),
        }
    }

    pub fn operational(message: impl Into<String>) -> Self {
        Self {
            kind: ErrorKind::Operational,
            message: message.into(),
        }
    }

    pub fn kind(&self) -> ErrorKind {
        self.kind
    }
}

impl fmt::Display for RunnerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for RunnerError {}

/// Reads, strictly validates, normalizes, and identifies a run manifest.
pub fn load_run_manifest(path: &Path) -> Result<ResolvedRun, RunnerError> {
    let source_manifest = canonical_file(path, "run manifest")?;
    let bytes = fs::read(&source_manifest).map_err(|error| {
        RunnerError::invalid(format!(
            "failed to read run manifest {}: {error}",
            source_manifest.display()
        ))
    })?;
    let manifest: RunManifest = serde_json::from_slice(&bytes).map_err(|error| {
        RunnerError::invalid(format!(
            "invalid run manifest {}: {error}",
            source_manifest.display()
        ))
    })?;
    validate_run_manifest(&manifest)?;
    let output_directory = resolve_output_path(
        source_manifest
            .parent()
            .expect("canonical manifest should have a parent"),
        Path::new(&manifest.output_directory),
    )?;
    let input_digest = input_digest(&manifest)?;
    Ok(ResolvedRun {
        manifest,
        source_manifest,
        output_directory,
        input_digest,
    })
}

/// Reads and strictly validates a batch manifest.
pub fn load_batch_manifest(path: &Path) -> Result<(BatchManifest, PathBuf), RunnerError> {
    let source_manifest = canonical_file(path, "batch manifest")?;
    let bytes = fs::read(&source_manifest).map_err(|error| {
        RunnerError::invalid(format!(
            "failed to read batch manifest {}: {error}",
            source_manifest.display()
        ))
    })?;
    let manifest: BatchManifest = serde_json::from_slice(&bytes).map_err(|error| {
        RunnerError::invalid(format!(
            "invalid batch manifest {}: {error}",
            source_manifest.display()
        ))
    })?;
    if manifest.runner_schema_version != RUNNER_SCHEMA_VERSION {
        return Err(RunnerError::invalid(format!(
            "unsupported runner schema {}, expected {RUNNER_SCHEMA_VERSION}",
            manifest.runner_schema_version
        )));
    }
    if manifest.jobs == 0 {
        return Err(RunnerError::invalid("batch jobs must be greater than zero"));
    }
    if manifest.runs.is_empty() {
        return Err(RunnerError::invalid("batch runs must not be empty"));
    }
    if manifest.runs.iter().any(|entry| entry.manifest.is_empty()) {
        return Err(RunnerError::invalid(
            "batch manifest paths must not be empty",
        ));
    }
    Ok((manifest, source_manifest))
}

fn validate_run_manifest(manifest: &RunManifest) -> Result<(), RunnerError> {
    if manifest.runner_schema_version != RUNNER_SCHEMA_VERSION {
        return Err(RunnerError::invalid(format!(
            "unsupported runner schema {}, expected {RUNNER_SCHEMA_VERSION}",
            manifest.runner_schema_version
        )));
    }
    if !valid_run_id(&manifest.run_id) {
        return Err(RunnerError::invalid(
            "run_id must match [A-Za-z0-9][A-Za-z0-9._-]{0,127}",
        ));
    }
    manifest
        .simulation
        .validate()
        .map_err(|error| RunnerError::invalid(error.to_string()))?;
    manifest
        .bootstrap
        .validate(&manifest.simulation)
        .map_err(|error| RunnerError::invalid(error.to_string()))?;
    if manifest.limits.ticks == 0 {
        return Err(RunnerError::invalid(
            "limits.ticks must be greater than zero",
        ));
    }
    if manifest.observation.every_n_ticks == 0 {
        return Err(RunnerError::invalid(
            "observation.every_n_ticks must be greater than zero",
        ));
    }
    if manifest.output_directory.is_empty() {
        return Err(RunnerError::invalid("output_directory must not be empty"));
    }
    Ok(())
}

fn valid_run_id(run_id: &str) -> bool {
    let bytes = run_id.as_bytes();
    !bytes.is_empty()
        && bytes.len() <= 128
        && bytes[0].is_ascii_alphanumeric()
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

/// Computes the schema `0.1.0` golden-compatible semantic input digest.
pub fn input_digest(manifest: &RunManifest) -> Result<String, RunnerError> {
    let bootstrap = manifest.bootstrap.normalized();
    let config = &manifest.simulation;
    let mut canonical = String::new();
    canonical.push_str("{\"runner_schema_version\":\"");
    canonical.push_str(RUNNER_SCHEMA_VERSION);
    canonical.push_str("\",\"simulation\":{");
    write!(
        canonical,
        "\"width\":{},\"height\":{},\"seed\":{}",
        config.width, config.height, config.seed
    )
    .expect("writing to a String should not fail");
    write_float(&mut canonical, "r_energy", config.r_energy);
    write_float(&mut canonical, "r_mass", config.r_mass);
    write_float(&mut canonical, "d_energy", config.d_energy);
    write_float(&mut canonical, "d_mass", config.d_mass);
    write_float(&mut canonical, "t_cap", config.t_cap);
    write_float(&mut canonical, "maintenance_rate", config.maintenance_rate);
    write_float(
        &mut canonical,
        "maintenance_exponent",
        config.maintenance_exponent,
    );
    write_float(
        &mut canonical,
        "local_action_exponent",
        config.local_action_exponent,
    );
    write!(
        canonical,
        ",\"n_synth\":{},\"inert_grace_ticks\":{}",
        config.n_synth, config.inert_grace_ticks
    )
    .expect("writing to a String should not fail");
    write_float(&mut canonical, "p_spawn", config.p_spawn);
    write!(
        canonical,
        ",\"mutation_base_log2\":{},\"mutation_background_log2\":{}",
        config.mutation_base_log2, config.mutation_background_log2
    )
    .expect("writing to a String should not fail");
    canonical.push_str("},\"bootstrap\":{\"programs\":[");
    for (position, program) in bootstrap.programs.iter().enumerate() {
        if position != 0 {
            canonical.push(',');
        }
        write!(
            canonical,
            "{{\"x\":{},\"y\":{},\"code\":[",
            program.x, program.y
        )
        .expect("writing to a String should not fail");
        for (code_position, byte) in program.code.iter().enumerate() {
            if code_position != 0 {
                canonical.push(',');
            }
            write!(canonical, "{byte}").expect("writing to a String should not fail");
        }
        write!(
            canonical,
            "],\"free_energy\":{},\"free_mass\":{}}}",
            program.free_energy, program.free_mass
        )
        .expect("writing to a String should not fail");
    }
    canonical.push_str("],\"environment\":[");
    for (position, preload) in bootstrap.environment.iter().enumerate() {
        if position != 0 {
            canonical.push(',');
        }
        write!(
            canonical,
            "{{\"x\":{},\"y\":{},\"free_energy\":{},\"free_mass\":{},\"bg_radiation\":{},\"bg_mass\":{}}}",
            preload.x,
            preload.y,
            preload.free_energy,
            preload.free_mass,
            preload.bg_radiation,
            preload.bg_mass
        )
        .expect("writing to a String should not fail");
    }
    write!(
        canonical,
        "]}},\"limits\":{{\"ticks\":{}}},\"observation\":{{\"every_n_ticks\":{}}}}}",
        manifest.limits.ticks, manifest.observation.every_n_ticks
    )
    .expect("writing to a String should not fail");

    Ok(sha256_bytes(canonical.as_bytes()))
}

fn write_float(output: &mut String, field: &str, value: f64) {
    let mut bits = value.to_bits();
    if bits == (-0.0_f64).to_bits() {
        bits = 0;
    }
    write!(output, ",\"{field}\":\"0x{bits:016x}\"").expect("writing to a String should not fail");
}

/// Returns compile-time build identity shared by runner binaries.
pub fn build_provenance() -> BuildProvenance {
    BuildProvenance {
        engine_crate_version: env!("CARGO_PKG_VERSION").to_owned(),
        simulator_spec_version: SPEC_VERSION.to_owned(),
        git_commit: nonempty(option_env!("PROTEUS_GIT_COMMIT").unwrap_or("")),
        source_dirty: match option_env!("PROTEUS_SOURCE_DIRTY") {
            Some("true") => Some(true),
            Some("false") => Some(false),
            _ => None,
        },
        source_digest: env!("PROTEUS_SOURCE_DIGEST").to_owned(),
        cargo_profile: env!("PROTEUS_CARGO_PROFILE").to_owned(),
        cargo_features: option_env!("PROTEUS_CARGO_FEATURES")
            .unwrap_or("")
            .split(',')
            .filter(|feature| !feature.is_empty())
            .map(str::to_owned)
            .collect(),
        target_triple: env!("PROTEUS_TARGET_TRIPLE").to_owned(),
        rustc_version: env!("PROTEUS_RUSTC_VERSION").to_owned(),
    }
}

fn nonempty(value: &str) -> Option<String> {
    (!value.is_empty()).then(|| value.to_owned())
}

/// Hashes an executable for forensic provenance.
pub fn executable_digest(path: &Path) -> Result<String, RunnerError> {
    let bytes = fs::read(path).map_err(|error| {
        RunnerError::operational(format!(
            "failed to read executable {}: {error}",
            path.display()
        ))
    })?;
    Ok(sha256_bytes(&bytes))
}

/// Returns metadata for the currently running executable.
pub fn current_build_info() -> Result<BuildInfo, RunnerError> {
    let executable = std::env::current_exe().map_err(|error| {
        RunnerError::operational(format!("failed to resolve current executable: {error}"))
    })?;
    Ok(BuildInfo {
        runner_schema_version: RUNNER_SCHEMA_VERSION.to_owned(),
        execution_digest: executable_digest(&executable)?,
        build: build_provenance(),
        rayon_available: cfg!(feature = "rayon"),
    })
}

pub(crate) fn run_manifest_record(
    run: &ResolvedRun,
    build_info: &BuildInfo,
    executable: &Path,
    started_at: String,
    engine_threads: u32,
) -> Result<RunManifestRecord, RunnerError> {
    Ok(RunManifestRecord {
        runner_schema_version: RUNNER_SCHEMA_VERSION.to_owned(),
        run_id: run.manifest.run_id.clone(),
        input_digest: run.input_digest.clone(),
        execution_digest: build_info.execution_digest.clone(),
        input: run.normalized_input()?,
        execution: ExecutionProvenance {
            bootstrap_rng_version: BOOTSTRAP_RNG_VERSION.to_owned(),
            bootstrap_rng_salt: format!("0x{BOOTSTRAP_RNG_SALT:016x}"),
            engine_threads,
        },
        build: build_info.build.clone(),
        launch: LaunchProvenance {
            executable: path_string(executable)?,
            source_manifest: path_string(&run.source_manifest)?,
            started_at,
        },
    })
}

pub(crate) fn artifact_paths(output: &Path) -> Result<ArtifactPaths, RunnerError> {
    Ok(ArtifactPaths {
        manifest: path_string(&output.join("manifest.json"))?,
        metrics: path_string(&output.join("metrics.jsonl"))?,
        summary: path_string(&output.join("summary.json"))?,
        stdout: path_string(&output.join("stdout.log"))?,
        stderr: path_string(&output.join("stderr.log"))?,
    })
}

pub(crate) fn read_json<T: for<'de> Deserialize<'de>>(
    path: &Path,
    description: &str,
) -> Result<T, RunnerError> {
    let bytes = fs::read(path).map_err(|error| {
        RunnerError::operational(format!(
            "failed to read {description} {}: {error}",
            path.display()
        ))
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        RunnerError::operational(format!("invalid {description} {}: {error}", path.display()))
    })
}

pub(crate) fn write_json_atomic<T: Serialize>(path: &Path, value: &T) -> Result<(), RunnerError> {
    let parent = path.parent().ok_or_else(|| {
        RunnerError::operational(format!("output path has no parent: {}", path.display()))
    })?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            RunnerError::operational(format!("output filename is not UTF-8: {}", path.display()))
        })?;
    let temporary = parent.join(format!(
        ".{file_name}.tmp-{}-{}",
        std::process::id(),
        TEMP_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    let result = (|| -> Result<(), RunnerError> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(|error| io_error("create temporary output", &temporary, error))?;
        serde_json::to_writer_pretty(&mut file, value).map_err(|error| {
            RunnerError::operational(format!("failed to encode {}: {error}", path.display()))
        })?;
        file.write_all(b"\n")
            .map_err(|error| io_error("write output", &temporary, error))?;
        file.flush()
            .map_err(|error| io_error("flush output", &temporary, error))?;
        file.sync_all()
            .map_err(|error| io_error("sync output", &temporary, error))?;
        fs::rename(&temporary, path).map_err(|error| io_error("commit output", path, error))?;
        sync_directory(parent);
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

pub(crate) fn create_new_file(path: &Path) -> Result<File, RunnerError> {
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| io_error("create output", path, error))
}

pub(crate) fn sync_directory(path: &Path) {
    if let Ok(directory) = File::open(path) {
        let _ = directory.sync_all();
    }
}

fn io_error(action: &str, path: &Path, error: io::Error) -> RunnerError {
    RunnerError::operational(format!("failed to {action} {}: {error}", path.display()))
}

/// Returns a current RFC 3339 UTC timestamp with millisecond precision.
pub fn timestamp_now() -> Result<String, RunnerError> {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| {
            RunnerError::operational(format!("system clock predates Unix epoch: {error}"))
        })?;
    let seconds = i64::try_from(duration.as_secs())
        .map_err(|_| RunnerError::operational("system time exceeds supported range"))?;
    let days = seconds / 86_400;
    let seconds_of_day = seconds % 86_400;
    let (year, month, day) = civil_from_days(days);
    let hour = seconds_of_day / 3_600;
    let minute = (seconds_of_day % 3_600) / 60;
    let second = seconds_of_day % 60;
    Ok(format!(
        "{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}.{:03}Z",
        duration.subsec_millis()
    ))
}

fn civil_from_days(days_since_epoch: i64) -> (i64, i64, i64) {
    let z = days_since_epoch + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    (year, month, day)
}

pub(crate) fn canonical_file(path: &Path, description: &str) -> Result<PathBuf, RunnerError> {
    let canonical = fs::canonicalize(path).map_err(|error| {
        RunnerError::invalid(format!(
            "failed to resolve {description} {}: {error}",
            path.display()
        ))
    })?;
    if !canonical.is_file() {
        return Err(RunnerError::invalid(format!(
            "{description} is not a file: {}",
            canonical.display()
        )));
    }
    path_string(&canonical)?;
    Ok(canonical)
}

pub(crate) fn resolve_output_path(base: &Path, requested: &Path) -> Result<PathBuf, RunnerError> {
    let joined = if requested.is_absolute() {
        requested.to_path_buf()
    } else {
        base.join(requested)
    };
    let lexical = lexical_normalize(&joined)?;
    let mut ancestor = lexical.as_path();
    let mut suffix = Vec::new();
    while !ancestor.exists() {
        let name = ancestor.file_name().ok_or_else(|| {
            RunnerError::invalid(format!("cannot resolve output path {}", joined.display()))
        })?;
        suffix.push(name.to_os_string());
        ancestor = ancestor.parent().ok_or_else(|| {
            RunnerError::invalid(format!("cannot resolve output path {}", joined.display()))
        })?;
    }
    let mut resolved = fs::canonicalize(ancestor).map_err(|error| {
        RunnerError::invalid(format!(
            "failed to resolve output ancestor {}: {error}",
            ancestor.display()
        ))
    })?;
    for component in suffix.iter().rev() {
        resolved.push(component);
    }
    path_string(&resolved)?;
    Ok(resolved)
}

fn lexical_normalize(path: &Path) -> Result<PathBuf, RunnerError> {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(_) | Component::RootDir | Component::Normal(_) => {
                normalized.push(component.as_os_str());
            }
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalized.pop() {
                    return Err(RunnerError::invalid(format!(
                        "path escapes filesystem root: {}",
                        path.display()
                    )));
                }
            }
        }
    }
    Ok(normalized)
}

pub(crate) fn path_string(path: &Path) -> Result<String, RunnerError> {
    path.to_str()
        .map(str::to_owned)
        .ok_or_else(|| RunnerError::invalid(format!("path is not UTF-8: {}", path.display())))
}

pub(crate) fn sha256_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("sha256:{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{input_digest, timestamp_now, RunManifest};

    #[test]
    fn golden_manifest_digest_is_stable() {
        let manifest: RunManifest = serde_json::from_str(
            r#"{
              "runner_schema_version":"0.1.0",
              "run_id":"example-run-0001",
              "simulation":{"width":64,"height":64,"seed":42,"r_energy":0.25,"r_mass":0.05,"d_energy":0.01,"d_mass":0.01,"t_cap":4.0,"maintenance_rate":0.0078125,"maintenance_exponent":1.0,"local_action_exponent":1.0,"n_synth":1,"inert_grace_ticks":10,"p_spawn":0.0,"mutation_base_log2":16,"mutation_background_log2":8},
              "bootstrap":{"programs":[{"x":32,"y":24,"code":[80,100],"free_energy":20,"free_mass":12}],"environment":[{"x":31,"y":24,"free_energy":20,"free_mass":12,"bg_radiation":0,"bg_mass":0}]},
              "limits":{"ticks":10000},"observation":{"every_n_ticks":50},"output_directory":"runs/example-run-0001"
            }"#,
        )
        .expect("golden manifest should parse");
        assert_eq!(
            input_digest(&manifest).expect("digest should build"),
            "sha256:e5053c7b811503c8d875fde5bb1a85d6fbbfb15596d591c300fae351cb11ac03"
        );
    }

    #[test]
    fn digest_normalizes_float_spelling_bootstrap_order_and_storage_identity() {
        let json = r#"{
          "runner_schema_version":"0.1.0",
          "run_id":"first-name",
          "simulation":{"width":2,"height":2,"seed":42,"r_energy":2.5e-1,"r_mass":5e-2,"d_energy":1e-2,"d_mass":0.010,"t_cap":4e0,"maintenance_rate":7.8125e-3,"maintenance_exponent":1,"local_action_exponent":1.0,"n_synth":1,"inert_grace_ticks":10,"p_spawn":-0.0,"mutation_base_log2":16,"mutation_background_log2":8},
          "bootstrap":{"programs":[{"x":1,"y":1,"code":[80],"free_energy":20,"free_mass":12},{"x":0,"y":0,"code":[100],"free_energy":4,"free_mass":3}],"environment":[{"x":0,"y":1,"free_energy":1,"free_mass":2,"bg_radiation":3,"bg_mass":4},{"x":1,"y":0,"free_energy":5,"free_mass":6,"bg_radiation":7,"bg_mass":8}]},
          "limits":{"ticks":10},"observation":{"every_n_ticks":2},"output_directory":"runs/first"
        }"#;
        let mut first: RunManifest = serde_json::from_str(json).expect("manifest should parse");
        let first_digest = input_digest(&first).expect("digest should build");

        first.run_id = "different-name".to_owned();
        first.output_directory = "somewhere/else".to_owned();
        first.bootstrap.programs.reverse();
        first.bootstrap.environment.reverse();
        let normalized_json = serde_json::to_string(&first).expect("manifest should serialize");
        let round_tripped: RunManifest =
            serde_json::from_str(&normalized_json).expect("manifest should round trip");

        assert_eq!(first_digest, input_digest(&first).unwrap());
        assert_eq!(first_digest, input_digest(&round_tripped).unwrap());
    }

    #[test]
    fn timestamp_has_rfc3339_utc_shape() {
        let timestamp = timestamp_now().expect("timestamp should build");
        assert_eq!(timestamp.len(), 24);
        assert_eq!(&timestamp[4..5], "-");
        assert_eq!(&timestamp[10..11], "T");
        assert!(timestamp.ends_with('Z'));
    }
}
