# docs/ - Agent Instructions

This file applies to work in `docs/`.

## Status

Read `STATUS.md` at the repo root for current project status. Mark items done when you complete them.

## Source Of Truth

**SPEC.md** and **SPEC-COMPANION.md** have equal authority. The spec defines the rules. The companion defines operational semantics and edge-case behavior for those same rules. If they contradict each other, flag it instead of silently preferring one.

**BACKEND-GUIDELINES.md** should follow the spec. If it disagrees with the spec, note the mismatch but do not fix it unless asked.

## When Editing The Spec

- Always check both **SPEC.md** and **SPEC-COMPANION.md**.
- Log every semantic change in **SPEC-CHANGELOG.md** with what changed and why.
- Keep spec edits minimal and targeted.
- If you notice a spec/companion inconsistency outside the task scope, mention it but do not fix it unless asked.

## When Implementing Against The Spec

- Read **BACKEND-GUIDELINES.md** before substantial implementation work or refactors.
- Cite the specific spec section you are implementing.
- If the spec is ambiguous, check the companion.
- If both are ambiguous, ask instead of guessing.
- Use the companion's suggested tests and invariants as an implementation checklist.

## Task Scoping

Keep tasks bounded. Good tasks reference specific spec sections and have a clear deliverable. If a task seems to require most of the spec just to get started, it probably needs to be broken down first.

Do not combine large spec edits with large implementation changes in one task unless the user explicitly wants that.

## analysis/ Folder

Treat `analysis/` as supporting material, not authority. Verify any behavioral claim there against **SPEC.md** and **SPEC-COMPANION.md** before relying on it.

If `analysis/NOTEBOOK.md` exists, treat it as append-only unless the user explicitly asks for cleanup or restructuring.
