# rust/

This folder contains the active Rust backend implementation for Proteus.

## Key Folders

- `src/` - Rust library source for the simulator core.
- `tests/` - Integration tests for engine semantics and the feature-gated web API surface.

## Key Files

- `Cargo.toml` - Rust package manifest for the active backend crate.
- `AGENTS.md` - Local instructions for work in this folder.
- `README.md` - Orientation for the active Rust backend surface.

## Current Crate Shape

- The default crate build is the simulator library.
- `src/observe.rs` contains read-only projections and binary/frame encoding used by external observers.
- `src/web/` contains the feature-gated REST/WebSocket API and single-simulation controller. Enable it with `--features web`.
- `src/bin/proteus-server.rs` provides a small server entrypoint when the `web` feature is enabled.
