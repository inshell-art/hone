# ME Repository Rules

- This repository implements ME as a local Rust CLI.
- Keep ME itself network-free: no telemetry, update checks, HTTP clients, cloud SDKs, model SDKs, wallet libraries, or blockchain libraries.
- User workspaces are not Git-backed. Do not use Git commits, branches, or worktrees as semantic state.
- Canonical semantic state lives in `.me/objects/**` and `.me/refs/current`.
- Generated workspace files under `views/**` are projections, not authority.
- Model output is a Proposal, never authority. Approval requires an explicit Decision.
