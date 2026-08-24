# Proteus Headless Runner Specification

**Status**: Implemented MVP contract.

**Targets**: Proteus v0.4.0 engine, API metrics schema v0.3.1.

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

The MVP bootstrap JSON has this shape:

```json
{
  "programs": [
    {
      "x": 32,
      "y": 24,
      "code": [80, 100],
      "free_energy": 20,
      "free_mass": 12
    }
  ],
  "environment": [
    {
      "x": 31,
      "y": 24,
      "free_energy": 20,
      "free_mass": 12,
      "bg_radiation": 0,
      "bg_mass": 0
    }
  ]
}
```

All fields in both entry types are required. Coordinates are 0-indexed `(x, y)`
API coordinates and must be in bounds. Program code is an array of bytes, must
be nonempty, and must not exceed `PROGRAM_SIZE_CAP`. Duplicate program
coordinates and duplicate environment coordinates are rejected.

An environment entry sets all four resource pools on its cell. Environment and
program entries may address the same cell: the environment entry first sets all
four pools, then program placement replaces `free_energy` and `free_mass` with
the program's attached values while retaining the preloaded background pools.
This follows the initialization order below and permits an exact preload on a
seed cell without adding bootstrap-only program fields.

Bootstrap RNG semantics are versioned as `seed-program-v1`. Preserve the
existing web-controller derivation when extracting it: for row-major
`cell_index`, construct
`cell_rng(simulation.seed ^ 0x0d7e_8ef0_4268_33c1, 0, cell_index)`, draw `Dir`
first as `next_u32() % 4`, then draw `ID` as `next_u32() as u8`. Because each
program uses a cell-derived RNG, manifest array order has no effect on the
initialized world. The version and salt are execution provenance, not
caller-configurable manifest fields.

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

The MVP input format is UTF-8 JSON. `proteus-run` accepts `proteus-run
--manifest <path> [--threads <N>] [--verbosity <0|1>]`; `-v` is an alias for
`--verbosity`. Thread count must be greater than zero and defaults to 1. A value
above 1 requires a Rayon-enabled build; otherwise it is a validation error.
Verbosity defaults to 1 and writes basic start, resolved-output, and completion
status to stderr. Verbosity 0 suppresses successful-run status but never errors.
Thread count and verbosity are operational execution settings, not simulation
inputs. JSON objects are strict: unknown fields, missing fields,
duplicate object keys, non-finite numbers, and values outside the corresponding
Rust integer range are errors. No manifest field receives an implicit default.

Proteus v0.4.0 manifests use runner schema `0.2.0` and are not backward
compatible with pre-v0.4.0 manifests. Existing run trees written under schema
`0.1.0` cannot resume: the probability field renames already make their
manifests fail `deny_unknown_fields` regardless of version. The schema bump
labels that existing break rather than adding a second incompatibility.

`runner_schema_version` must equal `0.2.0`. The simulation fields `d_energy_log2`, `d_mass_log2`, `maintenance_rate_log2`, and `p_spawn_log2` must each be present explicitly and contain either `null` (never) or an integer in `0..=63`; both mutation exponent fields must be integers in `0..=63`. Runner manifests never inherit the web/config serde defaults.

A single-run manifest contains only exact values:

