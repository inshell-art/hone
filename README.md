# Hone

Hone is a local system for refining thought over time.

Thoughts arrive. Facets endure. Hone records how one becomes the other.

Install Hone, create a workspace, and open that directory in Codex App.
Codex helps compare new thoughts with your current facets. Hone validates
the proposal and changes current meaning only after your explicit approval.

Your sources, facets, history, and snapshots remain in your local directory.
Hone itself does not use the network.

This repository is a clean-room local agent-native implementation. The
historical browser-based implementation is preserved at
`https://github.com/inshell-art/hone-legacy`.

## Install For Development

```bash
cargo install --path crates/hone-cli --force
```

## Create A Workspace

```bash
hone new ~/Hone
hone new /tmp/hone-demo --demo
```

Then open the workspace directory in Codex App and select Local mode.

## Privacy Boundary

Hone stores data locally and the executable performs no network requests.
Codex's model may run remotely and may receive the source and facet content
that the user asks it to analyze. Hone minimizes default context but cannot
make Codex inference local.

## Developer Checks

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run -p hone-cli -- --help
```
