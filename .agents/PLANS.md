# Plans

A plan is an implementation document written during deliberation and consumed during execution. It bridges the gap between "we decided what to do" and "now do it in a fresh context."

## What a plan captures

- **What to build and why.** The deliverable, the motivation, and the intended outcome.
- **Settled design decisions.** The choices that came out of deliberation, stated as facts, not options. Include the rationale inline — the plan may outlive the conversation that produced it.
- **Key files to modify.** Full paths. Name functions and modules when precision matters.
- **Existing code to reuse.** Point to what's already there so the executor doesn't reinvent it.
- **Verification.** What to run and what to observe to confirm the work is correct. Not "the code compiles" but concrete observable outcomes: commands, expected output, documents to check against a section list.

## Living document

Plans are living documents. If the executing agent hits a surprise or changes approach mid-task, update the plan to reflect reality. The plan should always describe the current state of the work, not just the original intent.

This matters most for multi-session tasks. A future context picking up the work has only the plan and the working tree — if the plan is stale, the next agent will repeat mistakes or diverge.

When updating a plan during execution:

- Revise the affected section in place rather than appending notes at the bottom.
- If a design decision changed, update the decision and its rationale together.
- If new files or steps were discovered, add them where they belong structurally.

## Writing guidance

**Be concrete, not exhaustive.** Name files, functions, and field names. Skip boilerplate that the executor can infer from reading the code. A plan that says "add a `max_fps` field to the WebSocket subscription message" is better than one that explains what WebSocket is.

**Lean on existing docs.** The repo has specs, guidelines, and companion documents. Reference them by path (`docs/SPEC.md §8`) rather than restating their contents. The executor can read files — don't paste 200 lines of spec into the plan.

**Put rationale next to decisions.** Don't separate the *what* from the *why*. "REST for control, WebSocket for streaming — because REST gives us standard HTTP semantics for request/response and WS avoids polling for continuous data" is one thought, not two sections.

**Scope to what the executor needs.** A plan for writing a spec document doesn't need a code architecture section. A plan for implementing a feature doesn't need design philosophy. Match the plan's shape to the task.

## When to create a plan

Not every task needs one. A plan is worth writing when:

- The task spans multiple files or systems and the executor needs orientation
- Design decisions were made during deliberation that would be lost on context clear
- The work is large enough that it might span multiple sessions
- There's a non-obvious verification step the executor wouldn't think to do

Single-file changes, bug fixes with obvious verification, and tasks fully described by a handoff prompt don't need a separate plan.
