# Proteus Spec TOC

Start here before opening the full spec. This file maps the major topics in `SPEC.md` to the operational clarifications in `SPEC-COMPANION.md` and the rationale history in `SPEC-CHANGELOG.md`.

## How To Use This File

- If you need the canonical rule, read `SPEC.md`.
- If you need phase ordering, conflict resolution, edge cases, or testable invariants, read the matching section in `SPEC-COMPANION.md`.
- If you need design intent or historical context, read `SPEC-CHANGELOG.md`.

## Quick Cross-Reference Map

| Topic | Read in `SPEC.md` | Then read in `SPEC-COMPANION.md` | Notes |
|---|---|---|---|
| Tick structure and pass ordering | `Execution Model` -> `Global Execution Order` | `2. Tick model at a glance`, `6. Pass-2 semantics`, `11. Arrival/decay ordering invariants` | This is the main execution-order bridge between the two docs. |
| Local budgets and one-nonlocal limit | `Execution Model` -> `Instruction Timing`, `Local Throughput` | `2. Tick model at a glance`, `6. Pass-2 semantics` | Use both when reasoning about per-tick work and queueing. |
| Program lifecycle, inert state, and newborns | `Physics` -> `Program Age`, `Program Lifecycle`, `Spontaneous Program Creation`; `Programs` -> `Newborn Program State` | `3. Entity eligibility sets`, `11. Arrival/decay ordering invariants`, `13.4 Lifecycle tests` | Covers who can act, age, mutate, or pay maintenance. |
| Mass, energy, decay, and conversion | `Physics` -> `Mass and Energy`, `Resource Pool Summary`, `Decay`, `Synthesis` | `10. Resource and conservation invariants`, `11. Arrival/decay ordering invariants`, `14. Non-obvious consequences worth remembering` | Use this path for harvest, decay, and conservation questions. |
| Maintenance and abandonment | `Physics` -> `Maintenance`, `Program Lifecycle` | `3.3 Inert programs`, `10.4 Free-resource decay threshold invariant`, `11. Arrival/decay ordering invariants`, `13.4 Lifecycle tests` | Important for live vs inert upkeep rules. |
| `Flag` behavior | `Programs` -> `Flag Semantics` | `4. Flag semantics`, `13.1 Flag tests` | The companion is the authoritative edge-case expansion. |
| Energy payment and stressed mutation | `Execution Model` -> `Energy Payment`; `Mutations` -> `Mutation Model` | `5. Cost-payment rules`, `5.4 Background-radiation-stressed mutation trigger`, `13.2 Payment tests` | Read both before changing instruction cost handling. |
| Protection and nonlocal conflict rules | `Execution Model` -> `Protection Model`, `Global Execution Order`; `Instruction Set` -> `Nonlocal` | `6. Pass-2 semantics`, `7. Pass-2 truth tables`, `13.3 Pass-2 tests` | This is the main source for protected/open target behavior. |
| Local instruction edge cases | `Instruction Set` -> local sections, `Instruction Deletion Semantics`; `Programs` -> `Stack`, `Stack-to-Instruction Truncation` | `8. Local instruction notes that matter for testing`, `9. Deletion, IP, and cursor invariants`, `13.5 Deletion tests` | Use this path for `for`/`next`, `listen`, `absorb`, deletion, and cursor motion. |
| Directed radiation and messaging | `Physics` -> `Mass and Energy`; `Instruction Set` -> `emit`, `listen`, `absorb` | `8.3 listen`, `8.4 absorb`, `10.1 Directed radiation packet invariant` | Covers energy/message coupling and packet semantics. |
| Background mass and `collect` | `Physics` -> `Mass and Energy`; `Instruction Set` -> `collect` | `10.2 Background-pool conversion invariant`, `11. Arrival/decay ordering invariants` | Use this path for mass arrival and crystallization questions. |
| Implementation structure and tests | `Implementation Notes` | `12. Suggested implementation data model`, `13. Suggested tests`, `15. Priority checklist before coding changes to the spec` | Best starting point for simulator work. |

## Annotated `SPEC.md` TOC

### `Design Philosophy`

- Purpose: high-level system goals and the `v0.2` design shift.
- Read with: `SPEC-CHANGELOG.md` for rationale history.

### `Physics`

- Covers the grid, mass and energy pools, decay, maintenance, lifecycle state, spontaneous creation, and synthesis.
- Read with: `2`, `3`, `10`, `11`, and `14` in `SPEC-COMPANION.md`.

### `Programs`

- Covers program structure, registers, newborn defaults, `Flag`, stack rules, and stack-to-instruction truncation.
- Read with: `3`, `4`, `8`, `9`, `12`, and `13.1` in `SPEC-COMPANION.md`.

### `Execution Model`

- Covers instruction timing, local throughput, payment rules, protection, and global pass order.
- Read with: `2`, `5`, `6`, `7`, and `11` in `SPEC-COMPANION.md`.

### `Instruction Set`

- Covers opcode layout, local instructions, local sensing, nonlocal instructions, and deletion semantics.
- Read with: `7`, `8`, `9`, and `13` in `SPEC-COMPANION.md`.

### `Mutations`

- Covers mutation triggers and mutation model.
- Read with: `5.4` in `SPEC-COMPANION.md`.

### `System Parameters`

- Covers tunable constants and default values.
- Read with: the matching sections above for the behavior each parameter affects.

### `Seed Replicator`

- Covers the reference organism and why it is plausible in the substrate.
- Read with: `Reference Energy/Mass Budget` and `SPEC-CHANGELOG.md` when evaluating design intent.

### `Reference Energy/Mass Budget`

- Covers the back-of-the-envelope viability analysis for the seed replicator.
- Read with: `Physics`, `Execution Model`, and `Seed Replicator`.

### `Implementation Notes`

- Covers snapshotting, Pass-2 application strategy, stochastic implementation, parallelization, and grid boundary handling.
- Read with: `12`, `13`, and `15` in `SPEC-COMPANION.md`.

### `Future Directions` and `Known Limitations`

- Covers deferred ideas and acknowledged open issues.
- Read with: `SPEC-CHANGELOG.md` when you need historical context for why something is still unresolved.
