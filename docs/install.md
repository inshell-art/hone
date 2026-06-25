# Installing ME

ME is a local meaning environment.

The intended user install path is the Inshell Homebrew tap:

```bash
brew install inshell-art/tap/me
me start
```

This installs the `me` executable from the `me` formula in the `inshell-art/homebrew-tap` repository and starts ME in Codex App.

Press Enter on:

```text
Start ME
```

For local development from this source checkout:

```bash
cargo install --path crates/me-cli --force
me start --workspace /tmp/ME-Lab
```

The Cargo package name is `me-cli`; the installed executable is `me`.
