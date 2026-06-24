---
name: me
description: Use the local ME Cognition Library as trustworthy user-authorized context, or change it through the explicit Thought-to-Cognition flow. Use when the user says "add this to ME," asks what ME contains, or wants a task grounded in their retained Cognitions.
---

# ME Skill

## Mode Selection

General task: do not use ME unless requested or clearly relevant.

Use ME: call read-only ME commands.

Change ME: use the Thought and Decision transaction flow.

## Use ME

1. Verify workspace with `me current --json`.
2. Put the user's task in a temporary UTF-8 Markdown file.
3. Call `me context --task <file> --json`.
4. Use selected Cognitions as user-authorized context.
5. Clearly distinguish ME Cognitions from Codex inference.
6. Do not mutate ME.
7. Do not claim the context is a complete representation of the user.

## Change ME

1. Preserve exact selected Thought text.
2. Capture it with `me thought capture --file <file> --kind <kind> --json`.
3. Show the exact text and intended action.
4. Obtain explicit user Decision if not already explicit.
5. Write a Decision file with `baseSnapshot`, `action`, `actor`, and exact final body.
6. Call `me cognition add --thought <thought-id> --decision <file> --json`.
7. Report Cognitions added, existing Cognitions changed, and the new Snapshot.

## Forbidden

Do not edit `.me/**` directly.
Do not use Codex memory as Current ME.
Do not bulk-import References as Cognitions.
Do not treat Procedures as Cognitions.
Do not save Codex Output into ME automatically.
Do not invent relationship objects.
Do not force synthesis.
