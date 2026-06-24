# ME

ME is a local meaning environment.

ME stores what you chose to keep. It is an authorized local Cognition
store with a deterministic mutation boundary:

```text
Thought -> Decision -> Cognition
```

Codex is the application layer. It can read bounded ME context, reason
about it, compare it with ordinary local files, and draft output. ME
itself decides only whether a validated user-authorized transaction can
advance canonical state.

## What ME Stores

- Thoughts: exact input occurrences.
- Decisions: explicit user-authorized transaction inputs.
- Cognitions: Thoughts the user chose to keep.
- Trees and Snapshots: immutable canonical state history.
- A current ref, journal, integrity checks, and a derived SQLite index.

Canonical state lives in `.me/objects/**` and `.me/refs/current`.
Generated files under `views/**` are readable projections, not
authority.

## What ME Does Not Store

- Codex output automatically.
- Global relationship graphs.
- Formal Apps, App Runs, Associations, or Proposals in schema v5.
- References or Procedures as Cognitions.

To keep a sentence from a draft, bring the exact sentence back as a new
Thought and approve a Decision.

## Install

```bash
brew install inshell-art/tap/me
```

For local development from this checkout:

```bash
cargo install --path crates/me-cli --force
```

## Create A Workspace

```bash
me new ~/ME
me new /tmp/me-demo --demo
```

Then open the workspace directory in Codex App and select Local mode.

## Use ME

Inspect current state:

```bash
me --workspace ~/ME current --json
me --workspace ~/ME cognition list --json
```

Retrieve bounded context for Codex:

```bash
printf 'Draft a reply using ME.\n' > /tmp/me-task.md
me --workspace ~/ME context --task /tmp/me-task.md --limit 20 --json
me --workspace ~/ME search "authorship" --limit 20 --json
```

Read commands do not create canonical objects, append journal entries,
or advance `.me/refs/current`.

## Change ME

Capture exact text as a Thought:

```bash
printf 'Delegating execution does not necessarily delegate artistic judgment.\n' > /tmp/thought.md
me --workspace ~/ME thought capture --file /tmp/thought.md --kind idea --json
```

Then approve a Decision file and add the Thought as a Cognition:

```json
{
  "baseSnapshot": "sha256:...",
  "action": "add-cognition",
  "actor": "local-user",
  "finalBodyMarkdown": "Delegating execution does not necessarily delegate artistic judgment."
}
```

```bash
me --workspace ~/ME cognition add --thought <thought-id> --decision /tmp/decision.json --json
```

## Migrate

Schema v4 workspaces migrate in place:

```bash
me migrate --from-v4 /path/to/workspace --json
```

The migration preserves old object files, writes a v5 current Snapshot,
archives historical App material at `.me/migrations/v4-apps.json`, and
exports historical App Run output under `exports/migration/v4-app-runs/`.

## Privacy Boundary

The `me` engine is local and performs no network requests. Codex's model
may run remotely and may receive the Thought and Cognition content that
the user asks it to analyze. ME minimizes default context but cannot
make Codex inference local.

## Naming

ME is the product. `me` is the executable, `inshell-art/me` is the
repository, and `me-cli` is the local Cargo package that builds the
executable. Registry names are adapters; they do not rename the product.

See [docs/naming.md](docs/naming.md) and [docs/install.md](docs/install.md).

## Developer Checks

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run -p me-cli -- --help
scripts/naming-contract-check.sh
```

The historical browser-based Hone implementation is preserved at
`https://github.com/inshell-art/hone-legacy`.
