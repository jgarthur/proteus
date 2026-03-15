# docs/

This folder contains the Proteus specification and related working documentation.

## Key Files

- `SPEC.md` - The canonical specification for the Proteus simulation substrate, including the tick/pass/local-action execution model.
- `SPEC-COMPANION.md` - Operational semantics, edge-case rules, invariants, and implementation-facing clarifications for the spec.
- `SPEC-TOC.md` - Cross-reference map for finding related sections across the spec documents.
- `SPEC-CHANGELOG.md` - Semantic changes and design rationale across spec versions.
- `BACKEND-GUIDELINES.md` - Backend architecture and implementation guidance that should remain consistent with the spec.
- `BACKEND-TESTING.md` - Testing strategy covering test infrastructure, property-based testing, fuzzing, multi-tick scenarios, pass-boundary verification, parallelism correctness, and edge cases.
- `API-SPEC.md` - Provisional external API specification for the frontend/backend interaction contract. REST + WebSocket hybrid.
- `FRONTEND-SPEC.md` - Provisional frontend design specification. Technology stack, architecture, grid visualization, controls, metrics, and layout for the web-based simulation viewer.

## Key Folders

- `analysis/` - Non-authoritative working notes, critiques, and synthesis documents. See `analysis/README.md`.
