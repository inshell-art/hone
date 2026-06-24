# ME Naming Audit

- checked_at: 2026-06-24T09:19:17Z
- policy: This report observes naming conflicts. It does not rename ME automatically.

## GitHub: inshell-art/me

- status: present
- official source checked: https://api.github.com/repos/inshell-art/me
- check time: 2026-06-24T09:19:17Z
- release impact: Must remain the source repository.

## GitHub: inshell-art/homebrew-tap

- status: not found
- official source checked: https://api.github.com/repos/inshell-art/homebrew-tap
- check time: 2026-06-24T09:19:17Z
- release impact: Required for brew install inshell-art/tap/me.

## crates.io: me

- status: present
- official source checked: https://crates.io/api/v1/crates/me
- check time: 2026-06-24T09:19:17Z
- release impact: Do not publish this project as crates.io/me.

## crates.io: me-cli

- status: not found
- official source checked: https://crates.io/api/v1/crates/me-cli
- check time: 2026-06-24T09:19:17Z
- release impact: Package remains publish = false until an explicit crates.io decision.

## npm: me

- status: present
- official source checked: https://registry.npmjs.org/me
- check time: 2026-06-24T09:19:17Z
- release impact: No npm distribution for unscoped me.

## npm: @inshell-art/me

- status: not found
- official source checked: https://registry.npmjs.org/@inshell-art%2fme
- check time: 2026-06-24T09:19:17Z
- release impact: Future scoped package candidate only after approval.

## PyPI: me

- status: present
- official source checked: https://pypi.org/pypi/me/json
- check time: 2026-06-24T09:19:17Z
- release impact: No PyPI distribution for me.

## PyPI: inshell-me

- status: not found
- official source checked: https://pypi.org/pypi/inshell-me/json
- check time: 2026-06-24T09:19:17Z
- release impact: Future namespaced package candidate only after approval.

## Homebrew Core: me

- status: not found
- official source checked: https://formulae.brew.sh/api/formula/me.json
- check time: 2026-06-24T09:19:17Z
- release impact: Use fully qualified inshell-art/tap/me regardless of Core name state.

## Local executable: me

- status: present at /Users/bigu/.asdf/shims/me
- official source checked: command -v me
- check time: 2026-06-24T09:19:17Z
- release impact: Check for executable collision before public release.

