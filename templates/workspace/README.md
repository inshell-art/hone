# ME

ME is a local meaning environment designed to be used through Codex App.

When a thought occurs, tell Codex:

> Add this Thought to ME: ...

ME captures the exact words. Once you approve keeping them, the Thought
becomes a Cognition in your local library.

Later, Codex can inspect, compare, and compose from your Cognitions
without changing them.

Start in Codex by running:

```bash
me start
```

Canonical state lives in `.me/objects/**` and `.me/refs/current`.
Generated views under `views/**` are readable projections and may be
overwritten.
