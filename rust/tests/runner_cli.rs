use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};
#[cfg(unix)]
use std::thread;
#[cfg(unix)]
use std::time::Duration;

use proteus::runner::{
    BatchManifest, BatchRunReference, BuildInfo, CompletionRecord, MetricsEnvelope,
    ObservationConfig, RunLimits, RunManifest, RunManifestRecord, RunSummary,
    RUNNER_SCHEMA_VERSION,
};
use proteus::{BootstrapConfig, EnvironmentPreload, SeedProgram, SimConfig};
use serde::Serialize;

static SANDBOX_COUNTER: AtomicU64 = AtomicU64::new(0);

struct Sandbox(PathBuf);

impl Sandbox {
    fn new(label: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "proteus-runner-{label}-{}-{}",
            std::process::id(),
            SANDBOX_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).expect("test sandbox should be unique");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn build_info_is_machine_readable_and_creates_no_artifacts() {
    let sandbox = Sandbox::new("build-info");
    let output = Command::new(run_binary())
        .arg("--build-info")
        .current_dir(sandbox.path())
        .output()
        .expect("build-info should launch");

    assert_success(&output);
    let info: BuildInfo = serde_json::from_slice(&output.stdout).expect("valid build info");
    assert_eq!(info.runner_schema_version, RUNNER_SCHEMA_VERSION);
    assert!(info.execution_digest.starts_with("sha256:"));
    assert!(info.build.source_digest.starts_with("sha256:"));
    assert_eq!(
        fs::read_dir(sandbox.path())
            .expect("sandbox should remain readable")
            .count(),
        0
    );
}

#[test]
fn direct_run_writes_tick_zero_cadence_and_forced_final_row() {
    let sandbox = Sandbox::new("direct");
    let manifest_path = sandbox.path().join("run.json");
    write_json(
        &manifest_path,
        &run_manifest("direct-run", "outputs/direct", 3, 2),
    );

    let output = run_command(&manifest_path, 1);
    assert_success(&output);

    let output_directory = sandbox.path().join("outputs/direct");
    let record: RunManifestRecord = read_json(&output_directory.join("manifest.json"));
    let summary: RunSummary = read_json(&output_directory.join("summary.json"));
    let rows = read_metrics(&output_directory.join("metrics.jsonl"));

    assert_eq!(
        rows.iter().map(|row| row.metrics.tick).collect::<Vec<_>>(),
        [0, 2, 3]
    );
    assert_eq!(record.run_id, "direct-run");
    assert_eq!(record.execution.engine_threads, 1);
    assert_eq!(summary.final_tick, 3);
    assert_eq!(summary.final_metrics, rows.last().unwrap().metrics);
    assert_eq!(summary.final_metrics.population, 1);
    assert_eq!(summary.final_metrics.event_totals.births, 0);
    assert!(!output_directory.join("completion.json").exists());
}

#[test]
fn cadence_changes_sampling_but_not_final_metrics() {
    let sandbox = Sandbox::new("cadence");
    let every_tick_path = sandbox.path().join("every-tick.json");
    let sparse_path = sandbox.path().join("sparse.json");
    write_json(
        &every_tick_path,
        &run_manifest("every-tick", "outputs/every-tick", 7, 1),
    );
    write_json(
        &sparse_path,
        &run_manifest("sparse", "outputs/sparse", 7, 100),
    );

    assert_success(&run_command(&every_tick_path, 1));
    assert_success(&run_command(&sparse_path, 1));

    let every_tick: RunSummary = read_json(&sandbox.path().join("outputs/every-tick/summary.json"));
    let sparse: RunSummary = read_json(&sandbox.path().join("outputs/sparse/summary.json"));
    assert_eq!(every_tick.final_metrics, sparse.final_metrics);
    assert_eq!(
        read_metrics(&sandbox.path().join("outputs/sparse/metrics.jsonl"))
            .iter()
            .map(|row| row.metrics.tick)
            .collect::<Vec<_>>(),
        [0, 7]
    );
}

#[test]
fn invalid_manifest_and_archive_collision_fail_before_output_creation() {
    let invalid_sandbox = Sandbox::new("invalid");
    let invalid_path = invalid_sandbox.path().join("invalid.json");
    let mut invalid =
        serde_json::to_value(run_manifest("invalid", "outputs/should-not-exist", 1, 1)).unwrap();
    invalid["unknown_field"] = serde_json::json!(true);
    write_json(&invalid_path, &invalid);

    let invalid_output = run_command(&invalid_path, 1);
    assert_eq!(invalid_output.status.code(), Some(2));
    assert!(!invalid_sandbox.path().join("outputs").exists());

    let batch_sandbox = Sandbox::new("archive-collision");
    let first_path = batch_sandbox.path().join("first.json");
    let second_path = batch_sandbox.path().join("second.json");
    write_json(&first_path, &run_manifest("first", "runs/x", 1, 1));
    write_json(
        &second_path,
        &run_manifest("second", "runs/x.attempt-0001", 1, 1),
    );
    let batch_path = batch_sandbox.path().join("batch.json");
    write_json(
        &batch_path,
        &BatchManifest {
            runner_schema_version: RUNNER_SCHEMA_VERSION.to_owned(),
            jobs: 2,
            runs: vec![
                BatchRunReference {
                    manifest: "first.json".to_owned(),
                },
                BatchRunReference {
                    manifest: "second.json".to_owned(),
                },
            ],
        },
    );

    let batch_output = batch_command(&batch_path, false);
    assert_eq!(batch_output.status.code(), Some(2));
    assert!(!batch_sandbox.path().join("runs").exists());
}

#[test]
fn batch_resume_ignores_rebuild_hash_but_retries_build_mismatch() {
    let sandbox = Sandbox::new("resume");
    let run_path = sandbox.path().join("run.json");
    let batch_path = sandbox.path().join("batch.json");
    write_json(&run_path, &run_manifest("resume-run", "runs/resume", 2, 1));
    write_json(
        &batch_path,
        &BatchManifest {
            runner_schema_version: RUNNER_SCHEMA_VERSION.to_owned(),
            jobs: 1,
            runs: vec![BatchRunReference {
                manifest: "run.json".to_owned(),
            }],
        },
    );

    assert_success(&batch_command(&batch_path, false));
    let output_directory = sandbox.path().join("runs/resume");
    let record_path = output_directory.join("manifest.json");
    let completion_path = output_directory.join("completion.json");
    let mut record: RunManifestRecord = read_json(&record_path);
    let mut completion: CompletionRecord = read_json(&completion_path);
    record.execution_digest = "sha256:rebuilt-binary".to_owned();
    completion.execution_digest = "sha256:rebuilt-binary".to_owned();
    write_json(&record_path, &record);
    write_json(&completion_path, &completion);

    assert_success(&batch_command(&batch_path, false));
    let skipped: CompletionRecord = read_json(&completion_path);
    assert_eq!(skipped.execution_digest, "sha256:rebuilt-binary");
    assert!(!sandbox.path().join("runs/resume.attempt-0001").exists());

    completion.build.source_digest = "sha256:different-source".to_owned();
    write_json(&completion_path, &completion);
    let incomplete = batch_command(&batch_path, false);
    assert_eq!(incomplete.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&incomplete.stderr).contains("is incomplete"));
    assert!(!sandbox.path().join("runs/resume.attempt-0001").exists());

