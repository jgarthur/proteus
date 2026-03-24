# Proteus

Proteus is an artificial life simulator specified as a discrete 2D world where self-replicating programs compete for mass, energy, space, and time. The core idea is that programs are not layered on top of the physics; program instructions and computational work are part of the substrate itself.

This README is a high-level orientation only. It was updated for spec `v0.3.1`.

*Proteus was designed by Joey Arthur with inspiration from Tom Ray's [Tierra](https://tomray.me/pubs/doc/index.html) and Max Robinson's [The Life Engine](https://thelifeengine.net/). There is been much AI assistance, especially for surfacing edge cases and contradictions in the [main spec](docs/SPEC.md), which started as a hand-written document but has taken on a life of its own (ha!). The code itself has been mostly written by agents, at breakneck speed.*

## Project Orientation

- `STATUS.md` tracks what is done and what is next. Keep it current.
- `docs/` contains the active specification and related working documentation. See `docs/README.md`.
- `frontend/` contains the active React + TypeScript simulator UI. See `frontend/README.md`.
- `rust/` contains the active Rust backend implementation surface. See `rust/README.md`.
- `legacy/` contains archived implementation prototypes and experiments that are kept for reference during the upcoming rewrite. See `legacy/README.md`.

## Quick Start

For local development, run the backend and frontend in separate terminals.

Backend:

```bash
cd rust
cargo run --bin proteus-server --features web
```

The Rust crate tunes the default `dev` profile for runtime speed, so plain
`cargo run` already builds an optimized simulator binary with thin LTO.

Frontend:

```bash
cd frontend
npm install
npm run dev
```

Default local endpoints:

- frontend dev server: `http://localhost:5173`
- backend API/WebSocket: `http://localhost:3000`

Basic build commands:

```bash
cd rust
cargo test
```

```bash
cd frontend
npm run build
```

## Current Spec Shape

At a high level, the `v0.3.1` spec defines:

- a 2D cellular world with only local adjacency
- conserved internal transfers of mass and energy, driven by Poisson ambient inputs and per-quantum binomial decay
- single-cell programs with mutable instruction sequences, local action budgets, and one nonlocal action per tick
- explicit lifecycle states for live, inert, abandoned, and newborn programs
- fresh simulations seeded from the stationary ambient background distribution when `D > 0`
- emergent ecology through generic read, write, transfer, deletion, signaling, and harvesting primitives

## Implementation Note

The current center of gravity is the spec in `docs/`. The active simulator backend is being rebuilt in `rust/`. Earlier Rust, Python, and frontend implementations live under `legacy/` as historical reference material.
