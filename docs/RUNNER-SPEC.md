# Proteus Headless Runner Specification (Provisional)

**Status**: Provisional design proposal; not yet implemented.

**Targets**: Proteus v0.2.1 engine, API metrics schema v0.2.2.

---

## 1. Purpose

This document defines a deterministic, non-interactive execution surface for Proteus simulations. It covers two tools:

- `proteus-run`: execute one fully specified simulation.
- `proteus-batch`: supervise a fixed list of `proteus-run` subprocesses.

The runner exists to make long runs and repeatable ensembles practical without starting the web server or frontend. It uses the Rust engine directly; it does not communicate through REST or WebSocket.

The runner is an execution substrate, not an experiment planner. It records what ran and what happened. A separate outer loop may choose configurations, classify outcomes, tune parameters, or produce plots.

## 2. Scope and Non-Goals

### In scope

- Exact single-run input and output contracts
- Shared world/bootstrap initialization
- Deterministic execution to a tick limit
- Sampled metrics and final cumulative event totals
- Subprocess supervision and bounded run-level concurrency
- Job-level completion, failure, and resume semantics
- Provenance sufficient to reproduce a run

### Explicit non-goals

- Parameter ranges, sweep expansion, or search strategies
- Fitness functions or an "emergence score"
- Bayesian optimization, Sobol sampling, or campaign promotion rules
- Scientific interpretation of extinction, reproduction, or novelty
- Runtime mutation of simulation parameters
- HTTP, WebSocket, or frontend integration
- In-run checkpoint/resume in the MVP

An outer loop may generate a fixed batch manifest and consume runner outputs, but those decisions are outside this contract.

## 3. Design Principles

1. **One run, one process.** Each `proteus-run` process owns exactly one `Simulation`.
2. **Exact inputs only.** The runner accepts fully expanded jobs, never parameter domains or search instructions.
3. **Shared initialization semantics.** Web and headless execution use the same bootstrap implementation; seed placement must not be copied into the runner.
4. **Observation cannot affect simulation.** Changing metrics cadence changes output volume, not tick execution or final state.
5. **Failures are data.** The supervisor records process failures and operational termination distinctly from biological outcomes.
6. **Reproducibility is explicit.** Inputs, engine identity, build settings, and termination reason are persisted with every run.
7. **The supervisor owns completion.** A child that panics, is killed, or is terminated by the operating system cannot be trusted to write its own final status.

## 4. Architecture

```text
outer loop (optional, out of scope)
    |
    | writes fixed jobs
    v
proteus-batch
    |-- proteus-run <run A>
    |-- proteus-run <run B>
    `-- proteus-run <run C>
          |
          `-- Simulation::run_tick_report()
```

### `proteus-run`

`proteus-run` reads one run manifest, initializes one world, advances it to the requested tick limit, writes observations, and exits. It does not launch other runs or decide whether a result is interesting.

### `proteus-batch`

`proteus-batch` reads an already-expanded list of run manifests and launches up to `jobs` child processes concurrently. It writes authoritative completion records, supports job-level resume, and exits nonzero if any requested run did not complete successfully.

Subprocess isolation is intentional. A simulation panic or runaway memory use should terminate one run rather than corrupting or killing an in-process ensemble. Process startup overhead is negligible relative to ecological runs.

## 5. Shared Bootstrap Boundary

Before implementing the runner, extract seed placement from `rust/src/web/controller.rs` into a reusable engine-adjacent bootstrap module. The web controller and runner must call the same code.

The shared bootstrap model owns:

- program bytecode and placement
- attached free energy and free mass
- randomized initial `Dir` and `ID`
- the exact bootstrap RNG salt and derivation
- explicit environmental preloads, including resource-rich neighboring empty cells

Initialization order is fixed:

1. Construct the grid and stationary Poisson background from `SimConfig`.
2. Apply explicit environmental preloads.
3. Place programs and their attached free resources.
4. Publish or record tick 0.
5. Begin the first simulation tick.

Environmental preloads use exact `set` semantics. They replace the initialized values at addressed cells rather than silently adding to stochastic background values.

Bootstrap programs are initial state, not births. They do not increment cumulative event totals.

This boundary corresponds to `SEED-BOOTSTRAP` and `SEED-ENVIRONMENT` in `BACKLOG.md`.

## 6. Single-Run Input Contract

The precise serialization format remains provisional; JSON is the recommended MVP format because it is unambiguous and easy for external tools to generate. A single-run manifest contains only exact values:

