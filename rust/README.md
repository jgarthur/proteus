# rust/

This folder contains the active Rust backend implementation for Proteus.

## Key Folders

- `src/` - Rust library source for the simulator core.
- `tests/` - Integration tests for engine semantics and the feature-gated web API surface.

## Key Files

- `Cargo.toml` - Rust package manifest for the active backend crate.
- `AGENTS.md` - Local instructions for work in this folder.
- `README.md` - Orientation for the active Rust backend surface.
- `src/web/smoke_test.html` - Minimal browser-based smoke-test viewer served by the backend at `/debug/smoke`; keep it aligned with the current REST/WS observer surface.

## Current Crate Shape

- The default crate build is the simulator library.
- `src/observe.rs` contains read-only projections and binary/frame encoding used by external observers.
- `src/web/` contains the feature-gated REST/WebSocket API and single-simulation controller. Enable it with `--features web`.
- The web surface also serves `/debug/smoke`, a deliberately small visual diagnostic for first-run verification before the full frontend exists.
- `src/bin/proteus-server.rs` provides a small server entrypoint when the `web` feature is enabled.

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
