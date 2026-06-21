# Hone Repository Rules

- This repository implements Hone as a local Rust CLI.
- Keep Hone itself network-free: no telemetry, update checks, HTTP clients, cloud SDKs, model SDKs, wallet libraries, or blockchain libraries.
- User workspaces are not Git-backed. Do not use Git commits, branches, or worktrees as semantic state.
- Canonical semantic state lives in `.hone/objects/**` and `.hone/refs/current`.
- Generated workspace files under `views/**` are projections, not authority.
- Model output is a Proposal, never authority. Approval requires an explicit Decision.