```json
{
  "runner_schema_version": "0.1.0",
  "run_id": "example-run-0001",
  "simulation": {
    "width": 64,
    "height": 64,
    "seed": 42,
    "r_energy": 0.25,
    "r_mass": 0.05,
    "d_energy": 0.01,
    "d_mass": 0.01,
    "t_cap": 4.0,
    "maintenance_rate": 0.0078125,
    "maintenance_exponent": 1.0,
    "local_action_exponent": 1.0,
    "n_synth": 1,
    "inert_grace_ticks": 10,
    "p_spawn": 0.0,
    "mutation_base_log2": 16,
    "mutation_background_log2": 8
  },
  "bootstrap": {
    "programs": [],
    "environment": []
  },
  "limits": {
    "ticks": 10000
  },
  "observation": {
    "every_n_ticks": 50
  },
  "output_directory": "runs/example-run-0001"
}
```

Required semantic fields are:

| Field | Meaning |
|---|---|
| `runner_schema_version` | Version of the runner manifest contract |
| `run_id` | Stable caller-supplied identity, unique within a batch |
| `simulation` | Fully specified immutable `SimConfig` |
| `bootstrap` | Exact program placement and environmental preload |
| `limits.ticks` | Final tick to execute; must be greater than zero |
| `observation.every_n_ticks` | Metrics sampling cadence; must be greater than zero |
| `output_directory` | Exclusive output location for this run |

The manifest contains no parameter ranges, replicate counts, or conditional promotion rules. A caller that wants 100 configurations supplies 100 exact manifests.

## 7. Run Identity and Provenance

`run_id` is the external human-readable identity. The runner also computes a canonical input digest from the normalized manifest fields that affect simulation or observation.

Each run records:

- runner schema version
- engine crate and simulator version
- Git commit when available
- whether the source worktree was dirty when built or launched
- enabled Cargo features
- complete expanded input manifest
- canonical input digest
- world RNG seed
- bootstrap RNG salt/version
- effective Rayon thread count, if Rayon is active
- start and finish timestamps
- wall-clock duration
- final tick
- termination reason

Same-version replay requires identical simulation inputs, bootstrap inputs, seed, and execution semantics. Determinism across different engine commits or runner schema versions is not promised.

## 8. Tick and Observation Semantics

The child tick loop has one event-recording funnel:

```text
report = simulation.run_tick_report()
event_totals.record(report)

if tick matches observation cadence:
    collect current-state metrics
    write metrics record
```

Every simulation tick executes even when no metrics row is written. An observation cadence of 50 means the runner records ticks 0, 50, 100, and so on; it does not skip simulation work.

Metrics retain the API v0.2.2 distinction:

- gauges such as population, resources, packet energy, and program sizes describe the sampled tick
- top-level birth, death, and mutation fields describe the most recently completed tick
- `event_totals` contain cumulative birth, death, and mutation counts over every completed tick

Cumulative totals prevent sampled observations from losing events between rows. The final completion summary always includes cumulative totals, even if the final tick does not match the observation cadence.

O(grid) aggregate metrics are computed only at observation cadence and for the final summary. The runner must not scan the grid on every tick merely to support a coarser output cadence.

## 9. Single-Run Outputs

One run directory contains:

```text
<run_id>/
├── manifest.json
├── metrics.jsonl
├── summary.json
├── completion.json
├── stdout.log
└── stderr.log
```

### `manifest.json`

An immutable copy of the normalized input plus provenance known before execution. It is written before the first tick.

### `metrics.jsonl`

Append-only sampled metrics, one JSON object per line. JSONL is the MVP format because it preserves the nested cumulative-event object and remains streamable after partial failure.

The first row represents tick 0. A final row is written at the actual final tick if cadence would otherwise omit it.

### `summary.json`

Written by `proteus-run` only after normal completion. It contains final tick, final gauges, cumulative event totals, duration, and the child's self-reported termination reason. It is useful output but is not the batch supervisor's authoritative evidence that the process completed.

### `completion.json`

Written only by `proteus-batch`, after the supervisor observes the child exit. The supervisor writes a temporary file, flushes it, and atomically renames it into place. A run launched without `proteus-batch` has no supervisor completion record.

### Logs

The supervisor captures child stdout and stderr per run. Log files must not be treated as the machine-readable result contract.

Snapshots and full-grid artifacts are deferred. They may be added after `SNAPSHOT-BOUNDARY` establishes a stable engine snapshot format.

## 10. Termination Semantics

The MVP child has one normal termination condition:

- `tick_limit_reached`

It may also end abnormally because of:

- invalid manifest or bootstrap
- output I/O failure
- engine/controller panic
- nonzero process exit
- signal or operating-system termination
- supervisor interruption

