# frontend/

React + TypeScript frontend for observing and controlling the Proteus simulator.

## Requirements

- Node.js 20+ recommended
- npm 10+
- Proteus backend running at `http://localhost:3000` by default

Set `VITE_PROTEUS_API_URL` if the backend is hosted elsewhere.

## Install

```bash
cd frontend
npm install
```

## Run

```bash
npm run dev
```

The Vite dev server starts on `http://localhost:5173`.

## Build

```bash
npm run build
```

This produces a production bundle in `frontend/dist/`.

## Test

```bash
npm test
```

Runs the vitest suite (`vitest run`): the pure modules in `src/lib/`, plus the
component tests beside the components they cover.

Pure-module tests run in vitest's default `node` environment. Component tests
opt into jsdom per file with a `// @vitest-environment jsdom` docblock — the
`test` block is deliberately absent from `vite.config.ts`, because importing
`defineConfig` from `vitest/config` pulls vitest's bundled vite types in
alongside this project's vite 8 and breaks `tsc -b`.

`src/components/controls/SeedComposer.test.tsx` renders `ConfigEditor` in
StrictMode inside the real `SimProvider` — the composer session lives there, so
its live write-through only exists once the two are mounted together. `lib/api`,
the WebSocket context, and `lib/random` are mocked, so the tests need no backend
and no `Math.random`: every layout they assert on is reproducible.

## Library modules

`src/lib/` holds the framework-free logic the components build on:

- `opcodes.ts` — the opcode table mirrored from `rust/src/opcode.rs`, plus a
  disassembler whose output is byte-for-byte identical to the backend's
  `disassembly` strings and an assembler that accepts that output back.
- `seedLibrary.ts` — the ten built-in seed organisms with their bytes,
  descriptions, provenance, family, evidence status, and verified regime.
  Seven come from the verified ladder in
  `docs/analysis/2026-08-23_seed-organisms.md`; three are the older
  hand-designed and evolved ones. Owns `DEFAULT_LITHOTROPH_CODE`, which
  `constants.ts` re-exports.
- `populate.ts` — largest-remainder apportionment of a population total over
  per-organism weights, and deterministic scattering of the result onto
  distinct free cells from a seeded PRNG. The stream seed combines the config
  seed with the composer session's own placement seed, so the layout can be
  re-rolled without moving the run. The config editor writes the result into
  `seed_programs` live, on every change to the mix, the grid, or either seed.

Each has a `*.test.ts` beside it.

## Manual Testing

Use [TEST-PLAN.md](/Users/joey/dev/proteus-frontend/frontend/TEST-PLAN.md) for the MVP manual verification checklist.
