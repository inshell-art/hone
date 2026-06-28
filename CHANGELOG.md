# Changelog

## 0.9.0

- Add the semantic state machine contract for ME's Thought -> Decision -> Cognition boundary.
- Add `me contract show` and `me contract check` for contract inspection and release validation.
- Add product-facing docs for the constitution, count design, contracts, harness boundary, and agent-tool testing.
- Add render templates, simulated agent-harness fixtures, golden UX coverage, and import-boundary regressions.
- Extend release checks to validate the semantic contract.

## 0.8.2

- Enforce the thought-to-cognition boundary in the engine: `me cognition add` now requires `approved: true`.
- Generalize the Codex skill policy so casual add, capture, save, note, remember, or put-in-ME wording captures a thought only.
- Require a separate explicit keep decision before any captured thought can become a cognition.

## 0.8.1

- Clarify the Codex skill contract so `Add this thought to ME:` captures a thought but does not approve creating a cognition.
- Require a separate explicit keep decision before Codex runs `me cognition add`.

## 0.8.0

- Tighten first-use terminology so empty onboarding starts with thoughts and introduces cognition only after the first keep action.
- Add noncanonical `.me/derived/guidance.json` for once-per-workspace progressive guidance.
- Hide Snapshot hashes and transaction internals from ordinary capture and keep success messages.
- Add first-read and multi-cognition milestone guidance without advancing canonical snapshots.
- Rewrite README, guide, workspace AGENTS, and ME skill copy around scenario-first Codex App usage.

## 0.7.0

- Add `me start` for one-command workspace resolution, derived preflight repair, and Codex App deep-link launch.
- Add `me welcome` as the canonical one-screen onboarding contract for `Start ME`.
- Shorten empty-workspace onboarding to the first Thought flow.
- Add stdin support for routine Thought capture, context retrieval, and Cognition add Decisions.
- Add user-level default workspace commands.
- Add explicit install-channel verification and Homebrew Core preparation artifacts.

## 0.6.0

- Reframe ME onboarding as a Codex-first product experience while keeping schema version 5.
- Replace `me home` with scenario-based empty and established workspace contracts that hide technical hashes.
- Replace `me guide` with a short Thought, Use, and Output-feedback tutorial.
- Update `me new` completion output to direct users to Codex App Local mode and "What can I do here?".
- Rewrite README, workspace README, AGENTS, Codex skill, and docs around the scenario-based product flow.
- Update demo workspaces with three useful Cognitions and one pending Thought.

## 0.5.0

- Move ME to schema v5 as an authorized Cognition store with a deterministic Thought -> Decision -> Cognition mutation boundary.
- Add read-only `me search`, `me context`, and `me cognition history`.
- Deprecate `me app *`, `me run *`, `me association *`, `me proposal *`, and `me cognition synthesize` with non-mutating compatibility guidance.
- Replace built-in Inspect ME and Speak for Me Apps with ordinary Procedure files.
- Add `references/` and `procedures/` to the workspace scaffold and remove Apps/Runs from generated v5 views and indexes.
- Add `me migrate --from-v4 <workspace>` with a v4 App audit archive and App Run Markdown exports.
- Update generated Codex skill instructions around General / Use ME / Change ME mode selection.

## 0.4.0

- Move ME to schema v4 with a minimal Cognition Library core: Thought to Decision to Cognition.
- Remove canonical Associations from ME Core; compatibility association commands now return guidance to use ME Apps.
- Add app-scoped analysis findings, run-scoped resolutions, and app-scoped policies.
- Add `me cognition add --thought <id> --decision <file>` for direct collection without a Proposal.
- Add `me migrate --from-v3 <workspace>` with a v3 Association audit archive.
- Add the ME naming and distribution contract: `me-cli` Cargo package, `me` binary, disabled Cargo publishing, Homebrew tap install docs, naming audit tooling, and stable version output.

## 0.3.0

- Rename the active product to ME, the explanatory expansion to Meaning Environment, and the primary binary to `me`.
- Add schema v3 objects for Thoughts, immutable Cognitions, confirmed Associations, ME Trees, ME Snapshots, ME Apps, and App Runs.
- Change the default Thought disposition to add the Thought as its own Cognition without rewriting existing Cognitions.
- Add inferred Association rebuilds, confirmed Association transactions, Cognition retirement/reactivation, optional Synthesis Cognitions, and built-in Inspect ME / Speak for Me local Apps.
- Add `me migrate --from-my-model <workspace>` for My Model v0.2 workspace migration while preserving `.my-model`.

## 0.2.0

- Rename the local CLI product to My Model and the primary binary to `my-model`.
- Add schema v2 objects for Thoughts, Cognition revisions, Integrations, Expressions, Model Trees, and Model Snapshots.
- Add `my-model migrate --from-hone <workspace>` for local v0.1 workspace migration.
- Replace primary workspace scaffold, Codex skill, views, index tables, and Homebrew template with My Model vocabulary.

## 0.1.0

- Initial local agent-native Hone implementation.
