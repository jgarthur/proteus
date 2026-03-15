# docs/ — Agent Instructions

## What's here

- **SPEC.md** — Canonical system rules for Proteus. Physics, programs, execution model, instruction set, resource budgets, parameters. This is the authority on what the substrate *is*.
- **SPEC-OPS-COMPANION.md** — Operational companion to the spec. Implementation-facing: tick ordering, truth tables, Flag semantics, Pass-2 resolution, edge cases, invariants, suggested tests. Derived from the spec, not independent of it.
- **SPEC-TOC.md** — Annotated table of contents with section numbers and cross-references between SPEC.md and the companion. **Read this first.**
- **CHANGELOG.md** — Record of spec changes.
- **MY-TODO.md** - Developer's todo list. You shouldn't need to reference this.
- **analysis/NOTEBOOK.md** — Append-only lab notebook. Observations, experiment notes, hypotheses, results. Newest entries at top.
- **analysis/** — Working analysis files. May be messy or outdated.
- **BACKEND-GUIDELINES.md** — Rust backend architecture and conventions. Grid layout, pass structure, RNG strategy, output formats, build order. Reference for new implementation work and significant refactors.

## How to navigate the spec

1. Always read **SPEC-TOC.md** first to find relevant sections.
2. Read only the sections of SPEC.md and SPEC-OPS-COMPANION.md needed for the current task.
3. Do not read the full spec into context unless explicitly asked.

The TOC includes cross-references between the two documents. If you are working on a topic covered in both (e.g., Pass-2 semantics), read both relevant sections.

## Source of truth

SPEC.md and SPEC-OPS-COMPANION.md have **equal authority**. The spec defines rules; the companion defines operational semantics and edge-case behavior for those same rules. If they contradict each other, flag it — do not silently prefer one over the other.

CHANGELOG.md records *why* things changed. Consult it when a rule seems surprising or when context on design intent would help.

## When editing the spec

Spec changes are high-stakes. Follow these rules:

- **Always check both documents.** If a change to SPEC.md affects behavior covered in the companion, update both or flag what still needs updating.
- **Use the TOC cross-references** to find companion sections affected by a spec edit.
- **Log every semantic change in CHANGELOG.md** with: what changed, why, and what alternatives were considered if relevant.
- **Do not rewrite sections you weren't asked to change.** Spec edits should be minimal and targeted.
- **If you notice an inconsistency** between the spec and companion while working on something else, mention it but do not fix it unless asked.

## When implementing the backend against the spec

- Read **BACKEND-GUIDELINES.md** before starting new implementation work or significant refactors. It covers grid layout, pass architecture, RNG strategy, struct design, and build conventions.
- Cite the specific section number you're implementing from.
- If the spec is ambiguous, check the companion — it likely has a truth table or edge-case rule that resolves it.
- If both are ambiguous, ask. Do not guess at intended semantics.
- The companion's §13 (Suggested Tests) is a good checklist for what invariants to preserve.

## Task scoping

Keep tasks bounded. Good tasks reference specific spec sections and have clear deliverables. If a task requires reading most of the spec to understand, it probably needs to be broken down.

Do not combine spec edits with implementation work in the same task unless the scope is very small.

## analysis/ folder

Treat analysis/NOTEBOOK.md as append-only. Add new entries at the top with a date and brief context. Do not reorganize or edit old entries.

Other files in analysis/ may be rough or outdated. Do not assume they reflect current spec semantics without checking.