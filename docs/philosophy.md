# ME Philosophy

AI product design is count design.

Natural language is vague. A user can say "add this", "remember this",
"this matters", "keep it", or "yes". The model can infer many possible
meanings from those words.

ME must define what counts.

## Count Design

Count design is deciding what a natural-language action is allowed to
mean in the product.

For ME:

- "Add this thought to ME" counts as Thought capture.
- "Yes, keep it" may count as Cognition approval only if a specific
  pending Thought was just shown back.
- "Draft a reply using ME" counts as read-only Cognition use.
- "This sentence is my thought" counts as Output feedback entering
  Thought capture.
- "Use this source document" counts as Reference use, not Cognition
  import.

## Semantic State Machine

The shape of an AI-native tool is not mainly a screen flow. It is a
semantic state machine:

```text
Utterance
  -> interpreted intent
  -> counted product meaning
  -> legal transition
  -> deterministic transaction
  -> canonical state
  -> rendered proof
```

For ME:

- Utterance: the user says something.
- Thought: exact expression captured.
- Decision: explicit keep, dismiss, retire, or reactivate instruction.
- Cognition: Thought the user chose to keep.
- Snapshot: historical state after a legal transition.

## Product Law

Prompts guide the model. Transactions govern the product.

A prompt or skill can instruct Codex to ask for approval before creating
a Cognition. ME Core must still enforce that a Decision without
`approved: true` cannot create a Cognition.

Conversation can stay fluid:

```text
ask
draft
compare
explore
summarize
argue
speculate
```

Canonical state stays rigid:

```text
Thought captured
Decision explicit
Cognition added
Snapshot advanced
```

The product's trust comes from that split.
