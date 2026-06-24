#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

today="$(date -u +%Y-%m-%d)"
checked_at="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
mkdir -p artifacts
report="artifacts/name-audit-${today}.md"

status_for_url() {
  local url="$1"
  local status
  status="$(curl -A "me-name-audit/0.5 (https://github.com/inshell-art/me)" -fsS -o /dev/null -w "%{http_code}" "$url" 2>/dev/null || true)"
  if [[ -z "$status" || "$status" == "000" ]]; then
    echo "unreachable"
  elif [[ "$status" == "200" ]]; then
    echo "present"
  elif [[ "$status" == "404" ]]; then
    echo "not found"
  else
    echo "HTTP ${status}"
  fi
}

write_entry() {
  local namespace="$1"
  local identifier="$2"
  local source="$3"
  local status="$4"
  local impact="$5"

  {
    echo "## ${namespace}: ${identifier}"
    echo
    echo "- status: ${status}"
    echo "- official source checked: ${source}"
    echo "- check time: ${checked_at}"
    echo "- release impact: ${impact}"
    echo
  } >>"$report"
}

cat >"$report" <<EOF
# ME Naming Audit

- checked_at: ${checked_at}
- policy: This report observes naming conflicts. It does not rename ME automatically.

EOF

write_entry "GitHub" "inshell-art/me" "https://api.github.com/repos/inshell-art/me" "$(status_for_url "https://api.github.com/repos/inshell-art/me")" "Must remain the source repository."
write_entry "GitHub" "inshell-art/homebrew-tap" "https://api.github.com/repos/inshell-art/homebrew-tap" "$(status_for_url "https://api.github.com/repos/inshell-art/homebrew-tap")" "Required for brew install inshell-art/tap/me."
write_entry "crates.io" "me" "https://crates.io/api/v1/crates/me" "$(status_for_url "https://crates.io/api/v1/crates/me")" "Do not publish this project as crates.io/me."
write_entry "crates.io" "me-cli" "https://crates.io/api/v1/crates/me-cli" "$(status_for_url "https://crates.io/api/v1/crates/me-cli")" "Package remains publish = false until an explicit crates.io decision."
write_entry "npm" "me" "https://registry.npmjs.org/me" "$(status_for_url "https://registry.npmjs.org/me")" "No npm distribution for unscoped me."
write_entry "npm" "@inshell-art/me" "https://registry.npmjs.org/@inshell-art%2fme" "$(status_for_url "https://registry.npmjs.org/@inshell-art%2fme")" "Future scoped package candidate only after approval."
write_entry "PyPI" "me" "https://pypi.org/pypi/me/json" "$(status_for_url "https://pypi.org/pypi/me/json")" "No PyPI distribution for me."
write_entry "PyPI" "inshell-me" "https://pypi.org/pypi/inshell-me/json" "$(status_for_url "https://pypi.org/pypi/inshell-me/json")" "Future namespaced package candidate only after approval."
write_entry "Homebrew Core" "me" "https://formulae.brew.sh/api/formula/me.json" "$(status_for_url "https://formulae.brew.sh/api/formula/me.json")" "Use fully qualified inshell-art/tap/me regardless of Core name state."

if command_path="$(command -v me 2>/dev/null)"; then
  local_status="present at ${command_path}"
else
  local_status="not found"
fi
write_entry "Local executable" "me" "command -v me" "$local_status" "Check for executable collision before public release."

echo "$report"
