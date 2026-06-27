# ME

ME is a local meaning environment operated through Codex App.

When a thought occurs, tell Codex:

> Add this thought to ME:
> ...

ME captures the exact words. You choose whether to keep them.

The prompt captures first. It does not keep the thought until you approve.
Casual add, capture, save, note, or remember wording is still only
thought capture.

A thought you keep in ME is called a cognition.

Codex can inspect, compare, and compose from your cognitions
without changing them.

## Install and Start ME

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

## A thought occurs

Suppose you think:

> Designing a generative system is part of authorship.

Tell Codex:

> Add this thought to ME:
> Designing a generative system is part of authorship.

ME captures the exact text first. It is not in ME yet.

That prompt is capture intent, not approval to keep it.

## Keep the thought

Codex asks whether to keep it.

After you approve, ME adds it to the local Cognition Library.

In ME, a thought you choose to keep is called a cognition.

The local engine requires explicit keep approval before converting a
thought into a cognition.

## Use a cognition

Ask:

> What do I have in ME about authorship?

or:

> Draft a short statement using ME.

Codex may read and compose from the cognition.

Reading and composing do not change ME.

## Keep something Codex produced

If Codex writes a sentence worth retaining, say:

> This is my thought. Add it to ME.

The sentence returns through the same capture and keep flow.

## The mental model

```text
COLLECT

Something occurs to you
        |
        v
      thought
        |
        | you choose to keep it
        v
     cognition
```

```text
USE

your task
  + Codex
  + relevant cognitions
        |
        v
      output
```

```text
KEEP FROM OUTPUT

useful output
        |
        | "This is my thought"
        v
      thought
        |
        v
     cognition
```

ME is the complete product.

```text
Codex App
  host and conversational interface

ME skill
  instructions teaching Codex how to operate ME

me executable
  deterministic local engine

ME workspace
  durable Cognition Library

ME
  complete product
```

Technical documentation may describe ME as a Codex-native local domain
application. That is a descriptive architecture phrase, not an official
OpenAI platform category.

## Advanced: References and Procedures

Cognitions are thoughts you explicitly chose to keep in ME.

References are local materials Codex may consult for a task.

Procedures are optional instructions for repeated workflows.

References and Procedures are not cognitions. Neither enters ME
automatically.

See [docs/references-and-procedures.md](docs/references-and-procedures.md).

## Advanced: backup, export, and CLI

Operational commands are available when you ask for technical help,
backup, export, restore, or integrity checks.

```bash
me status --json
me fsck --json
me export /tmp/me-export.json
me bundle create /tmp/me.bundle.tar
me bundle verify /tmp/me.bundle.tar
me bundle restore /tmp/me.bundle.tar /tmp/restored-me
```

See [docs/cli.md](docs/cli.md).

## Privacy

ME stores canonical Cognition data locally.

The `me` engine does not use the network.

Codex's model may run remotely and receives the cognitions selected for
the task you ask it to perform.

See [docs/privacy.md](docs/privacy.md).

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
scripts/verify-install-channel.sh
```

Naming and installation notes:

- [docs/install.md](docs/install.md)
- [docs/naming.md](docs/naming.md)
- [docs/homebrew-core-submission.md](docs/homebrew-core-submission.md)
- [docs/codex-experience.md](docs/codex-experience.md)
- [docs/mental-model.md](docs/mental-model.md)

The historical browser-based Hone implementation is preserved at
`https://github.com/inshell-art/hone-legacy`.