```json
{
  "runner_schema_version": "0.2.0",
  "run_id": "example-run-0001",
  "simulation": {
    "width": 64,
    "height": 64,
    "seed": 42,
    "r_energy": 0.25,
    "r_mass": 0.05,
    "d_energy_log2": 7,
    "d_mass_log2": 7,
    "t_cap": 4.0,
    "maintenance_rate_log2": 7,
    "maintenance_exponent": 1.0,
    "local_action_exponent": 1.0,
    "n_synth": 1,
    "inert_grace_ticks": 10,
    "p_spawn_log2": null,
    "mutation_base_log2": 16,
    "mutation_background_log2": 8
  },
  "bootstrap": {
    "programs": [
      {
        "x": 32,
        "y": 24,
        "code": [80, 100],
        "free_energy": 20,
        "free_mass": 12
      }
    ],
    "environment": [
      {
        "x": 31,
        "y": 24,
        "free_energy": 20,
        "free_mass": 12,
        "bg_radiation": 0,
        "bg_mass": 0
      }
    ]
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
| `limits.ticks` | Number of complete ticks to execute and resulting final tick counter; must be greater than zero |
| `observation.every_n_ticks` | Metrics sampling cadence; must be greater than zero |
| `output_directory` | Exclusive output location for this run |

`run_id` must match `[A-Za-z0-9][A-Za-z0-9._-]{0,127}`. A relative
`output_directory` is resolved relative to the single-run manifest's parent
directory, not the process working directory. The normalized output record uses
the resolved absolute path. All input and resolved paths must be valid UTF-8 in
runner schema `0.2.0`.

The `simulation` object contains every field of `SimConfig` with exactly the
names shown above. It is validated by `SimConfig::validate()` after JSON type
validation. `limits.ticks` and `observation.every_n_ticks` are `u64` values
greater than zero. The manifest contains no parameter ranges, replicate counts,
or conditional promotion rules. A caller that wants 100 configurations supplies
100 exact manifests.

Direct `proteus-run` execution requires `output_directory` not to exist and
creates it exclusively. It never deletes, truncates, or adopts an existing run
directory. Missing parent directories are created as needed only after input
validation succeeds. In supervised mode, `proteus-batch` reserves the fresh
directory and commits its output `manifest.json` and log files before launch;
the child accepts and verifies that supervisor-owned reservation but rejects any
unexpected runner artifacts. The supervisor/child reservation mechanism is
internal, not a third public execution mode. The implementation uses the
undocumented `--internal-supervised` transport flag; it is not a supported
user-facing invocation and succeeds only against a matching fresh reservation.

## 7. Run Identity and Provenance

`run_id` is the external human-readable identity. The runner also computes a canonical input digest from the normalized manifest fields that affect simulation or observation.

For schema `0.2.0`, the digest projection contains, in order:

1. `runner_schema_version`
2. the complete `simulation` object in the field order shown in §6
3. one `bootstrap` object containing `programs`, sorted by `(y, x)`, followed by
   `environment`, sorted by `(y, x)`
4. `limits`
5. `observation`

`run_id` and `output_directory` are excluded because identity and storage do not
change simulation or observation results. The canonical projection is compact
UTF-8 JSON with no whitespace or trailing newline. Object fields use the fixed
order above and the nested field order shown in §6. Integers use base-10 digits
with no leading zeroes. Optional exponent fields encode `Some(k)` as the
base-10 integer `k` and `None` as the bare JSON token `null` (never as a quoted
string). Each finite `f64` is represented in the digest projection as a JSON
string containing `0x` followed by exactly 16 lowercase hexadecimal
digits from `f64::to_bits()`; negative zero is normalized to positive zero first.
This float representation is part of the runner contract and does not depend on
`serde_json`'s number formatter. The runner computes SHA-256 over those exact
bytes and renders `sha256:<lowercase hex>`. Array sorting means reordering
independent bootstrap entries does not change the digest. A change to this
canonical serialization requires a new runner schema version. Callers should
normally treat the digest as an opaque runner-produced value.
The child and supervisor must call one shared normalization-and-digest library
function; independent implementations are forbidden.

For the complete example manifest in §6, the canonical digest projection is the
following single line (the displayed line ending is not part of the bytes):

```text
{"runner_schema_version":"0.2.0","simulation":{"width":64,"height":64,"seed":42,"r_energy":"0x3fd0000000000000","r_mass":"0x3fa999999999999a","d_energy_log2":7,"d_mass_log2":7,"t_cap":"0x4010000000000000","maintenance_rate_log2":7,"maintenance_exponent":"0x3ff0000000000000","local_action_exponent":"0x3ff0000000000000","n_synth":1,"inert_grace_ticks":10,"p_spawn_log2":null,"mutation_base_log2":16,"mutation_background_log2":8},"bootstrap":{"programs":[{"x":32,"y":24,"code":[80,100],"free_energy":20,"free_mass":12}],"environment":[{"x":31,"y":24,"free_energy":20,"free_mass":12,"bg_radiation":0,"bg_mass":0}]},"limits":{"ticks":10000},"observation":{"every_n_ticks":50}}
```

Its required digest is
`sha256:6eec9c637d8967472f2b9712ee52014968086285e5fc14ad32cba13d38fe9292`.

Build provenance describes the executable that performs the simulation. Capture
it at build time, not by inspecting a possibly unrelated checkout at launch:

- engine crate version and simulator spec version
- Git commit, or `null` when unavailable
- whether the source worktree was dirty at build time, or `null` when unavailable
- source digest, always present and independent of Git metadata
- Cargo profile, enabled Cargo features, target triple, and Rust compiler version

`source_digest` is SHA-256 over a canonical headless-build source inventory
rooted at the Rust crate. The inventory contains `Cargo.toml`, `Cargo.lock`,
`build.rs` when present, and every non-hidden regular file below `src/` except
`src/web/` and `src/bin/proteus-server.rs`. Paths are UTF-8, relative to the
crate, `/`-separated, and sorted bytewise. For each file, hash the path length as
an unsigned 64-bit big-endian integer, the path bytes, the content length in the
same encoding, then the raw content bytes. Render the result as
`sha256:<lowercase hex>`. A selected non-regular file or non-UTF-8 path is a
build error. This gives Git-less builds a meaningful identity without making a
web-only asset such as `src/web/smoke_test.html` invalidate headless resume.
The build script must emit `cargo:rerun-if-changed` for every selected file and
inventory directory so incremental builds cannot retain a stale digest.

Launch and completion provenance collectively contain runtime facts: resolved
executable path, start and finish UTC timestamps, monotonic wall-clock duration,
and engine thread count. They also record `execution_digest`, the SHA-256 digest
of the exact `proteus-run` executable bytes rendered as `sha256:<lowercase hex>`.
It is forensic provenance, not the resume compatibility key. Failure to resolve
or read `current_exe()` is a direct-run output failure and a batch preflight
failure; `execution_digest` is never `null`.

The run artifact set therefore records:

- runner schema version
- engine crate and simulator version
- build and launch provenance as defined above
- complete expanded input manifest
- canonical input digest
- execution digest
- world RNG seed
- bootstrap RNG salt/version
- start and finish timestamps
- wall-clock duration
- final tick
- termination reason

Same-version replay requires identical simulation inputs, bootstrap inputs, seed,
and execution semantics. Determinism across different engine commits or runner
schema versions is not promised. Thread count is excluded from the input digest
and build compatibility comparison: serial/Rayon parity is an engine invariant,
and the effective count remains recorded in provenance.

All output timestamps use RFC 3339 UTC strings with a `Z` suffix. Durations are
unsigned integer milliseconds measured with a monotonic clock. Feature lists are
sorted lexicographically. Paths in output records are resolved absolute UTF-8
paths.

## 8. Tick and Observation Semantics

The child tick loop has one event-recording funnel:

```text
write metrics record for tick 0 with a zero TickReport

