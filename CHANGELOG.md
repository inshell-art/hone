# Changelog

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
