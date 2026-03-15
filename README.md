# Proteus

Proteus is an artificial life simulator specified as a discrete 2D world where self-replicating programs compete for mass, energy, space, and time. The core idea is that programs are not layered on top of the physics; program instructions and computational work are part of the substrate itself.

This README is a high-level orientation only. It was updated for spec `v0.2.0`.

## Project Orientation

- `docs/` contains the active specification and related working documentation. See `docs/README.md`.
- `legacy/` contains archived implementation prototypes and experiments that are kept for reference during the upcoming rewrite. See `legacy/README.md`.

## Current Spec Shape

At a high level, the `v0.2.0` spec defines:

- a 2D cellular world with only local adjacency
- conserved internal transfers of mass and energy, driven by external ambient inputs
- single-cell programs with mutable instruction sequences, local execution budgets, and one nonlocal action per tick
- explicit lifecycle states for live, inert, abandoned, and newborn programs
- emergent ecology through generic read, write, transfer, deletion, signaling, and harvesting primitives

## Implementation Note

The current center of gravity is the spec in `docs/`. Earlier Rust, Python, and frontend implementations now live under `legacy/` as historical reference material. A new implementation surface has not been established yet, but the intended direction remains a Rust backend for simulation plus a frontend for visualization and interaction.