    assert_success(&batch_command(&batch_path, true));
    let archived: CompletionRecord = read_json(
        &sandbox
            .path()
            .join("runs/resume.attempt-0001/completion.json"),
    );
    let retried: CompletionRecord = read_json(&completion_path);
    assert_eq!(archived.build.source_digest, "sha256:different-source");
    assert!(retried.success);
    assert_ne!(retried.build.source_digest, archived.build.source_digest);
}

#[test]
fn retry_refuses_foreign_directory_before_launching_any_run() {
    let sandbox = Sandbox::new("unsafe-output");
    let unsafe_path = sandbox.path().join("unsafe.json");
    let new_path = sandbox.path().join("new.json");
    let batch_path = sandbox.path().join("batch.json");
    write_json(&unsafe_path, &run_manifest("unsafe", "runs/unsafe", 1, 1));
    write_json(&new_path, &run_manifest("new", "runs/new", 1, 1));
    fs::create_dir_all(sandbox.path().join("runs/unsafe")).unwrap();
    fs::write(sandbox.path().join("runs/unsafe/user-data"), b"keep me").unwrap();
    write_json(
        &batch_path,
        &BatchManifest {
            runner_schema_version: RUNNER_SCHEMA_VERSION.to_owned(),
            jobs: 2,
            runs: vec![
                BatchRunReference {
                    manifest: "unsafe.json".to_owned(),
                },
                BatchRunReference {
                    manifest: "new.json".to_owned(),
                },
            ],
        },
    );

    let output = batch_command(&batch_path, true);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        fs::read(sandbox.path().join("runs/unsafe/user-data")).unwrap(),
        b"keep me"
    );
    assert!(!sandbox.path().join("runs/new").exists());
    assert!(!sandbox.path().join("runs/unsafe.attempt-0001").exists());
}

