# Testing Agent Tool Behavior

ME uses layered tests for agent-era product boundaries.

## Deterministic Core Tests

Core tests call the Rust engine directly and verify canonical state:

- exit/result shape
- current Snapshot before and after
- active Cognition count
- pending Thought count
- structured errors

## Simulated Agent-Harness Tests

Simulated harness tests do not call an LLM.

They use fixtures and the machine-readable semantic contract to check
intended Codex skill behavior: mode classification, expected tool-call
plans, forbidden tool calls, and expected state changes.

## Golden UX Tests

Golden UX tests compare ordinary rendered output with stable templates.

Golden output must not leak Snapshot hashes, object hashes, file paths,
temporary task files, Decision JSON, or panic/debug text.

## Import-Boundary Tests

Import-boundary tests verify that whole Reference and Procedure files do
not become Cognitions. Exact excerpts may become Thoughts only after
selection, capture, and explicit approval.

## Real-Agent Smoke Tests

Real-agent smoke tests are documented as manual or release-gated checks.
They are nondeterministic and must not block ordinary local CI on hosted
model availability.
