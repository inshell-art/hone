#!/usr/bin/env bash
set -euo pipefail

cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run -p hone-cli -- --help >/dev/null
