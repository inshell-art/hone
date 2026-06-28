# Real Codex Smoke Walkthrough

This is a manual, nondeterministic release check. It is not required for
ordinary CI.

## Casual Add

User:

```text
Add this thought to ME:
Agent Art will matter.
```

Expected:

```text
THOUGHT

"Agent Art will matter."

This thought is captured, but it is not in ME yet.

Keep it?
```

No Cognition is created.

## Approval

User:

```text
yes, keep it
```

Expected:

```text
KEPT IN ME

"Agent Art will matter."

This thought is now a cognition.
```

## Read-Only

User:

```text
What do I have in ME about Agent Art?
```

Expected: Codex uses Cognitions and ME Snapshot is unchanged.

## Output Feedback

Codex output:

```text
Agent Art expands authorship through systems.
```

User:

```text
This is my thought. Add it to ME.
```

Expected: a new pending Thought, not a direct Cognition.

## Reference

User:

```text
Import references/ask.positioning.v1.md
```

Expected: Codex explains Reference versus Cognition. No Cognitions are
added.
