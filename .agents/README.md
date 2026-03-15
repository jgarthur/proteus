# .agents/

Working directory for agent-assisted tasks. Not automation infrastructure — just files that help hand work between conversation contexts.

## Why this exists

Claude Code conversations have finite context. Complex tasks often span multiple sessions: you deliberate in one, execute in the next, review in a third. These files bridge the gaps.

## Folders

### `prompts/`

Handoff prompts. Each file scopes a task for a fresh context that has no memory of prior conversations.

A good handoff prompt:

- States the deliverable and where it goes
- Lists source documents in priority order with authority rules (what overrides what)
- Bakes in decisions already made — don't re-deliberate settled questions
- Defines scope and explicit non-goals
- Specifies the shape of the output document
- Sets a task boundary (what files may be edited)
- Tells the agent what to do when it hits a conflict: note it as an open question, don't silently resolve it
- Ends with observable verification: what to run, what to check, how to know the task is done

The pattern that produced these: start a conversation, read the sources together, deliberate on design questions, settle the answers, then write the handoff prompt that encodes those answers as constraints. The next context gets a clean task instead of a messy exploration.

Delete handoff prompts after they're completed. They're instructions, not records.

### `plans/`

Implementation plans. Written during deliberation, consumed during execution. See `PLANS.md` for guidance on writing and maintaining them.

## Patterns

**Deliberate then hand off.** Don't try to deliberate and execute in the same context if the task is large. Deliberation is cheap and benefits from back-and-forth. Execution benefits from a clean, focused context with clear instructions.

**Encode decisions as constraints.** If you spent 20 minutes deciding between REST+WS hybrid vs pure WebSocket, the handoff prompt should say "REST for control, WebSocket for streaming" as a fixed constraint, not present it as an open question again.

**Put rationale in the artifacts, not in a log.** Decision rationale belongs in the documents the decisions produce — spec files, design docs, code comments at non-obvious points. Handoff prompts encode the *what* as constraints. Specs explain the *why* in context. Don't create a separate decision log that nobody maintains.

**Source hierarchy matters.** When multiple docs describe the same thing, be explicit about which one wins. "SPEC.md is authoritative, GUIDELINES.md is advisory" prevents the next context from making the wrong call.

**Scope tightly.** A handoff that says "design the frontend" is worse than one that says "create `docs/FRONTEND-SPEC.md` covering these 13 sections, don't edit Rust code." Tight scope means the agent can finish without guessing.

**Verify observably.** Every handoff and plan should answer: how does the executor know they're done? Not "the code compiles" but "the document covers all 16 sections, non-goals from the handoff are excluded, and `docs/README.md` is updated." For code tasks: what commands to run, what output to expect.
