use std::collections::{HashSet, VecDeque};
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use super::{
    artifact_paths, build_provenance, canonical_file, load_batch_manifest, load_run_manifest,
    path_string, read_json, run_manifest_record, timestamp_now, write_json_atomic, ArtifactPaths,
    BuildInfo, CompletionCategory, CompletionRecord, ResolvedRun, RunManifestRecord, RunSummary,
    RunnerError, RUNNER_SCHEMA_VERSION,
};

const INTERRUPT_GRACE: Duration = Duration::from_secs(5);
const POLL_INTERVAL: Duration = Duration::from_millis(20);

/// Parsed `proteus-batch` execution options.
#[derive(Clone, Debug)]
pub struct BatchOptions {
    pub manifest_path: PathBuf,
    pub retry_incomplete: bool,
}

/// Semantic batch result mapped to the process exit contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BatchOutcome {
    Success,
    Failed,
    Interrupted,
}

struct PendingRun {
    run: ResolvedRun,
    archive_existing: bool,
}

struct RunningChild {
    run: ResolvedRun,
    child: Child,
    started_at: String,
    started: Instant,
    artifacts: ArtifactPaths,
    interrupted: bool,
}

impl Drop for RunningChild {
    fn drop(&mut self) {
        match self.child.try_wait() {
            Ok(Some(_)) => {}
            Ok(None) | Err(_) => {
                let _ = self.child.kill();
                let _ = self.child.wait();
            }
        }
    }
}

enum ExistingState {
    New,
    Complete,
    Incomplete,
    Unsafe(String),
}

/// Supervises a fixed batch of child processes with bounded concurrency.
pub fn batch_main(options: BatchOptions) -> Result<BatchOutcome, RunnerError> {
    let (batch, batch_path) = load_batch_manifest(&options.manifest_path)?;
    let child_path = locate_child_executable()?;
    let child_info = inspect_child(&child_path)?;
    if child_info.runner_schema_version != RUNNER_SCHEMA_VERSION {
        return Err(RunnerError::invalid(format!(
            "child runner schema {} does not match {RUNNER_SCHEMA_VERSION}",
            child_info.runner_schema_version
        )));
    }
    if child_info.build != build_provenance() {
        return Err(RunnerError::invalid(
            "proteus-batch and proteus-run build provenance do not match",
        ));
    }

    let base = batch_path
        .parent()
        .expect("canonical batch manifest should have a parent");
    let mut runs = Vec::with_capacity(batch.runs.len());
    for reference in &batch.runs {
        let path = if Path::new(&reference.manifest).is_absolute() {
            PathBuf::from(&reference.manifest)
        } else {
            base.join(&reference.manifest)
        };
        runs.push(load_run_manifest(&path)?);
    }
    validate_batch_paths(&batch_path, &runs)?;

    let mut pending = VecDeque::new();
    let mut had_failure = false;
    for run in runs {
        match existing_state(&run, &child_info)? {
            ExistingState::New => pending.push_back(PendingRun {
                run,
                archive_existing: false,
            }),
            ExistingState::Complete => {}
            ExistingState::Incomplete if options.retry_incomplete => {
                pending.push_back(PendingRun {
                    run,
                    archive_existing: true,
                });
            }
            ExistingState::Incomplete => {
                eprintln!(
                    "run {} is incomplete; use --retry-incomplete to archive and restart it",
                    run.manifest.run_id
                );
                had_failure = true;
            }
            ExistingState::Unsafe(message) => {
                return Err(RunnerError::invalid(format!(
                    "run {} has an unsafe output collision: {message}",
                    run.manifest.run_id
                )));
            }
        }
    }

    let signal_count = Arc::new(AtomicUsize::new(0));
    let handler_count = Arc::clone(&signal_count);
    ctrlc::set_handler(move || {
        handler_count.fetch_add(1, Ordering::SeqCst);
    })
    .map_err(|error| {
        RunnerError::operational(format!("failed to install signal handler: {error}"))
    })?;

    let mut running = Vec::<RunningChild>::new();
    let mut interrupted_at = None::<Instant>;

    while !pending.is_empty() || !running.is_empty() {
        let observed_signals = signal_count.load(Ordering::SeqCst);
        if observed_signals > 0 && interrupted_at.is_none() {
            interrupted_at = Some(Instant::now());
            for child in &mut running {
                child.interrupted = true;
                request_termination(&mut child.child);
            }
        }

        if interrupted_at.is_none() {
            while running.len() < batch.jobs as usize {
                let Some(pending_run) = pending.pop_front() else {
                    break;
                };
                match launch_child(pending_run, &child_path, &child_info)? {
                    Some(child) => running.push(child),
                    None => had_failure = true,
                }
            }
        }

        if let Some(started) = interrupted_at {
            if signal_count.load(Ordering::SeqCst) > 1 || started.elapsed() >= INTERRUPT_GRACE {
                for child in &mut running {
                    let _ = child.child.kill();
                }
            }
        }

        let mut index = 0;
        let mut made_progress = false;
        while index < running.len() {
            let status = running[index].child.try_wait().map_err(|error| {
                RunnerError::operational(format!(
                    "failed to poll child for run {}: {error}",
                    running[index].run.manifest.run_id
                ))
            })?;
            if let Some(status) = status {
                let finished = running.swap_remove(index);
                let success = finish_child(finished, status, &child_info)?;
                had_failure |= !success;
                made_progress = true;
            } else {
                index += 1;
            }
        }

        if !made_progress && (!running.is_empty() || interrupted_at.is_none()) {
            thread::sleep(POLL_INTERVAL);
        }
        if interrupted_at.is_some() && running.is_empty() {
            break;
        }
    }

    if interrupted_at.is_some() {
        Ok(BatchOutcome::Interrupted)
    } else if had_failure || !pending.is_empty() {
        Ok(BatchOutcome::Failed)
    } else {
        Ok(BatchOutcome::Success)
    }
}

