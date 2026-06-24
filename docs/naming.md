# ME Naming

ME is the product name.

ME is a local meaning environment.

The naming layers are intentionally separate:

| Layer | Name |
|---|---|
| Product | ME |
| Descriptor | a local meaning environment |
| Repository | inshell-art/me |
| Cargo CLI package | me-cli |
| Executable | me |
| Homebrew tap repository | inshell-art/homebrew-tap |
| Homebrew install coordinate | inshell-art/tap/me |
| Workspace config | me.toml |
| Workspace state | .me/ |
| Codex skill | me |

The product stays ME even when a package registry cannot provide the exact `me` coordinate. The Cargo package is `me-cli` and publishes the `me` binary. Cargo publication is disabled until an explicit registry decision is made.

Do not publish placeholder packages for crates.io, npm, or PyPI. Do not rename ME because a registry identifier is occupied. If a future registry distribution is needed, rerun the naming audit and choose an operator-approved namespaced coordinate.