The runner reports operational facts; it does not label a run scientifically as successful, extinct, frozen, or emergent.

Wall-clock limits, memory limits, extinction stops, saturation stops, and bloat stops are deferred. When added, operational resource limits must be recorded as `resource_exhausted` or another explicit operational reason—not as biological extinction.

## 11. Batch Input and Scheduling

The batch manifest is an exact list of single-run manifests plus a concurrency limit:

```json
{
  "runner_schema_version": "0.1.0",
  "jobs": 8,
  "runs": [
    { "manifest": "manifests/run-0001.json" },
    { "manifest": "manifests/run-0002.json" }
  ]
}
```

MVP exposes only run-level `jobs`. It builds `proteus-run` against the existing serial engine path, does not expose `threads_per_job`, and does not attempt automatic CPU topology management. Rayon integration and per-child thread scheduling are deferred. Independent-run parallelism is the MVP strategy for ensembles.

The supervisor validates all run IDs and output directories before launching the first child. Duplicate identities or overlapping output directories are errors.

Scheduling order has no simulation semantics. Each child is deterministic from its own inputs and cannot share mutable simulation state with another child.

## 12. Completion Records and Resume

`proteus-batch` writes one authoritative `completion.json` per run after observing the child process exit. A separate append-only batch index may be added for convenient analysis, but it is not authoritative for resume.

A completion record contains at least:

- `run_id`
- canonical input digest
- child exit status or signal
- supervisor-observed termination category
- whether a valid child summary was present
- start and finish timestamps
- wall-clock duration
- paths to manifest, metrics, summary, stdout, and stderr

The supervisor, not the child, owns this record. A killed or out-of-memory child cannot reliably describe its own termination.

On resume:

1. A run is complete only when a successful supervisor completion record exists and its input digest matches the requested manifest.
2. Matching completed runs are skipped.
3. Missing, failed, or digest-mismatched runs are not silently accepted as complete.
4. The default retry policy is explicit rather than automatic: rerun only when requested by the caller.
5. Partial output from a retried run is preserved or moved aside; it is never confused with the new attempt.

MVP resume occurs only between whole runs. Resuming within a partially completed simulation is deferred with snapshot support.

## 13. Exit Status

Recommended process-level behavior:

- `proteus-run` exits 0 only after reaching its tick limit and durably writing its final outputs.
- `proteus-run` exits nonzero for invalid input, engine failure, or output failure.
- `proteus-batch` exits 0 only if every requested run is already complete or completes successfully during this invocation.
- `proteus-batch` exits nonzero if any requested run fails, cannot launch, or remains incomplete.

Exact numeric exit codes are an implementation detail until the CLI contract is finalized.

## 14. MVP Delivery Boundary

The smallest corrected MVP is:

1. Shared bootstrap module with neighborhood preload support.
2. `proteus-run` wrapping `Simulation::run_tick_report()`.
3. Tick-limit termination only.
4. Sampled JSONL metrics using cumulative event counters.
5. Immutable manifest and final summary.
6. `proteus-batch` supervising `N` single-threaded subprocesses.
7. Supervisor-owned atomic completion records.
8. Job-level resume by run ID plus canonical input digest.

Defer memory limits, wall-clock limits, Rayon/thread controls, in-run checkpoints, optimizer integration, and scientific stopping rules.

## 15. Required Tests

- A headless run and direct engine run produce the same final state for identical inputs.
- Web and headless bootstrap produce identical tick-0 worlds.
- Bootstrap programs do not count as births.
- Observation cadences 1 and 100 produce identical final cumulative event totals.
- Observation cadence does not change final simulation state.
- The final metrics row is emitted when the tick limit is off cadence.
- Duplicate run IDs and overlapping output directories are rejected before launch.
- `jobs` is never exceeded.
- A child panic/nonzero exit produces a supervisor-authored failure completion record.
- A killed child cannot be mistaken for success because it happened to leave a summary file.
- Resume skips only matching successful input digests.
- Interrupted JSONL output remains readable through its last complete line.

## 16. Open Implementation Questions

These questions are inside the runner boundary and should be settled before implementation:

1. Should manifests embed run definitions or reference one file per run? The MVP may support both only if normalization produces one canonical form.
2. Should provenance be captured at build time, launch time, or both when a binary is run from a dirty checkout?
3. What durability guarantees beyond write-temp, flush, and atomic rename are required on each supported filesystem?
4. How should partial output directories be named and retained across explicit retries?
5. Should `metrics.jsonl` use the API metrics object verbatim or a runner envelope with schema/provenance fields?

Questions about which configurations to try, how to score outcomes, or how to optimize emergence are not runner design questions and do not belong in this document.