fn locate_child_executable() -> Result<PathBuf, RunnerError> {
    let current = std::env::current_exe().map_err(|error| {
        RunnerError::invalid(format!("failed to resolve batch executable: {error}"))
    })?;
    let sibling = current.with_file_name(format!("proteus-run{}", std::env::consts::EXE_SUFFIX));
    canonical_file(&sibling, "proteus-run executable")
}

fn inspect_child(path: &Path) -> Result<BuildInfo, RunnerError> {
    let output = Command::new(path)
        .arg("--build-info")
        .output()
        .map_err(|error| {
            RunnerError::invalid(format!(
                "failed to inspect child {}: {error}",
                path.display()
            ))
        })?;
    if !output.status.success() {
        return Err(RunnerError::invalid(format!(
            "child build-info failed with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    serde_json::from_slice(&output.stdout)
        .map_err(|error| RunnerError::invalid(format!("invalid child build-info output: {error}")))
}

fn validate_batch_paths(batch_path: &Path, runs: &[ResolvedRun]) -> Result<(), RunnerError> {
    let mut run_ids = HashSet::new();
    for run in runs {
        if !run_ids.insert(run.manifest.run_id.as_str()) {
            return Err(RunnerError::invalid(format!(
                "duplicate run_id in batch: {}",
                run.manifest.run_id
            )));
        }
        if run.output_directory.parent().is_none() {
            return Err(RunnerError::invalid(
                "output directory must not be a filesystem root",
            ));
        }
    }

    let input_paths = std::iter::once(batch_path)
        .chain(runs.iter().map(|run| run.source_manifest.as_path()))
        .collect::<Vec<_>>();
    for (index, run) in runs.iter().enumerate() {
        for input in &input_paths {
            if input.starts_with(&run.output_directory) {
                return Err(RunnerError::invalid(format!(
                    "output directory {} contains input manifest {}",
                    run.output_directory.display(),
                    input.display()
                )));
            }
            if path_in_archive_namespace(&run.output_directory, input) {
                return Err(RunnerError::invalid(format!(
                    "input manifest {} collides with retry namespace for {}",
                    input.display(),
                    run.output_directory.display()
                )));
            }
        }
        for other in runs.iter().skip(index + 1) {
            if run.output_directory.starts_with(&other.output_directory)
                || other.output_directory.starts_with(&run.output_directory)
            {
                return Err(RunnerError::invalid(format!(
                    "overlapping output directories: {} and {}",
                    run.output_directory.display(),
                    other.output_directory.display()
                )));
            }
            if path_in_archive_namespace(&run.output_directory, &other.output_directory)
                || path_in_archive_namespace(&other.output_directory, &run.output_directory)
            {
                return Err(RunnerError::invalid(format!(
                    "output directory collides with retry archive namespace: {} and {}",
                    run.output_directory.display(),
                    other.output_directory.display()
                )));
            }
        }
    }
    Ok(())
}

fn path_in_archive_namespace(output: &Path, candidate: &Path) -> bool {
    let Some(parent) = output.parent() else {
        return false;
    };
    let Ok(relative) = candidate.strip_prefix(parent) else {
        return false;
    };
    let Some(first) = relative.components().next() else {
        return false;
    };
    let Some(first) = first.as_os_str().to_str() else {
        return false;
    };
    let Some(output_name) = output.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    let Some(suffix) = first.strip_prefix(&format!("{output_name}.attempt-")) else {
        return false;
    };
    suffix.len() >= 4 && suffix.bytes().all(|byte| byte.is_ascii_digit())
}

fn existing_state(run: &ResolvedRun, child_info: &BuildInfo) -> Result<ExistingState, RunnerError> {
    if !path_is_occupied(&run.output_directory)? {
        return Ok(ExistingState::New);
    }
    if !run.output_directory.is_dir() {
        return Ok(ExistingState::Unsafe(format!(
            "output path is not a directory: {}",
            run.output_directory.display()
        )));
    }
    let manifest_path = run.output_directory.join("manifest.json");
    let record = match read_json::<RunManifestRecord>(&manifest_path, "runner ownership manifest") {
        Ok(record) => record,
        Err(error) => return Ok(ExistingState::Unsafe(error.to_string())),
    };
    if record.runner_schema_version != RUNNER_SCHEMA_VERSION
        || record.input.output_directory != path_string(&run.output_directory)?
    {
        return Ok(ExistingState::Unsafe(
            "existing directory does not contain a matching runner ownership marker".to_owned(),
        ));
    }
    let completion_path = run.output_directory.join("completion.json");
    let completion = match read_json::<CompletionRecord>(&completion_path, "completion record") {
        Ok(completion) => completion,
        Err(_) => return Ok(ExistingState::Incomplete),
    };
    if completion.success
        && completion.runner_schema_version == RUNNER_SCHEMA_VERSION
        && completion.category == CompletionCategory::Succeeded
        && completion.valid_summary
        && completion.child_exit_code == Some(0)
        && completion.child_signal.is_none()
        && completion.run_id == run.manifest.run_id
        && completion.input_digest == run.input_digest
        && completion.build == child_info.build
        && record.run_id == completion.run_id
        && record.input_digest == completion.input_digest
        && record.execution_digest == completion.execution_digest
        && record.build == completion.build
        && completion.artifacts == artifact_paths(&run.output_directory)?
    {
        Ok(ExistingState::Complete)
    } else {
        Ok(ExistingState::Incomplete)
    }
}

fn launch_child(
    pending: PendingRun,
    child_path: &Path,
    child_info: &BuildInfo,
) -> Result<Option<RunningChild>, RunnerError> {
    let run = pending.run;
    if pending.archive_existing {
        archive_existing(&run.output_directory)?;
    }
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
            "failed to reserve output directory {}: {error}",
            run.output_directory.display()
        ))
    })?;

    let started_at = timestamp_now()?;
    let record = run_manifest_record(&run, child_info, child_path, started_at.clone(), 1)?;
    write_json_atomic(&run.output_directory.join("manifest.json"), &record)?;
    let stdout_path = run.output_directory.join("stdout.log");
    let stderr_path = run.output_directory.join("stderr.log");
    let stdout = File::create(&stdout_path).map_err(|error| {
        RunnerError::operational(format!(
            "failed to create {}: {error}",
            stdout_path.display()
        ))
    })?;
    let stderr = File::create(&stderr_path).map_err(|error| {
        RunnerError::operational(format!(
            "failed to create {}: {error}",
            stderr_path.display()
        ))
    })?;
    let artifacts = artifact_paths(&run.output_directory)?;
    let started = Instant::now();
    match Command::new(child_path)
        .args([
            "--manifest",
            run.source_manifest
                .to_str()
                .expect("validated manifest path should be UTF-8"),
            "--threads",
            "1",
            "--supervised",
        ])
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr))
        .spawn()
    {
        Ok(child) => Ok(Some(RunningChild {
            run,
            child,
            started_at,
            started,
            artifacts,
            interrupted: false,
        })),
        Err(error) => {
            let completion = completion_record(
                &run,
                child_info,
                CompletionCategory::LaunchFailed,
                false,
                None,
                None,
                false,
                started_at,
                started,
                artifacts,
            )?;
            write_json_atomic(&run.output_directory.join("completion.json"), &completion)?;
            eprintln!("failed to launch run {}: {error}", run.manifest.run_id);
            Ok(None)
        }
    }
}

