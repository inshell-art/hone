#!/usr/bin/env bash
set -euo pipefail

if ! command -v brew >/dev/null 2>&1; then
  echo "Homebrew is required for Core install verification." >&2
  exit 1
fi

brew_info="$(brew info me || true)"
if [[ "$brew_info" != *"inshell-art/me"* && "$brew_info" != *"Local meaning environment"* ]]; then
  echo "brew info me does not appear to resolve to ME." >&2
  exit 1
fi

brew install me

version="$(me --version)"
if [[ "$version" != ME\ * ]]; then
  echo "installed me executable did not identify as ME: $version" >&2
  exit 1
fi

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
me start --workspace "$tmp/ME" --no-open --json >/dev/null
me --workspace "$tmp/ME" fsck >/dev/null

echo "Homebrew Core install verified."
