# rust/

This folder contains the active Rust backend implementation for Proteus.

## Key Folders

- `src/` - Rust library source for the simulator core.
- `tests/` - Integration tests for engine semantics, headless runner behavior, and the feature-gated web API surface.
- `examples/` - Standalone binaries used by checks that cannot run inside a single test binary. See `examples/parity_digest.rs`.
- `example-batch/` - Runnable four-seed batch example with one exact manifest per run.
- `scripts/` - Developer and CI check scripts. See `scripts/check-rayon-parity.sh`.

## Key Files

- `Cargo.toml` - Rust package manifest for the active backend crate.
- `AGENTS.md` - Local instructions for work in this folder.
- `README.md` - Orientation for the active Rust backend surface.
- `example-run.json` - Small runnable headless manifest matching the frontend starter's simulation and bootstrap values.
- `src/web/smoke_test.html` - Minimal browser-based smoke-test viewer served by the backend at `/debug/smoke`; keep it aligned with the current REST/WS observer surface.

## Current Crate Shape

- The default crate build is the simulator library.
- `src/observe.rs` contains read-only projections and binary/frame encoding used by external observers.
- `src/web/` contains the feature-gated REST/WebSocket API and single-simulation controller. Enable it with `--features web`.
- `src/bootstrap.rs` provides the shared deterministic program and environment initialization used by web and headless execution.
- `src/runner/` contains strict manifests, provenance, output, and supervision for headless execution.
- The web surface also serves `/debug/smoke`, a deliberately small visual diagnostic for first-run verification before the full frontend exists.
- `src/bin/proteus-server.rs` provides a small server entrypoint when the `web` feature is enabled.
- `src/bin/proteus-run.rs` executes one manifest; `src/bin/proteus-batch.rs` supervises a bounded list of those runs.

## Running Headless Jobs

The included `example-run.json` is a schema `0.2.0` manifest whose simulation
and bootstrap values match the frontend starter. From `rust/`, run it with:

```bash
cargo run --bin proteus-run -- --manifest example-run.json
```

It writes its artifacts under the ignored `target/proteus-runs/` directory.
By default, `proteus-run` reports the run ID, resolved output directory, tick
target, thread count, and completion status on stderr. Pass `--verbosity 0` or
`-v 0` to suppress successful-run status; errors are always reported.
Direct runs require a fresh `output_directory`, so change that field before
reusing the example for another run. See the complete input and output contract
in [`docs/RUNNER-SPEC.md`](../docs/RUNNER-SPEC.md).

The batch example runs the same starter scenario with seeds 1 through 4 and
launches up to four single-threaded child processes at once:

```bash
cargo build --bins
cargo run --bin proteus-batch -- --manifest example-batch/batch.json
```

Batch manifests reference one exact run manifest per child, so the example is a
small flat directory rather than one templated file. Its outputs also go under
`target/proteus-runs/`. `proteus-batch` reports preflight, launches, output
directories, and outcomes on stderr. Child status is kept in each run's
`stderr.log`, rather than interleaved on the terminal. Pass `--verbosity 0` or
`-v 0` to suppress normal supervisor status and normal child log status; errors
and interrupts are always reported.

Building all binaries together ensures `proteus-batch` can find the matching
`proteus-run` sibling it supervises. Batch children are always single-threaded.
A Rayon-enabled direct run can use `--threads N`; `--build-info` prints the
machine-readable runner build identity. Interrupted or otherwise incomplete
batches resume whole runs with `--retry-incomplete`, archiving the prior attempt
before restarting it.

## Running the Web Backend

From `rust/`:

```bash
cargo run --bin proteus-server --features web
```

This starts the full REST and WebSocket backend at `http://127.0.0.1:3000` for
the React frontend and other API clients. It also serves a standalone diagnostic
viewer at `http://127.0.0.1:3000/debug/smoke`; the smoke page is optional and is
not a separate server.

The crate's default `dev` profile is intentionally tuned for runtime speed
(`opt-level = 3`, thin LTO, `codegen-units = 1`), so plain `cargo run` is the
fast path for local simulator work.

To bind a different address/port:

```bash
cargo run --bin proteus-server --features web -- 127.0.0.1:4000
```

## Optional Parallelism

The backend also exposes an opt-in `rayon` feature for deterministic
data-parallel execution of the embarrassingly parallel passes:

```bash
cargo test --features rayon
```

The default build remains single-threaded.

### Verifying Serial/Rayon Parity

The serial and Rayon paths are mutually exclusive `cfg` blocks, so no single test
binary can compare them - a test compiled with `--features rayon` cannot see the
serial code at all. `scripts/check-rayon-parity.sh` closes that gap by building
`examples/parity_digest.rs` in both configurations and diffing full-replay digests
across sparse, moderate, dense, and moving fixtures at several thread counts:

```bash
./scripts/check-rayon-parity.sh            # 300 ticks, threads 1 2 4 8
./scripts/check-rayon-parity.sh 1000 1 16  # custom ticks and thread counts
```

It exits non-zero and prints a per-fixture diff if the two paths disagree. Run it
after touching any `#[cfg(feature = "rayon")]` block; CI runs it on every push.

The digest now covers six fixtures: sparse, frontend-style growth, dense mixed
activity, movement, one carrier for every spec-defined instruction, and a
successful/conflicting nonlocal-action fixture. It also prints activity totals so
a fixture that silently dies or stops exercising its intended behavior is visible.

### Benchmarking tick phases and grown ecologies

`examples/tick_bench.rs` reports whole-tick and per-phase wall time for six
fixtures. Its `web-256x256-single` fixture is the exact growing web scenario
recorded in `docs/analysis/2026-08-12_rayon-optimization-results.md`. An optional
fourth argument replays an untimed checkpoint once, then clones it for each timed
repetition:

```bash
cargo run --release --example tick_bench -- 1000 3 web-256x256-single
RAYON_NUM_THREADS=4 cargo run --release --features rayon --example tick_bench -- \
  100 3 web-256x256-single 6500
```

Build through `cargo run` with explicit features: serial and Rayon examples share
one output path, so directly invoking a stale `target/release/examples/tick_bench`
can benchmark the wrong feature set.
