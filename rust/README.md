# rust/

This folder contains the active Rust backend implementation for Proteus.

## Key Folders

- `src/` - Rust library source for the simulator core.
- `tests/` - Integration tests for engine semantics, headless runner behavior, and the feature-gated web API surface.
- `examples/` - Standalone binaries used by checks that cannot run inside a single test binary. See `examples/parity_digest.rs`.
- `scripts/` - Developer and CI check scripts. See `scripts/check-rayon-parity.sh`.

## Key Files

- `Cargo.toml` - Rust package manifest for the active backend crate.
- `AGENTS.md` - Local instructions for work in this folder.
- `README.md` - Orientation for the active Rust backend surface.
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

Use a schema `0.1.0` manifest as defined in `docs/RUNNER-SPEC.md`:

```bash
cargo run --bin proteus-run -- --manifest ../manifests/run.json
cargo build --bins
./target/debug/proteus-batch --manifest ../manifests/batch.json
```

Building all binaries together ensures `proteus-batch` can find the matching
`proteus-run` sibling it supervises. Batch children are always single-threaded.
A Rayon-enabled direct run can use `--threads N`; `--build-info` prints the
machine-readable runner build identity. Interrupted or otherwise incomplete
batches resume whole runs with `--retry-incomplete`, archiving the prior attempt
before restarting it.

## Running The Smoke Test

From `rust/`:

```bash
cargo run --features web --bin proteus-server
```

The crate's default `dev` profile is intentionally tuned for runtime speed
(`opt-level = 3`, thin LTO, `codegen-units = 1`), so plain `cargo run` is the
fast path for local simulator work.

Then open `http://127.0.0.1:3000/debug/smoke`.

To bind a different address/port:

```bash
cargo run --features web --bin proteus-server -- 127.0.0.1:4000
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
