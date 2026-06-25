# Privacy

ME stores canonical Cognition data locally.

Canonical state lives in:

```text
.me/objects/**
.me/refs/current
```

The `me` engine does not use the network. It does not perform telemetry,
update checks, cloud sync, wallet access, or model API calls.

`me start` opening Codex App is a local operating-system action, not an
ME network request.

Codex's model may run remotely and receives the Cognitions selected for
the task you ask it to perform. Do not treat the full product as
entirely offline merely because the ME engine is local.

ME minimizes default context by retrieving bounded Cognitions for the
current task.
