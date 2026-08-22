# Repo-wide Agent Instructions

## STATUS.md

- `STATUS.md` at the repo root is the project status log. Read it at the start of a conversation to understand what is done and what is next.
- Before working on a named status item, run `rg -n '<ITEM-NAME>' .` to find its code and documentation breadcrumbs.
- When you complete a task that corresponds to a status item, mark it done (`[x]`).
- If new follow-up work emerges, add it to the **Next** section.

## README.md files

- Read the entire `README.md` file present when entering a folder for the first time. It exists to help orient the work.
- Keep the `README.md` at the repo root as a high-level orientation to the project. Describe top-level folders at a high level, not individual files inside those folders.
- Put detailed file and subfolder descriptions in the local `README.md` for that folder.

## Versioning

- Use semantic versioning: `x.y.z`.
- After a change to the `y` version, update the top-level `README.md` so it stays aligned with the current spec generation.

## Merging

- Prefer squash merges when landing a branch into `main` unless the user explicitly asks to preserve branch history.

## Failure Analysis

- When a test fails, explain the failure explicitly before or alongside the fix.
- Reason from first principles when possible: describe the relevant execution steps, invariants, or data-flow that produce the observed result rather than just naming the changed assertion or implementation detail.

## Orchestration Notes (Proteus-Specific)

Companions to the generic `orchestrate` skill; that skill carries the strategy, this
section carries what its rules mean in this repo.

- **Draw-stream discipline is the review focus for engine work.** Determinism rests
  on per-cell RNG stream *positions*: probability-0/1 branches consume no draws, and
  any sampler change must match the old code's draw consumption at every call site or
  every later draw shifts. Structural-invariant tests (conservation, determinism,
  property, shuffled-order) must pass UNMODIFIED through any migration — if one
  fails, the change is wrong; never adjust those tests.
- **Sampled-literal reconciliation**: when draw streams legitimately change, expected
  literals move. Each update needs the Failure Analysis treatment above (plausible
  stream shift vs structural bug), and `scripts/check-rayon-parity.sh` plus
  `parity_digest` byte-comparisons are the proof mechanism for "stream-preserving"
  claims.
- **Draw-accounting tests must exercise the drawing path**: a twin-RNG test whose
  only fixture hits a no-draw shortcut (e.g. `p = 1.0`, k = 0) verifies nothing —
  cover both the k = 0 and k > 0 paths, and prefer mutation-testing new tests.
- **tick_bench A/B across behavior-changing branches**: check per-fixture world
  comparability first via the activity lines (final_programs / occupancy). Fixtures
  grown from checkpoints (dense web windows) are ecology-sensitive and can silently
  compare different worlds; the synthetic dense fixtures (100% occupancy both sides)
  and empty grids are the reliable regime anchors. Growth trajectories are bimodal
  (takeoff vs stall), so single-seed ecology comparisons are unreliable — use paired
  multi-seed ensembles (parameter changes do NOT re-roll the noise field; cell_rng
  derives from world seed/tick/cell only).