fn archive_existing(output: &Path) -> Result<(), RunnerError> {
    let parent = output
        .parent()
        .expect("validated output should have a parent");
    let name = output
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| RunnerError::operational("output filename is not UTF-8"))?;
    for ordinal in 1_u64.. {
        let archive = parent.join(format!("{name}.attempt-{ordinal:04}"));
        if !path_is_occupied(&archive)? {
            fs::rename(output, &archive).map_err(|error| {
                RunnerError::operational(format!(
                    "failed to archive {} as {}: {error}",
                    output.display(),
                    archive.display()
                ))
            })?;
            return Ok(());
        }
    }
    unreachable!("u64 retry archive namespace should not be exhausted")
}

fn path_is_occupied(path: &Path) -> Result<bool, RunnerError> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(RunnerError::operational(format!(
            "failed to inspect output path {}: {error}",
            path.display()
        ))),
    }
}

fn finish_child(
    child: RunningChild,
    status: ExitStatus,
    child_info: &BuildInfo,
) -> Result<bool, RunnerError> {
    let summary_path = child.run.output_directory.join("summary.json");
    let summary = read_json::<RunSummary>(&summary_path, "child summary").ok();
    let valid_summary = summary.as_ref().is_some_and(|summary| {
        summary.runner_schema_version == RUNNER_SCHEMA_VERSION
            && summary.run_id == child.run.manifest.run_id
            && summary.input_digest == child.run.input_digest
            && summary.execution_digest == child_info.execution_digest
            && summary.build == child_info.build
            && summary.final_tick == child.run.manifest.limits.ticks
            && summary.termination_reason == "tick_limit_reached"
    });
    let (category, success) = if child.interrupted {
        (CompletionCategory::SupervisorInterrupted, false)
    } else if status.success() && valid_summary {
        (CompletionCategory::Succeeded, true)
    } else if status.success() {
        (CompletionCategory::InvalidChildOutput, false)
    } else if status.code().is_none() {
        (CompletionCategory::Signaled, false)
    } else {
        (CompletionCategory::ChildFailed, false)
    };
    let completion = completion_record(
        &child.run,
        child_info,
        category,
        success,
        status.code(),
        exit_signal(&status),
        valid_summary,
        child.started_at.clone(),
        child.started,
        child.artifacts.clone(),
    )?;
    write_json_atomic(
        &child.run.output_directory.join("completion.json"),
        &completion,
    )?;
    Ok(success)
}

