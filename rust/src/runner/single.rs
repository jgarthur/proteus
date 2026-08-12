use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;
use std::time::Instant;

use crate::observe::{collect_metrics, EventTotals};
use crate::{apply_bootstrap, Simulation, TickReport};

use super::{
    build_provenance, create_new_file, current_build_info, read_json, run_manifest_record,
    sync_directory, timestamp_now, write_json_atomic, BuildInfo, MetricsEnvelope, ResolvedRun,
    RunManifestRecord, RunSummary, RunnerError, RUNNER_SCHEMA_VERSION,
};

/// Parsed `proteus-run` execution options.
#[derive(Clone, Debug)]
pub struct RunOptions {
    pub manifest_path: PathBuf,
    pub threads: u32,
    pub verbosity: u8,
    pub supervised: bool,
}

/// Executes one fully specified headless simulation.
pub fn run_main(options: RunOptions) -> Result<(), RunnerError> {
    if options.threads == 0 {
        return Err(RunnerError::invalid("--threads must be greater than zero"));
    }
    if options.verbosity > 1 {
        return Err(RunnerError::invalid(
            "--verbosity/-v requires either 0 or 1",
        ));
    }
    configure_threads(options.threads)?;
    let run = super::load_run_manifest(&options.manifest_path)?;
    let build_info = current_build_info()?;
    let executable = std::env::current_exe().map_err(|error| {
        RunnerError::operational(format!("failed to resolve current executable: {error}"))
    })?;
    let started_at = timestamp_now()?;
    let started = Instant::now();

    let record = if options.supervised {
        verify_supervised_reservation(&run, &build_info, options.threads)?
    } else {
        reserve_direct_output(&run, &build_info, &executable, &started_at, options.threads)?
    };
    if options.verbosity > 0 {
        eprintln!(
            "proteus-run: starting run {} (ticks={}, observe_every={}, threads={})",
            run.manifest.run_id,
            run.manifest.limits.ticks,
            run.manifest.observation.every_n_ticks,
            options.threads
        );
        eprintln!(
            "proteus-run: output directory: {}",
            run.output_directory.display()
        );
    }

    let metrics_path = run.output_directory.join("metrics.jsonl");
    let mut metrics_file = create_new_file(&metrics_path)?;
    let mut simulation = Simulation::new(run.manifest.simulation.clone()).map_err(|error| {
        RunnerError::operational(format!("failed to create simulation: {error}"))
    })?;
    apply_bootstrap(&mut simulation, &run.manifest.bootstrap)
        .map_err(|error| RunnerError::invalid(error.to_string()))?;

    let mut event_totals = EventTotals::default();
    let mut last_report = TickReport::default();
    let mut latest_metrics = collect_metrics(
        simulation.grid(),
        0,
        simulation.tick(),
        last_report,
        event_totals,
    );
    write_metrics(&mut metrics_file, &record, latest_metrics.clone())?;
    let mut last_written_tick = 0;

    while simulation.tick() < run.manifest.limits.ticks {
        last_report = simulation.run_tick_report();
        event_totals.record(last_report);
        if simulation.tick() % run.manifest.observation.every_n_ticks == 0 {
            latest_metrics = collect_metrics(
                simulation.grid(),
                0,
                simulation.tick(),
                last_report,
                event_totals,
            );
            write_metrics(&mut metrics_file, &record, latest_metrics.clone())?;
            last_written_tick = simulation.tick();
        }
    }

    if last_written_tick != simulation.tick() {
        latest_metrics = collect_metrics(
            simulation.grid(),
            0,
            simulation.tick(),
            last_report,
            event_totals,
        );
        write_metrics(&mut metrics_file, &record, latest_metrics.clone())?;
    }
    metrics_file.flush().map_err(|error| {
        RunnerError::operational(format!(
            "failed to flush {}: {error}",
            metrics_path.display()
        ))
    })?;
    metrics_file.sync_all().map_err(|error| {
        RunnerError::operational(format!(
            "failed to sync {}: {error}",
            metrics_path.display()
        ))
    })?;

    let summary = RunSummary {
        runner_schema_version: RUNNER_SCHEMA_VERSION.to_owned(),
        run_id: run.manifest.run_id.clone(),
        input_digest: run.input_digest.clone(),
        execution_digest: build_info.execution_digest,
        build: build_provenance(),
        final_tick: simulation.tick(),
        termination_reason: "tick_limit_reached".to_owned(),
        started_at,
        finished_at: timestamp_now()?,
        wall_duration_ms: duration_millis(started.elapsed()),
        final_metrics: latest_metrics,
    };
    write_json_atomic(&run.output_directory.join("summary.json"), &summary)?;
    sync_directory(&run.output_directory);
    if options.verbosity > 0 {
        eprintln!(
            "proteus-run: completed run {} (final_tick={}, duration_ms={})",
            summary.run_id, summary.final_tick, summary.wall_duration_ms
        );
    }
    Ok(())
}

