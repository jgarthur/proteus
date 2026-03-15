# Repo-wide Agent Instructions

## README.md files

- Read the entire `README.md` file present when entering a folder for the first time. It exists to help orient the work.
- Keep the `README.md` at the repo root as a high-level orientation to the project. Describe top-level folders at a high level, not individual files inside those folders.
- Put detailed file and subfolder descriptions in the local `README.md` for that folder.

## Versioning

- Use semantic versioning: `x.y.z`.
- After a change to the `y` version, update the top-level `README.md` so it stays aligned with the current spec generation.

## Failure Analysis

- When a test fails, explain the failure explicitly before or alongside the fix.
- Reason from first principles when possible: describe the relevant execution steps, invariants, or data-flow that produce the observed result rather than just naming the changed assertion or implementation detail.