#[cfg(unix)]
#[test]
fn interrupting_batch_reaps_the_child_and_records_supervisor_interruption() {
    let sandbox = Sandbox::new("interrupt");
    let run_path = sandbox.path().join("run.json");
    let batch_path = sandbox.path().join("batch.json");
    write_json(
        &run_path,
        &run_manifest("interrupted-run", "runs/interrupted", 1_000_000_000, 10_000),
    );
    write_json(
        &batch_path,
        &BatchManifest {
            runner_schema_version: RUNNER_SCHEMA_VERSION.to_owned(),
            jobs: 1,
            runs: vec![BatchRunReference {
                manifest: "run.json".to_owned(),
            }],
        },
    );

    let batch = Command::new(batch_binary())
        .args(["--manifest", batch_path.to_str().unwrap()])
        .output_spawn()
        .expect("proteus-batch should launch");
    let batch_pid = batch.id();
    let output_directory = sandbox.path().join("runs/interrupted");
    for _ in 0..500 {
        if output_directory.join("manifest.json").exists() {
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }
    if !output_directory.join("manifest.json").exists() {
        let _ = Command::new("kill")
            .args(["-INT", &batch_pid.to_string()])
            .status();
        let output = batch
            .wait_with_output()
            .expect("timed-out batch should exit after SIGINT");
        panic!(
            "batch did not reserve the run within five seconds; stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let signal_status = Command::new("kill")
        .args(["-INT", &batch_pid.to_string()])
        .status()
        .expect("SIGINT command should launch");
    assert!(signal_status.success());
    let output = batch
        .wait_with_output()
        .expect("interrupted batch should exit");
    assert_eq!(output.status.code(), Some(130));

    let completion: CompletionRecord = read_json(&output_directory.join("completion.json"));
    assert!(!completion.success);
    assert_eq!(
        completion.category,
        proteus::runner::CompletionCategory::SupervisorInterrupted
    );
    assert!(!completion.valid_summary);
}

#[cfg(feature = "rayon")]
#[test]
fn direct_runner_thread_counts_preserve_final_metrics_and_input_identity() {
    let sandbox = Sandbox::new("threads");
    let serial_path = sandbox.path().join("serial.json");
    let parallel_path = sandbox.path().join("parallel.json");
    write_json(
        &serial_path,
        &run_manifest("serial", "outputs/serial", 20, 4),
    );
    write_json(
        &parallel_path,
        &run_manifest("parallel", "outputs/parallel", 20, 4),
    );

    assert_success(&run_command(&serial_path, 1));
    assert_success(&run_command(&parallel_path, 4));

    let serial_record: RunManifestRecord =
        read_json(&sandbox.path().join("outputs/serial/manifest.json"));
    let parallel_record: RunManifestRecord =
        read_json(&sandbox.path().join("outputs/parallel/manifest.json"));
    let serial: RunSummary = read_json(&sandbox.path().join("outputs/serial/summary.json"));
    let parallel: RunSummary = read_json(&sandbox.path().join("outputs/parallel/summary.json"));
    assert_eq!(serial_record.input_digest, parallel_record.input_digest);
    assert_eq!(serial.final_metrics, parallel.final_metrics);
    assert_eq!(parallel_record.execution.engine_threads, 4);
}

fn run_manifest(
    run_id: &str,
    output_directory: &str,
    ticks: u64,
    every_n_ticks: u64,
) -> RunManifest {
    RunManifest {
        runner_schema_version: RUNNER_SCHEMA_VERSION.to_owned(),
        run_id: run_id.to_owned(),
        simulation: SimConfig {
            width: 2,
            height: 2,
            seed: 42,
            r_energy: 0.0,
            r_mass: 0.0,
            d_energy: 0.0,
            d_mass: 0.0,
            t_cap: 4.0,
            maintenance_rate: 0.0,
            maintenance_exponent: 1.0,
            local_action_exponent: 1.0,
            n_synth: 1,
            inert_grace_ticks: 10,
            p_spawn: 0.0,
            mutation_base_log2: 16,
            mutation_background_log2: 8,
        },
        bootstrap: BootstrapConfig {
            programs: vec![SeedProgram {
                x: 0,
                y: 0,
                code: vec![0],
                free_energy: 20,
                free_mass: 12,
            }],
            environment: vec![EnvironmentPreload {
                x: 1,
                y: 0,
                free_energy: 3,
                free_mass: 4,
                bg_radiation: 5,
                bg_mass: 6,
            }],
        },
        limits: RunLimits { ticks },
        observation: ObservationConfig { every_n_ticks },
        output_directory: output_directory.to_owned(),
    }
}

fn run_command(manifest: &Path, threads: u32) -> Output {
    Command::new(run_binary())
        .args([
            "--manifest",
            manifest.to_str().unwrap(),
            "--threads",
            &threads.to_string(),
        ])
        .output()
        .expect("proteus-run should launch")
}

fn batch_command(manifest: &Path, retry_incomplete: bool) -> Output {
    let mut command = Command::new(batch_binary());
    command.args(["--manifest", manifest.to_str().unwrap()]);
    if retry_incomplete {
        command.arg("--retry-incomplete");
    }
    command.output().expect("proteus-batch should launch")
}

fn run_binary() -> &'static str {
    env!("CARGO_BIN_EXE_proteus-run")
}

fn batch_binary() -> &'static str {
    env!("CARGO_BIN_EXE_proteus-batch")
}

fn write_json(path: &Path, value: &impl Serialize) {
    fs::write(path, serde_json::to_vec_pretty(value).unwrap()).expect("test JSON should write");
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> T {
    serde_json::from_slice(&fs::read(path).expect("JSON file should exist"))
        .expect("JSON file should be valid")
}

fn read_metrics(path: &Path) -> Vec<MetricsEnvelope> {
    fs::read_to_string(path)
        .expect("metrics should exist")
        .lines()
        .map(|line| serde_json::from_str(line).expect("metrics line should be valid"))
        .collect()
}

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "command failed with {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[cfg(unix)]
trait OutputSpawn {
    fn output_spawn(&mut self) -> std::io::Result<std::process::Child>;
}

#[cfg(unix)]
impl OutputSpawn for Command {
    fn output_spawn(&mut self) -> std::io::Result<std::process::Child> {
        use std::process::Stdio;

        self.stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()
    }
}