fn reserve_direct_output(
    run: &ResolvedRun,
    build_info: &BuildInfo,
    executable: &std::path::Path,
    started_at: &str,
    threads: u32,
) -> Result<RunManifestRecord, RunnerError> {
    let parent = run.output_directory.parent().ok_or_else(|| {
        RunnerError::operational(format!(
            "output directory has no parent: {}",
            run.output_directory.display()
        ))
    })?;
    fs::create_dir_all(parent).map_err(|error| {
        RunnerError::operational(format!(
            "failed to create output parent {}: {error}",
            parent.display()
        ))
    })?;
    fs::create_dir(&run.output_directory).map_err(|error| {
        RunnerError::operational(format!(
            "output directory must not already exist ({}): {error}",
            run.output_directory.display()
        ))
    })?;
    let record = run_manifest_record(run, build_info, executable, started_at.to_owned(), threads)?;
    write_json_atomic(&run.output_directory.join("manifest.json"), &record)?;
    Ok(record)
}

fn verify_supervised_reservation(
    run: &ResolvedRun,
    build_info: &BuildInfo,
    threads: u32,
) -> Result<RunManifestRecord, RunnerError> {
    if !run.output_directory.is_dir() {
        return Err(RunnerError::operational(format!(
            "supervised output reservation is missing: {}",
            run.output_directory.display()
        )));
    }
    let record: RunManifestRecord = read_json(
        &run.output_directory.join("manifest.json"),
        "supervisor manifest record",
    )?;
    let expected_input = run.normalized_input()?;
    if record.runner_schema_version != RUNNER_SCHEMA_VERSION
        || record.run_id != run.manifest.run_id
        || record.input_digest != run.input_digest
        || record.execution_digest != build_info.execution_digest
        || record.input != expected_input
        || record.build != build_info.build
        || record.execution.engine_threads != threads
    {
        return Err(RunnerError::operational(
            "supervisor manifest record does not match requested run",
        ));
    }
    for unexpected in ["metrics.jsonl", "summary.json", "completion.json"] {
        if run.output_directory.join(unexpected).exists() {
            return Err(RunnerError::operational(format!(
                "unexpected runner artifact in fresh reservation: {unexpected}"
            )));
        }
    }
    Ok(record)
}

fn write_metrics(
    file: &mut File,
    record: &RunManifestRecord,
    metrics: crate::observe::MetricsSnapshot,
) -> Result<(), RunnerError> {
    let envelope = MetricsEnvelope {
        runner_schema_version: RUNNER_SCHEMA_VERSION.to_owned(),
        run_id: record.run_id.clone(),
        input_digest: record.input_digest.clone(),
        execution_digest: record.execution_digest.clone(),
        metrics,
    };
    serde_json::to_writer(&mut *file, &envelope)
        .map_err(|error| RunnerError::operational(format!("failed to encode metrics: {error}")))?;
    file.write_all(b"\n")
        .map_err(|error| RunnerError::operational(format!("failed to write metrics: {error}")))?;
    file.flush()
        .map_err(|error| RunnerError::operational(format!("failed to flush metrics: {error}")))?;
    Ok(())
}

fn duration_millis(duration: std::time::Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

#[cfg(feature = "rayon")]
fn configure_threads(threads: u32) -> Result<(), RunnerError> {
    rayon::ThreadPoolBuilder::new()
        .num_threads(threads as usize)
        .build_global()
        .map_err(|error| RunnerError::operational(format!("failed to configure Rayon: {error}")))
}

#[cfg(not(feature = "rayon"))]
fn configure_threads(threads: u32) -> Result<(), RunnerError> {
    if threads == 1 {
        Ok(())
    } else {
        Err(RunnerError::invalid(
            "--threads above 1 requires a Rayon-enabled build",
        ))
    }
}
