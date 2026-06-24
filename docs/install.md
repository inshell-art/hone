# Installing ME

ME is a local meaning environment.

The intended user install path is the Inshell Homebrew tap:

```bash
brew install inshell-art/tap/me
```

This installs the `me` executable from the `me` formula in the `inshell-art/homebrew-tap` repository.

For local development from this source checkout:

```bash
cargo install --path crates/me-cli --force
```

The Cargo package name is `me-cli`; the installed executable is `me`.

