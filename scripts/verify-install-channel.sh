#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

channel="$(awk -F\" '/^channel = / { print $2 }' release/install-channel.toml)"
install_command="$(awk -F\" '/^install_command = / { print $2 }' release/install-channel.toml)"

case "$channel" in
  tap)
    [[ "$install_command" == "brew install inshell-art/tap/me" ]]
    grep -q "^brew install inshell-art/tap/me$" README.md
    grep -q "^brew install inshell-art/tap/me$" docs/install.md
    if rg -n "^brew install me$" README.md docs/install.md templates >/dev/null; then
      echo "tap channel must not advertise unqualified brew install me" >&2
      exit 1
    fi
    ;;
  core)
    [[ "$install_command" == "brew install me" ]]
    if [[ "${ME_CORE_INSTALL_VERIFIED:-}" != "1" ]]; then
      echo "channel=core requires ME_CORE_INSTALL_VERIFIED=1 after clean Homebrew verification" >&2
      exit 1
    fi
    grep -q "^brew install me$" README.md
    grep -q "^brew install me$" docs/install.md
    ;;
  development)
    [[ "$install_command" == "cargo install --path crates/me-cli --force" ]]
    ;;
  *)
    echo "Unsupported install channel: $channel" >&2
    exit 1
    ;;
esac

grep -q "^me start$" README.md
grep -q "^me start$" docs/install.md

echo "install channel verified: $channel"
