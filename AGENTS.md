# ME Repository Rules

- This repository implements ME as a local Rust CLI.
- ME is a local meaning environment designed to be used through Codex App.
- Codex is the interface; `me` is the local engine that safely reads and changes ME.
- Keep ME itself network-free: no telemetry, update checks, HTTP clients, cloud SDKs, model SDKs, wallet libraries, or blockchain libraries.
- User workspaces are not Git-backed. Do not use Git commits, branches, or worktrees as semantic state.
- Canonical semantic state lives in `.me/objects/**` and `.me/refs/current`.
- Generated workspace files under `views/**` are projections, not authority.
- Model output is never authority. Canonical change requires a Thought and an explicit Decision.
- `me search` and `me context` are read-only and must not create canonical objects or advance the current Snapshot.
- References and Procedures are ordinary local files, not Cognitions.
- App, Run, Association, Proposal, and Synthesis commands are legacy compatibility only in schema v5.
