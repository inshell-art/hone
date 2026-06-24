#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

metadata="$(cargo metadata --format-version 1 --no-deps)"

python3 - "$metadata" <<'PY'
import json
import sys

metadata = json.loads(sys.argv[1])
packages = metadata["packages"]
names = {package["name"] for package in packages}
expected = {"me-core", "me-markdown", "me-index", "me-store", "me-cli"}

missing = expected - names
if missing:
    raise SystemExit(f"missing Cargo packages: {sorted(missing)}")

if "me" in names:
    raise SystemExit("Cargo package must not be named me")

for package in packages:
    if package["name"] in expected and package.get("publish") != []:
        raise SystemExit(f"{package['name']} must set publish = false")

cli = next(package for package in packages if package["name"] == "me-cli")
if not any(target["name"] == "me" and "bin" in target["kind"] for target in cli["targets"]):
    raise SystemExit("me-cli must publish a binary target named me")
PY

version_output="$(cargo run -q -p me-cli -- --version)"
if [[ "$version_output" != ME\ * ]]; then
  echo "me --version must begin with ME; got: $version_output" >&2
  exit 1
fi

version_json="$(cargo run -q -p me-cli -- version --json)"
python3 - "$version_json" <<'PY'
import json
import sys

data = json.loads(sys.argv[1])
expected = {
    "product": "ME",
    "descriptor": "a local meaning environment",
    "binary": "me",
    "cargoPackage": "me-cli",
    "workspaceSchema": 5,
}
for key, value in expected.items():
    if data.get(key) != value:
        raise SystemExit(f"version JSON {key} mismatch: {data.get(key)!r}")
PY

grep -qx "# ME" README.md
grep -q "ME is a local meaning environment." README.md
grep -q "brew install inshell-art/tap/me" README.md
grep -q "brew install inshell-art/tap/me" docs/install.md
grep -q 'desc "Local meaning environment"' packaging/homebrew/me.rb.template
grep -q 'class Me < Formula' packaging/homebrew/me.rb.template
grep -q 'std_cargo_args(path: "crates/me-cli")' packaging/homebrew/me.rb.template

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
cargo run -q -p me-cli -- new "$tmp/ME" --json >/dev/null
test -f "$tmp/ME/me.toml"
test -d "$tmp/ME/.me"
test -d "$tmp/ME/.agents/skills/me"
test ! -e "$tmp/ME/hone.toml"
test ! -e "$tmp/ME/.hone"
test ! -e "$tmp/ME/my-model.toml"
test ! -e "$tmp/ME/.my-model"

if rg -n "My Ego|the tap/me product|the me-cli app" README.md docs templates crates >/dev/null; then
  echo "Found prohibited product-facing naming copy." >&2
  exit 1
fi

if rg -n --glob '!scripts/naming-contract-check.sh' "npm publish|twine upload|cargo publish" .github scripts Cargo.toml crates docs README.md >/dev/null 2>&1; then
  echo "Found publication workflow or command that needs review." >&2
  exit 1
fi

echo "naming contract checks passed"