#[allow(clippy::too_many_arguments)]
fn completion_record(
    run: &ResolvedRun,
    child_info: &BuildInfo,
    category: CompletionCategory,
    success: bool,
    exit_code: Option<i32>,
    signal: Option<String>,
    valid_summary: bool,
    started_at: String,
    started: Instant,
    artifacts: ArtifactPaths,
) -> Result<CompletionRecord, RunnerError> {
    Ok(CompletionRecord {
        runner_schema_version: RUNNER_SCHEMA_VERSION.to_owned(),
        run_id: run.manifest.run_id.clone(),
        input_digest: run.input_digest.clone(),
        execution_digest: child_info.execution_digest.clone(),
        build: child_info.build.clone(),
        success,
        category,
        child_exit_code: exit_code,
        child_signal: signal,
        valid_summary,
        started_at,
        finished_at: timestamp_now()?,
        wall_duration_ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
        artifacts,
    })
}

#[cfg(unix)]
fn exit_signal(status: &ExitStatus) -> Option<String> {
    use std::os::unix::process::ExitStatusExt;
    status.signal().map(|signal| format!("signal:{signal}"))
}

#[cfg(not(unix))]
fn exit_signal(_status: &ExitStatus) -> Option<String> {
    None
}

fn request_termination(child: &mut Child) {
    #[cfg(unix)]
    {
        let status = Command::new("kill")
            .args(["-TERM", &child.id().to_string()])
            .status();
        if !status.is_ok_and(|status| status.success()) {
            let _ = child.kill();
        }
    }
    #[cfg(not(unix))]
    {
        let _ = child.kill();
    }
}

#[cfg(test)]
mod tests {
    use super::path_in_archive_namespace;
    use std::path::Path;

    #[test]
    fn retry_archive_namespace_includes_descendants() {
        let output = Path::new("/tmp/runs/x");
        assert!(path_in_archive_namespace(
            output,
            Path::new("/tmp/runs/x.attempt-0001")
        ));
        assert!(path_in_archive_namespace(
            output,
            Path::new("/tmp/runs/x.attempt-10000/nested")
        ));
        assert!(!path_in_archive_namespace(
            output,
            Path::new("/tmp/runs/x.attempt-12")
        ));
        assert!(!path_in_archive_namespace(
            output,
            Path::new("/tmp/runs/y.attempt-0001")
        ));
    }
}
