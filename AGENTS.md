# AGENTS.md

This repo is a Vite + React + Lexical app.

## Quick Start
- `npm install`
- `npm run dev -- --host 127.0.0.1 --strictPort` (Vite on port 5173 by default)

## Tests
- `npm run test` runs Cypress E2E via `scripts/run-e2e.mjs` (spawns Vite on a free port).
- `npm run test:coverage` collects coverage for E2E tests.
- `npm run test:unit` runs Vitest.

## Firebase emulators / pre-push
- `.husky/pre-push` runs `npm run emu` then `npm run test` with `BASE_URL=http://localhost:5002`.
- The emulator requires the Firebase CLI (`firebase`), provided by `firebase-tools` in devDependencies.
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
- Dev server rules: always run Vite with `--host 127.0.0.1 --strictPort`; never run two servers on the same port (including IPv4/IPv6 split); if a port is occupied, stop the existing process before starting a new one.
- Avoid changing Firebase config/emulator settings unless explicitly requested.

## Security and leak-prevention rules
- Never introduce secrets into the repo.
- Do not add or modify code that includes any: private keys, seed phrases, mnemonics; service account JSON; API keys or tokens (RPC keys included); `.env` files or `.pem`/`.key` files.
- Treat any `VITE_*` env vars as public (baked into client JS). Never store secrets in them.
- Always run a leak scan before committing.
- Before proposing a commit/PR, run:
  - `git diff --staged` and manually inspect for secrets.
  - `gitleaks detect --no-git --redact` (or repo’s chosen scanner).
- If any potential secret is detected, stop and remove it; do not “mask” it.
- Do not print sensitive values in CI logs.
- Avoid adding workflow steps like `echo $TOKEN`, `printenv`, verbose debug logs that may include headers/keys.
- Avoid logging full RPC URLs if they include keys.
- No new third-party telemetry by default.
- Do not add analytics, session replay, fingerprinting, or new error trackers unless explicitly requested.
- If error tracking exists, ensure it:
  - does not capture wallet addresses or RPC payloads.
  - does not capture user identifiers.
- Protect deployment and workflow integrity:
  - Do not weaken branch protections in documentation or instructions.
  - Pin GitHub Action versions where possible.
  - Prefer least-privilege tokens and avoid long-lived credentials.
- Remove debug artifacts:
  - Before committing, ensure no debug-only endpoints, “test wallets”, or local RPC defaults ship to production configs.
  - Ensure production defaults do not point to localhost RPC.
- Security PR checklist (must pass):
  - No secrets in diff.
  - No new telemetry.
  - No new external endpoints without clear reason.
  - Build succeeds with clean env.
  - Any new config is documented and safe to be public.
