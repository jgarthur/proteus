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

### Closing experiments

- Keep one report per research question. Lead with its answer, scope, implications, limits, and disposition; update it instead of accumulating session handoffs.
- At closure, reconcile durable findings into `analysis/README.md` and catalog the detailed report in `analysis/ARCHIVE.md`. Keep speculative next steps distinct from active work.
- Mark the corresponding `STATUS.md` item done. Put only selected follow-ups in its **Next** section; park unselected ideas in `BACKLOG.md`. A closed investigation needs no successor task.
- Preserve compact evidence for conclusions and unresolved conflicts. Raw traces, per-tick dumps, copied binaries and exploratory outputs are scratch artifacts; extract needed examples before cleanup and state when original archives are unavailable. Do not regenerate bulk archives merely to restore a historical path.
- In the report or its evidence README, identify retained inputs, result tables, decisive examples and rerun commands. Record whether raw archives remain available; hashes identify files but do not substitute for retained evidence. Check references after cleanup.

Use these fields for the opening finding card:

- **Question and scope:** the test, rules version and material conditions.
- **Answer:** observed results, including negative or conflicting outcomes.
- **Meaning:** implications for emergence or the engineering decision.
- **Limits:** inference, hypothesis, and what remains untested.
- **Disposition:** active, closed, or superseded; evidence that would reopen it.

Keep the findings overview short. Link to measured results rather than carrying
forward an untested plan's estimates. Label older assumptions at their source
when a reader could mistake them for the current rules.

Use this structure inside the report; do not create a separate summary, closure
log or handoff for each session. Extend the same report while pursuing the same
question, preserving unresolved findings and distinguishing observations from
interpretation. Historical plans need a scope/status label and a link to their
results, rather than a retrospective finding card.
