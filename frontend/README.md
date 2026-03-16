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

## Manual Testing

Use [TEST-PLAN.md](/Users/joey/dev/proteus-frontend/frontend/TEST-PLAN.md) for the MVP manual verification checklist.
