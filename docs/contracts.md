# ME Product Contracts

Prompts guide the model. Transactions govern the product.

ME v0.9 defines seven product contracts.

## Vocabulary Contract

Canonical user-facing terms:

- Thought
- Decision
- Cognition
- Reference
- Procedure
- Output
- Snapshot

Avoid replacing them in ordinary UX with "note", "memory", "belief
entry", "fact", "saved idea", "facet", "article container", or "honed
item".

## Intent Contract

Codex must classify user utterances into one of these modes:

- General Codex
- Using ME read-only
- Changing ME: capture Thought
- Changing ME: approve Cognition
- Changing ME: retire/reactivate Cognition
- Technical ME request

Common count mappings:

| User wording | Counts as |
| --- | --- |
| add this | Thought capture |
| save this | Thought capture |
| remember this | Thought capture |
| note this | Thought capture |
| put this in ME | Thought capture |
| keep it | approval only if a pending Thought was shown |
| yes, keep that thought | approval only if a pending Thought was shown |
| what do I have in ME about X | read-only use |
| draft using ME | read-only use |
| this sentence is my thought | Thought capture from Output |
| import this document | Reference handling unless exact excerpts are selected |

## State Contract

Legal states:

```text
Thought:
  pending
  kept-only
  dismissed
  added

Cognition:
  active
  retired
```

Legal transitions:

```text
capture-thought:
  none -> Thought.pending

keep-thought-only:
  Thought.pending -> Thought.kept-only

dismiss-thought:
  Thought.pending -> Thought.dismissed

add-cognition:
  Thought.pending -> Thought.added
  create Cognition.active

retire-cognition:
  Cognition.active -> Cognition.retired

reactivate-cognition:
  Cognition.retired -> Cognition.active
```

Illegal shortcuts:

```text
none -> Cognition.active
Codex Output -> Cognition.active
Reference -> Cognition.active
Procedure -> Cognition.active
```

## Transaction Contract

State-changing commands must produce a canonical Decision object with a
base Snapshot, action, actor, referenced target, and validated target
state.

For `add-cognition`, the submitted Decision intent must include:

```json
{
  "action": "add-cognition",
  "approved": true
}
```

Missing `approved: true` fails before any Cognition is created.

## Render Contract

Before approval:

```text
THOUGHT

"..."

This thought is captured, but it is not in ME yet.

Keep it?
```

After approval:

```text
KEPT IN ME

"..."

This thought is now a cognition.
```

Read-only use:

```text
USING ME

ME was read, not changed.
```

Output feedback:

```text
If this output contains something worth keeping, say:

  This is my thought. Add it to ME.
```

## Skill Contract

The ME skill must:

- use stable terms
- classify modes
- call read-only commands for use
- call mutation commands only after a Decision
- never edit `.me/**`
- never use Codex memory as Current ME
- never bulk-import References as Cognitions
- never save Output into ME directly

## Test Contract

Every product boundary must have a test:

- casual add captures only
- missing `approved: true` is rejected
- explicit approval succeeds
- read-only context does not mutate Snapshot
- Output cannot become Cognition directly
- Reference is not bulk-imported
- Procedure is not Cognition