while simulation.tick() < limits.ticks:
    report = simulation.run_tick_report()
    event_totals.record(report)

    if tick matches observation cadence:
        collect current-state metrics
        write metrics record

if final tick was not written at cadence:
    collect current-state metrics using the final TickReport
    write metrics record
```

Starting from tick 0, `limits.ticks = N` means exactly `N` calls to
`run_tick_report()` and a final `simulation.tick()` of `N`. "Final tick" always
means the post-tick counter value, not an additional tick to execute.

Every simulation tick executes even when no metrics row is written. An observation cadence of 50 means the runner records ticks 0, 50, 100, and so on; it does not skip simulation work.

Metrics retain the API v0.3.1 distinction:

- gauges such as population, resources, program sizes, packet energy, and the `census` object describe the sampled tick
- top-level birth, death, and mutation fields describe the most recently completed tick
- `event_totals` contain cumulative birth, death, and mutation counts over every completed tick

Cumulative totals prevent sampled observations from losing events between rows. The final completion summary always includes cumulative totals, even if the final tick does not match the observation cadence.

The `census` object is a gauge and has **no cumulative counterpart**. It must never be diffed across rows the way `event_totals` are diffed: the difference between two censuses is not an event count, because programs are created and destroyed between samples. Only `event_totals` carry the losslessness guarantee that survives a coarse cadence.

O(grid) aggregate metrics are computed only at observation cadence and for the final summary. The runner must not scan the grid on every tick merely to support a coarser output cadence.

Each JSONL line uses a runner envelope rather than a bare API object:

```json
{
  "runner_schema_version": "0.2.0",
  "run_id": "example-run-0001",
  "input_digest": "sha256:...",
  "execution_digest": "sha256:...",
  "metrics": {
    "epoch": 0,
    "tick": 0,
    "population": 1,
    "live_count": 1,
    "inert_count": 0,
    "total_energy": 20,
    "packet_energy": 0,
    "total_mass": 12,
    "mean_program_size": 2.0,
    "max_program_size": 2,
    "unique_genomes": 1,
    "births": 0,
    "boot_births": 0,
    "spawn_births": 0,
    "deaths": 0,
    "mutations": 0,
    "base_mutations": 0,
    "background_mutations": 0,
    "event_totals": {
      "births": 0,
      "boot_births": 0,
      "spawn_births": 0,
      "deaths": 0,
      "mutations": 0,
      "base_mutations": 0,
      "background_mutations": 0
    },
    "census": {
      "live_sizes":  { "scale": "linear", "first_value": 1, "counts": [0, 1, "…256 entries…"],
                       "overflow": 0, "count": 1, "sum": 2, "max": 2 },
      "inert_sizes": { "scale": "linear", "first_value": 1, "counts": ["…256 entries…"],
                       "overflow": 0, "count": 0, "sum": 0, "max": 0 },
      "opcode_counts": ["…256 entries, indexed by instruction byte…"],
      "size1": {
        "live_count": 0, "inert_count": 0,
        "live_age_sum": 0, "live_max_age": 0,
        "live_opcode_counts": ["…256 entries…"]
      },
      "lineage": {
        "roots": 1, "orphans": 0,
        "generation": { "scale": "linear", "first_value": 0, "counts": ["…512 entries…"], "overflow": 0,
                        "count": 1, "sum": 0, "max": 0 },
        "birth_tick_sum": 0, "birth_tick_count": 1,
        "offspring": { "scale": "linear", "first_value": 0, "counts": ["…16 entries…"], "overflow": 0,
                       "count": 1, "sum": 0, "max": 0 },
        "origin_seed": 1, "origin_spawn": 0, "origin_append": 0
      },
      "stack_depths": { "scale": "log2", "first_value": 0, "counts": ["…33 entries…"],
                        "overflow": 0, "count": 1, "sum": 0, "max": 0 }
    }
  }
}
```

The `counts` arrays above are abbreviated. Every one is written out in full in a
real row: 256 entries for each size histogram and for both opcode censuses, 512
for `generation`, 33 for the log2-scaled `stack_depths`, and 16 for `offspring`.
A row is therefore roughly 7-9 KB rather than the ~400 B of a censusless row, so
a 200 k-tick run at an observation cadence of 50-500 writes on the order of
3-36 MB of `metrics.jsonl`. Choose the cadence with that in mind; it does not change what
the final census contains.

`metrics` is the API v0.3.1 `MetricsSnapshot` object verbatim, including every
field in that schema. `census` is described in API-SPEC section 10; it is always
present in a runner row, unlike the web API where it is opt-in. The runner envelope makes lines safe to concatenate across
runs without requiring their directory context. The runner has one metrics
epoch, numbered 0. At tick 0, all top-level per-tick event fields and cumulative
event totals are zero.

## 9. Single-Run Outputs

One supervised run directory contains:

```text
<output_directory>/
├── manifest.json
├── metrics.jsonl       # after the child starts
├── summary.json        # successful child only
├── completion.json     # proteus-batch only
├── stdout.log          # proteus-batch only
└── stderr.log          # proteus-batch only
```

A direct `proteus-run` directory contains only `manifest.json`,
`metrics.jsonl`, and, after success, `summary.json`. It has no authoritative
completion record or supervisor-captured logs.

### `manifest.json`

An immutable output record containing the normalized single-run manifest,
canonical input digest, bootstrap RNG version, and build/launch provenance known
before execution. It is written before tick 0. In direct mode the child writes
it; in batch mode the supervisor writes it while reserving the directory and the
child verifies it before executing. This makes the directory recognizably
runner-owned even if the child cannot launch. This output record is distinct from
the strict input schema and is not itself accepted as a run manifest.

Schema `0.2.0` has these exact top-level fields:

| Field | Type and contents |
|---|---|
| `runner_schema_version` | String, `0.2.0` |
| `run_id` | String copied from input |
| `input_digest` | Canonical `sha256:<hex>` digest |
| `execution_digest` | SHA-256 digest of the exact `proteus-run` executable |
| `input` | Object containing normalized `simulation`, sorted `bootstrap`, `limits`, `observation`, and resolved `output_directory` |
| `execution` | Object containing `bootstrap_rng_version`, hexadecimal-string `bootstrap_rng_salt`, and `engine_threads` |
| `build` | Rebuild-stable object containing `engine_crate_version`, `simulator_spec_version`, nullable `git_commit`, nullable `source_dirty`, non-null `source_digest`, `cargo_profile`, sorted `cargo_features`, `target_triple`, and `rustc_version` |
| `launch` | Object containing resolved `executable`, resolved `source_manifest`, and `started_at` |

`bootstrap_rng_version` is `seed-program-v1` and `bootstrap_rng_salt` is
`0x0d7e8ef0426833c1` for this schema. `engine_threads` is the effective count
selected by `--threads`; it is 1 by default and for every batch child.

### `metrics.jsonl`

Append-only sampled metrics, one JSON object per line. JSONL is the MVP format because it preserves the nested cumulative-event object and remains streamable after partial failure.

The first row represents tick 0. A final row is written at the actual final tick
if cadence would otherwise omit it. Each complete line is flushed to the
operating system before the next tick begins; the runner does not `fsync` every
sample. A crash may therefore lose recently flushed data or leave one partial
final line, but all preceding complete lines remain valid JSON records. Readers
must ignore an incomplete final line.

### `summary.json`

Written by `proteus-run` only after normal completion. It contains runner schema
version, run ID, input and execution digests, final tick, final metrics,
cumulative event totals, duration, and the child's self-reported
`tick_limit_reached` termination reason.
It is useful output but is not the batch supervisor's authoritative evidence that
the process completed.

Its exact fields are:

| Field | Type and contents |
|---|---|
| `runner_schema_version` | String, `0.2.0` |
| `run_id` | String |
| `input_digest` | String |
| `execution_digest` | String |
| `build` | Exact build object copied from `manifest.json` |
| `final_tick` | `u64`, equal to `limits.ticks` |
| `termination_reason` | String, `tick_limit_reached` |
| `started_at` | RFC 3339 UTC string recorded by the child before world initialization |
| `finished_at` | RFC 3339 UTC string |
| `wall_duration_ms` | `u64` monotonic child duration |
| `final_metrics` | Complete API v0.3.1 `MetricsSnapshot` at `final_tick`, census included |

`final_metrics.event_totals` is the authoritative cumulative-event value in the
summary; it is not duplicated in a second top-level field.

### `completion.json`

Written only by `proteus-batch`, after the supervisor observes the child attempt
finish or fail to launch. A run launched without `proteus-batch` has no
supervisor completion record.

### Logs

The supervisor captures child stdout and stderr per run, so child status does
not interleave on the supervisor's terminal. The default child stderr includes
the basic `proteus-run` status described in §6; batch verbosity 0 suppresses
that normal child status. Log files must not be treated as the machine-readable
result contract.

### Durability boundary

`manifest.json`, `summary.json`, and `completion.json` use the same commit
protocol: write a temporary file in the destination directory, write the entire
JSON value, flush and `sync_all` the file, atomically rename it to the final name,
then sync the parent directory where the platform supports directory syncing.
`metrics.jsonl` is flushed per row and is flushed and `sync_all`ed before
`summary.json` is committed. Failure of a required write, file sync, or rename is
an output failure. Directory-sync support is best-effort on platforms that do
not expose it.

These guarantees target ordinary local filesystems with same-directory atomic
rename. The MVP does not claim crash-atomic behavior on network or userspace
filesystems that do not honor those primitives. Logs have no durability promise
beyond normal file I/O.

Snapshots and full-grid artifacts are deferred. They may be added after `SNAPSHOT-BOUNDARY` establishes a stable engine snapshot format.

## 10. Termination Semantics

The MVP child has one normal termination condition:

- `tick_limit_reached`

It may also end abnormally because of:

- invalid manifest or bootstrap
- output I/O failure
- engine panic
- nonzero process exit
- signal or operating-system termination
- supervisor interruption

`proteus-batch` handles its normal interrupt/termination signals. On the first
signal it stops launching jobs, requests termination of every running child,
waits up to five seconds, force-terminates remaining children, reaps them, and
writes `supervisor_interrupted` completion records. A second signal skips the
grace period but the supervisor still makes a best effort to reap children and
write records. Platform-specific signal or termination details are recorded in
the completion record. The supervisor must not deliberately leave child
processes running after it exits. A child that exited 0 and committed a valid
summary before it was reaped remains `succeeded` even when the supervisor
observed the interrupt first; completion records report the process outcome,
not the order in which polling and signal delivery happened.

The runner reports operational facts; it does not label a run scientifically as successful, extinct, frozen, or emergent.

Wall-clock limits, memory limits, extinction stops, saturation stops, and bloat stops are deferred. When added, operational resource limits must be recorded as `resource_exhausted` or another explicit operational reason—not as biological extinction.

## 11. Batch Input and Scheduling

The MVP batch format is UTF-8 JSON with the same strict-object rules as §6.
`proteus-batch` accepts `proteus-batch --manifest <path>` and an optional
`--retry-incomplete` flag. It also accepts the same `--verbosity <0|1>` and `-v`
alias as `proteus-run`. The default reports batch preflight, skips, launches,
resolved output directories, and outcomes to stderr. Verbosity 0 suppresses
normal supervisor status and is propagated to every child; failures and
interrupts are still reported. Batch manifests reference one file per run;
embedded run definitions are not supported in schema `0.2.0`.

The supervisor resolves `proteus-run` next to its own executable, using the
platform executable suffix where applicable. A custom child binary path is not
part of the MVP. Before preflight mutates any output path, the supervisor invokes
`proteus-run --build-info`. That mode prints one compact JSON object containing
`runner_schema_version`, `execution_digest`, the exact `build` object defined in
§9, and `rayon_available`, then exits. It cannot be combined with `--manifest`,
performs no simulation, and creates no run output. The supervisor requires
schema `0.2.0` and exact equality with its own build object; otherwise batch
preflight fails. The reported child execution digest is written into the run
artifacts, and the child verifies it again after launch.

The batch manifest is an exact list of single-run manifests plus a concurrency limit:

```json
{
  "runner_schema_version": "0.2.0",
  "jobs": 8,
  "runs": [
    { "manifest": "manifests/run-0001.json" },
    { "manifest": "manifests/run-0002.json" }
  ]
}
```

`runner_schema_version` must equal `0.2.0`, `jobs` is a `u32` greater than zero,
and `runs` must be nonempty. Each `manifest` path is nonempty and is resolved
relative to the batch manifest's parent directory. Each run's relative
`output_directory` remains relative to that run manifest, as defined in §6.

Batch execution exposes only run-level `jobs`; it does not expose
`threads_per_job` or attempt automatic CPU topology management. The supervisor
always launches children with `--threads 1`, so independent-run parallelism is
the ensemble strategy and `jobs` cannot accidentally multiply a per-child Rayon
pool. A direct `proteus-run` may use `--threads N` with a Rayon-enabled build to
accelerate one long run. Effective thread count is recorded but excluded from
the input digest and completion match because it must not affect results.

Before launching the first child, the supervisor parses and validates every run
manifest, computes every digest, and resolves every output path. Duplicate run
IDs are errors. Output directories are errors when their resolved paths are
equal or one is an ancestor of another. Resolve paths against a canonicalized
nearest existing ancestor so symlink aliases cannot bypass the overlap check.
An output path must not resolve to a filesystem root, and it must not equal or
contain the batch manifest or any referenced run manifest.

Each output directory also reserves a retry-archive namespace in its parent:
siblings named `<output-name>.attempt-N`, where `N` is a zero-padded decimal
integer of at least four digits (`0001`, ..., `9999`, `10000`, ...). Preflight
rejects a declared output path that equals or descends from another run's archive
namespace, and rejects a batch/run manifest located inside such a namespace.
For example, `runs/x` and `runs/x.attempt-0001` cannot appear in the same batch.
This check occurs on the resolved paths after symlink handling.

Any preflight error, including an unsafe foreign or malformed output collision,
prevents all launches and creates no output directories. Unsafe collisions are
batch-level failures rather than per-run failures because partial launch after
failed ownership validation would violate the all-or-nothing preflight boundary.
After successful preflight, the supervisor creates missing output parents as
needed and reserves each run directory immediately before its launch.

Scheduling order has no simulation semantics. Each child is deterministic from its own inputs and cannot share mutable simulation state with another child.

## 12. Completion Records and Resume

`proteus-batch` writes one authoritative `completion.json` per attempted run
after observing the child process exit, a launch failure, or supervisor-initiated
termination. A separate append-only batch index may be added for convenient
analysis, but it is not authoritative for resume.

A completion record contains a boolean `success` and one supervisor category:
`succeeded`, `child_failed`, `signaled`, `launch_failed`,
`invalid_child_output`, or `supervisor_interrupted`. Raw platform exit status or
signal information is retained separately from this portable category. A child
is successful only when it exits 0 and leaves a valid `summary.json` whose run
ID, input digest, build object, final tick, and termination reason match the
requested run. An exit-0 child with a missing or invalid summary is
`invalid_child_output`.

The exact completion fields are:

| Field | Type and contents |
|---|---|
| `runner_schema_version` | String, `0.2.0` |
| `run_id` | String |
| `input_digest` | String |
| `execution_digest` | String |
| `build` | Exact child build object |
| `success` | Boolean |
| `category` | One portable category listed above |
| `child_exit_code` | Signed integer or `null` when unavailable |
| `child_signal` | Platform signal/termination string or `null` |
| `valid_summary` | Boolean |
| `started_at` | Supervisor child-launch-attempt timestamp |
| `finished_at` | Supervisor completion timestamp |
| `wall_duration_ms` | `u64` monotonic supervisor-observed duration |
| `artifacts` | Object containing absolute `manifest`, `metrics`, `summary`, `stdout`, and `stderr` paths |

Artifact paths name the expected locations and may point to absent files after a
failure; `valid_summary` distinguishes a validated summary from a mere path.
They are launch-time provenance and are not rewritten if an output tree moves.

The supervisor, not the child, owns this record. A killed or out-of-memory child cannot reliably describe its own termination.

On every batch invocation:

1. A run is complete only when a valid `success: true` supervisor completion
   record exists and its run ID, input digest, and build object match the
   requested manifest and current child build. `execution_digest` is recorded
   but is deliberately ignored for resume so rebuilding identical sources does
   not invalidate completed work. The absolute output and artifact paths stored
   in the old records are also ignored: relocating a complete output tree does
   not change its scientific or build identity.
2. Matching completed runs are skipped, including when `--retry-incomplete` is
   present.
3. A missing directory is a new run and is launched without a retry flag.
4. An existing runner-owned directory with a missing/failed completion or a
   run-ID, input-digest, or build mismatch is incomplete. Without
   `--retry-incomplete`, it is not modified or launched, and the batch exits
   nonzero.
5. A directory is runner-owned for retry only when it contains a valid runner
   output `manifest.json` for the current runner schema. Its stored absolute
   paths describe where the attempt was launched and need not equal the current
   location. An existing directory without a valid marker is never moved or
   adopted, even with `--retry-incomplete`; the batch reports an unsafe output
   collision and aborts preflight without launching any run.
6. With `--retry-incomplete`, the supervisor atomically renames a runner-owned
   incomplete directory to the first unused sibling
   `<output-name>.attempt-N`, using the archive namespace defined in §11 and
   starting at `0001`. An existing file or directory makes a candidate occupied.
   It then reserves a new output directory and launches the requested run. No
   files are copied forward.

Direct `proteus-run` has no retry flag and never archives output. This keeps
resume and retry policy under the supervisor that owns authoritative completion.
The batch does not provide an MVP option to rerun a matching successful job.
After an interrupted batch, the normal command is to invoke the same manifest
with `--retry-incomplete`; matching completed runs are skipped and each
interrupted run is archived and restarted from tick 0.

MVP resume occurs only between whole runs. Resuming within a partially completed simulation is deferred with snapshot support.

## 13. Exit Status

Process-level behavior:

- `0`: all requested work completed successfully. For a batch, matching runs
  skipped as already complete count as successful.
- `1`: an engine, child, launch, output, or incomplete-run failure occurred.
- `2`: CLI usage, manifest parsing, manifest validation, or batch preflight
  failed before simulation work began.
- `130`: `proteus-batch` was interrupted and performed supervised shutdown.

Raw child exit codes and signals are recorded in `completion.json`; they are not
remapped to this four-code supervisor interface.

## 14. MVP Delivery Boundary

The smallest corrected MVP is:

1. Shared bootstrap module with neighborhood preload support.
2. Shared manifest, digest, and observation serialization that is usable without
   enabling the web feature.
3. A SHA-256 dependency plus build-time source and runtime executable provenance
   support for the input, source, and execution digests.
4. `proteus-run` wrapping `Simulation::run_tick_report()`.
5. Tick-limit termination only.
6. Sampled JSONL metrics using cumulative event counters.
7. Immutable manifest and final summary.
8. `proteus-batch` supervising `N` single-threaded subprocesses from
   file-referenced manifests.
9. Supervisor-owned atomic completion records.
10. Job-level resume by run ID, canonical input digest, and rebuild-stable build
    object.

Defer memory limits, wall-clock limits, batch per-child thread controls,
automatic CPU topology management, in-run checkpoints, optimizer integration,
and scientific stopping rules.

## 15. Required Tests

- A headless run and direct engine run produce the same final state for identical inputs.
- Web and headless bootstrap produce identical tick-0 worlds.
- Bootstrap programs do not count as births.
- Bootstrap array order does not change the tick-0 world or input digest.
- Environment preload/program overlap follows the documented resource override order.
- Unknown, missing, duplicate, out-of-range, and non-finite manifest values are rejected.
- Alternate JSON float spellings and normalized-manifest round trips produce the same input digest.
- The §7 golden projection produces its exact frozen SHA-256 digest.
- Source digest is stable without Git metadata and changes when any selected source byte changes.
- A tick limit of `N` performs exactly `N` complete ticks and reports final tick `N`.
- Observation cadences 1 and 100 produce identical final cumulative event totals.
- Observation cadence does not change final simulation state.
- Observation cadence does not change the final census.
- The final metrics row is emitted when the tick limit is off cadence.
- Duplicate run IDs and overlapping output directories are rejected before launch.
- A declared output cannot collide with or descend from another run's retry-archive namespace.
- `jobs` is never exceeded.
- A child panic/nonzero exit produces a supervisor-authored failure completion record.
- A killed child cannot be mistaken for success because it happened to leave a summary file.
- Resume skips only successful completions matching run ID, input digest, and the current child build object.
- Rebuilding identical sources may change `execution_digest` without invalidating resume.
- Relocating a completed output tree does not invalidate resume or rewrite its historical paths.
- Incomplete retry archives the entire old directory before launching and never reruns matching success.
- Retry refuses to move an existing directory without a valid runner ownership marker.
- Batch-relative manifest paths and run-relative output paths resolve as specified.
- Interrupting the supervisor terminates and reaps its children and writes interruption records.
- Direct runs with thread counts 1 and N produce identical state, events, and input digests.
- `proteus-run --build-info` creates no artifacts and reports the executable digest used by supervision.
- Interrupted JSONL output remains readable through its last complete line.

## 16. Deferred Contract Extensions

The MVP decisions above are closed for runner schema `0.2.0`. Later schema
versions may add embedded batch runs, an explicit successful-run replacement
mode, additional termination limits, or another metrics encoding. They must not
silently change schema `0.2.0` normalization, digest, bootstrap, retry, or output
semantics.

Questions about which configurations to try, how to score outcomes, or how to
optimize emergence remain outside the runner boundary.
