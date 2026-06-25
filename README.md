# ME

## What ME is

ME is a local meaning environment designed to be used through Codex App.

When a thought occurs, tell Codex:

> Add this Thought to ME: ...

ME captures the exact words. Once you approve keeping them, the Thought
becomes a Cognition in your local library.

Later, Codex can use your Cognitions for any task:

> What do I have in ME about authorship?

> Draft a reply using ME.

> Compare this decision with what I have in ME.

Codex may read, analyze, and compose from your Cognitions. Only the
`me` engine changes the canonical Cognition Library, and it does so only
from your explicit Decision.

## Install ME and open it in Codex

Install the local ME engine:

```bash
brew install inshell-art/tap/me
```

Start ME:

```bash
me start
```

ME opens a local Codex thread in your workspace.

Press Enter on:

```text
Start ME
```

## A Thought occurs

You are walking, writing, coding, or talking and something occurs to you:

> Designing a generative system is part of authorship.

Open your ME workspace in Codex and say:

> Add this Thought to ME:
> Designing a generative system is part of authorship.

Codex shows the exact text and intended operation.

After you approve it, ME stores it as a Cognition.

No existing Cognition is rewritten.

## Use what you have kept

Ask:

> What do I have in ME about artistic authorship?

Codex retrieves relevant Cognitions and explains them.

Or ask:

> Draft a short reply using ME.

Codex composes from the Cognition Library.

Reading and composing do not change ME.

## Keep something Codex produced

Codex drafts:

> Delegating execution does not necessarily delegate artistic judgment.

You decide that this sentence expresses something worth retaining.

Say:

> This is my Thought. Add it to ME.

It enters the same Thought -> approval -> Cognition flow.

## The mental model

```text
COLLECT

Something occurs to you
        |
        v
      Thought
        |
        | you approve keeping it
        v
     Cognition
```

```text
USE

Your task
  + Codex
  + relevant Cognitions
        |
        v
      Output
```

```text
KEEP

Useful Output
        |
        | "This is my Thought"
        v
      Thought
        |
        v
     Cognition
```

## Cognitions, References, and Procedures

Cognition:
Something you explicitly chose to keep in ME.

Reference:
Local material Codex may consult for a task.

Procedure:
Optional instructions for a repeated workflow.

References and Procedures are not Cognitions. Neither enters ME
automatically.

See [docs/references-and-procedures.md](docs/references-and-procedures.md).

## Privacy and local storage

ME stores canonical Cognition data locally.

The `me` engine does not use the network.

Codex's model may run remotely and receives the Cognitions that are
selected for the task you ask it to perform.

See [docs/privacy.md](docs/privacy.md).

## Advanced CLI

The intended v0.x product experience is ME with Codex App in Local mode.
The CLI is the local engine, administrative interface, and automation
contract.

Common technical commands:

```bash
me start --no-open --json
me welcome --json
me home --json
me status --json
me fsck --json
me context --stdin --json
me bundle create /tmp/me.bundle.tar
```

See [docs/cli.md](docs/cli.md).

## Development

From this source checkout:

```bash
cargo install --path crates/me-cli --force
```

Checks:

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run -p me-cli -- --help
scripts/naming-contract-check.sh
```

Naming and installation notes:

- [docs/install.md](docs/install.md)
- [docs/naming.md](docs/naming.md)
- [docs/homebrew-core-submission.md](docs/homebrew-core-submission.md)
- [docs/codex-experience.md](docs/codex-experience.md)
- [docs/mental-model.md](docs/mental-model.md)

The historical browser-based Hone implementation is preserved at
`https://github.com/inshell-art/hone-legacy`.
