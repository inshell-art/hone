# AGENTS.md

This repo is a Vite + React + Lexical app.

## Quick Start
- `npm install`
- `npm run dev` (Vite on port 5173 by default)

## Tests
- `npm run test` runs Cypress E2E via `scripts/run-e2e.mjs` (spawns Vite on a free port).
- `npm run test:coverage` collects coverage for E2E tests.
- `npm run test:unit` runs Vitest.

## Firebase emulators / pre-push
- `.husky/pre-push` runs `npm run emu` then `npm run test` with `BASE_URL=http://localhost:5002`.
- The emulator requires the Firebase CLI (`firebase`).
- If the emulator is unavailable, you can push with `HUSKY=0`.

## Branching / merging
- Keep `main` aligned with `origin/main`; do feature work on a dedicated branch when asked (especially UI polish).
- Merge to `main` only when requested; delete the feature branch when asked.

## Linting / formatting
- `npm run lint`
- `npm run type-check`
- `npm run prettier`

## Workflow preferences
- Commit before pushing; use concise commit messages.
- Run tests when requested (E2E is slow). If the Firebase emulator is required and missing, note it or use `HUSKY=0` only when allowed.
- Prefer Vite HMR for UI tweaks; restart the dev server only if HMR misses a change.
- Avoid changing Firebase config/emulator settings unless explicitly requested.
