---
name: me
description: Use the local ME Cognition Library as trustworthy user-authorized context, or change it through the explicit Thought-to-Cognition flow. Use when the user says "add this to ME," asks what ME contains, or wants a task grounded in their retained Cognitions.
---

# ME Skill

## Mode Selection

General task: do not use ME unless requested or clearly relevant.

Use ME: call read-only ME commands.

Change ME: use the Thought and Decision transaction flow.

## Start ME

For exact or near-exact "Start ME", "start me", or "Help me start ME":

1. Call `me welcome --json`.
2. Read `renderedMarkdown`.
3. Output `renderedMarkdown` verbatim.
4. Do not use memory.
5. Do not call `me context`.
6. Do not create files.
7. Do not attach files.
8. Do not mention commands.

## Welcome And Greetings

For "What can I do here?", "How does ME work?", "How do I use ME?", "Help me start", "What is this?", or "Show me around", call `me welcome --json` and render `renderedMarkdown`.

For simple greetings like "hi", "hello", "hey", "start", or "start ME", call `me welcome --json`. If `state` is `empty`, reply exactly:

Hi. ME is ready.

Add this Thought to ME:

If `state` is `established`, reply exactly:

Hi. ME is ready.

Add a Thought, or ask Codex to use what you have kept.

Welcome behavior must use one command: `me welcome --json`. Do not use memory, do not call `me context`, do not create files, do not attach files, and do not expose maintenance commands.

## Use ME

1. Verify workspace with `me current --json`.
2. Prefer stdin for transient tasks: `me context --stdin --json`.
3. Use `me context --task <file> --json` only when the user intentionally provided a file.
4. Use selected Cognitions as user-authorized context.
5. Clearly distinguish ME Cognitions from Codex inference.
6. Do not mutate ME.
7. Do not claim the context is a complete representation of the user.

## Change ME

1. Preserve exact selected Thought text.
2. Prefer stdin for transient text: `me thought capture --stdin --kind <kind> --json`.
3. Show the exact text and intended action.
4. Obtain explicit user Decision if not already explicit.
5. Prepare a Decision JSON with `baseSnapshot`, `action`, `actor`, and exact final body.
6. Prefer stdin for transient Decisions: `me cognition add --thought <thought-id> --decision-stdin --json`.
7. Report Cognitions added, existing Cognitions changed, and the new Snapshot.

## Feedback

When the user says "This sentence is my Thought", "Add this part to ME", or "Keep this from the draft", re-enter the normal Thought flow. Codex Output never enters ME automatically.

## Technical

Only expose CLI and integrity details when asked for technical status, integrity, snapshots, backup, or CLI help.

## Forbidden

Do not edit `.me/**` directly.
Do not use Codex memory as Current ME.
Do not bulk-import References as Cognitions.
Do not treat Procedures as Cognitions.
Do not save Codex Output into ME automatically.
Do not invent relationship objects.
Do not force synthesis.
Do not show snapshots, fsck, bundle, index, or other maintenance details unless the user asks for technical status.
