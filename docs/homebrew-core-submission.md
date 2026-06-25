# Homebrew Core Submission

ME can switch the public install command to:

```bash
brew install me
```

only after the Homebrew Core formula is accepted and clean-install
verification passes.

## Required Evidence

- Stable tagged release.
- MIT license.
- Source build from the release archive.
- Formula test creates a workspace, runs `fsck`, and checks `welcome`.
- No self-update behavior.
- No unversioned install downloads.
- Public homepage and README.
- Evidence of use beyond the author, or an accepted exception.
- `brew info me` resolves to the ME formula.
- `brew install me` installs the intended `me` executable.
- No conflicting Core formula or linked executable is selected.

## Local Preparation

The draft formula lives at:

```text
packaging/homebrew-core/me.rb
```

Do not submit automatically.

After Core acceptance and clean-install verification, update:

```text
release/install-channel.toml
```

to:

```toml
channel = "core"
install_command = "brew install me"
```

Then run:

```bash
ME_CORE_INSTALL_VERIFIED=1 scripts/verify-install-channel.sh
```
