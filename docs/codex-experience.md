# ME With Codex

ME with Codex App is the intended v0.x product experience.

Codex is the interface and reasoning layer.

ME is the trusted local Cognition Library.

The `me` command is the engine that safely reads and changes ME.

## Local Mode

Create a workspace with:

```bash
me start
```

ME opens a new local Codex thread in the selected workspace.

Press Enter on:

```text
Start ME
```

Codex should call `me welcome --json` and render `renderedMarkdown`
verbatim.

## Reading

When you ask Codex to inspect, compare, or compose from ME, Codex writes
your transient task to standard input and calls:

```bash
me context --stdin --json
```

The selected Cognitions are user-authorized context. Codex must
distinguish those Cognitions from its own inference.

## Changing

When you ask to capture something in ME, Codex preserves the exact text,
captures it as a thought, says it is not in ME yet, and waits for a
separate keep decision before creating a cognition.

`Add this thought to ME:` is capture intent. It is not approval to keep
the thought as a cognition.
