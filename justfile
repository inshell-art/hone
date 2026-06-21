fmt:
    cargo fmt

check:
    cargo fmt --check
    cargo clippy --workspace --all-targets -- -D warnings
    cargo test --workspace

install-dev:
    cargo install --path crates/hone-cli --force

demo:
    rm -rf /tmp/hone-demo
    cargo run -p hone-cli -- new /tmp/hone-demo --demo

e2e: install-dev demo
    cargo test --workspace
