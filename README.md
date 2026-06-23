# ME

ME is your local Meaning Environment.

Tell ME a Thought. It becomes a Cognition only when you choose to add it. ME keeps Cognitions loosely: they may overlap, recur, qualify, or contradict one another without being forced into one canonical statement.

ME Apps apply explicit domain rules when you use those Cognitions for a task. An App can inspect ME, compose a draft, or support another local workflow. App Outputs never become Cognitions automatically.

ME stores its Cognition Library and history in your local workspace. The `me` engine itself does not use the network.

ME is not a digital clone, a complete identity, or an agent authorized to act as you.

The historical browser-based Hone implementation is preserved at `https://github.com/inshell-art/hone-legacy`.

## Install For Development

```bash
cargo install --path crates/me-cli --force
```

## Create A Workspace

```bash
me new ~/ME
me new /tmp/me-demo --demo
```

Then open the workspace directory in Codex App and select Local mode.

## Privacy Boundary

ME stores data locally and the executable performs no network requests. Codex's model may run remotely and may receive the Thought and Cognition content that the user asks it to analyze. ME minimizes default context but cannot make Codex inference local.

## Developer Checks

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run -p me-cli -- --help
```
