fmt:
    cargo fmt

check:
    cargo fmt --check
    cargo clippy --workspace --all-targets -- -D warnings
    cargo test --workspace
    scripts/verify-install-channel.sh

verify-install-channel:
    scripts/verify-install-channel.sh

install-dev:
    cargo install --path crates/me-cli --force

demo:
    rm -rf /tmp/me-demo
    cargo run -p me-cli -- new /tmp/me-demo --demo

e2e: install-dev demo
    cargo test --workspace
