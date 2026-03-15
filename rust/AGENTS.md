# rust/ - Agent Instructions

This file applies to work in `rust/`.

## Scope

- `rust/` is the active implementation surface for the new Rust backend.
- `legacy/rust/` is deprecated reference material and is not a migration target.

## Workflow

- After a batch of Rust edits, run `cargo fmt` from `rust/`.
- After a batch of Rust edits, run `cargo clippy --all-targets --all-features` from `rust/`.
- Prefer adding or updating tests alongside behavior changes.

## Spec Alignment

- `docs/SPEC.md` and `docs/SPEC-COMPANION.md` are the authoritative behavior docs.
- `docs/BACKEND-GUIDELINES.md` is implementation guidance and should stay consistent with the spec.
- Keep pass boundaries explicit in the code structure so execution-order semantics remain easy to verify.
