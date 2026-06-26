---
name: me
description: Use the local ME Cognition Library as trustworthy user-authorized context, or change it through the explicit thought-to-cognition flow. Use when the user says "add this to ME," asks what ME contains, or wants a task grounded in their retained cognitions.
---

# ME Skill

## Mode Selection

General task: do not use ME unless requested or clearly relevant.

Use ME: call read-only ME commands.

Change ME: use the thought, explicit decision, and cognition transaction flow.

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
9. Do not explain cognition yet when `state` is `empty`.

## Welcome And Greetings

For "What can I do here?", "How does ME work?", "How do I use ME?", "Help me start", "What is this?", or "Show me around", call `me welcome --json` and render `renderedMarkdown`.

For simple greetings like "hi", "hello", "hey", "start", or "start ME", call `me welcome --json`. If `state` is `empty`, reply exactly:

Hi. ME is ready.

Add this thought to ME:

If `state` is `established`, reply exactly:

Hi. ME is ready.

Add another thought, or ask Codex to use what you have kept.

Welcome behavior must use one command: `me welcome --json`. Do not use memory, do not call `me context`, do not create files, do not attach files, and do not expose maintenance commands.

## Use ME

1. Verify workspace with `me current --json`.
2. Prefer stdin for transient tasks: `me context --stdin --json`.
3. Use `me context --task <file> --json` only when the user intentionally provided a file.
4. Use selected cognitions as user-authorized context.
5. Clearly distinguish ME cognitions from Codex inference.
6. Do not mutate ME.
7. Do not claim the context is a complete representation of the user.
8. If `guidance.kind` is `first-read`, append `guidance.renderedMarkdown` once after the answer.

## Change ME

1. Preserve exact selected thought text.
2. Prefer stdin for transient text: `me thought capture --stdin --kind <kind> --json`.
3. Show the exact text, say it is not in ME yet, and ask whether to keep it.
4. Do not mention Decision files, canonical mutation, transaction internals, snapshots, or hashes.
5. Obtain explicit user decision if not already explicit.
6. Prepare a Decision JSON with `baseSnapshot`, `action`, `actor`, and exact final body.
7. Prefer stdin for transient Decisions: `me cognition add --thought <thought-id> --decision-stdin --json`.
8. After the first successful add, teach: "In ME, a thought you choose to keep is called a cognition."
9. State that Codex can use it without changing ME.
10. Suggest up to two use prompts and one add prompt from `nextGuidance`.
11. For later additions, use the brief `renderedMarkdown` success copy.
12. Hide technical fields unless the user asks for technical status.

## Feedback

When the user says "This sentence is my thought", "Add this part to ME", or "Keep this from the draft", re-enter the normal thought flow. Codex output never enters ME automatically.

## Technical

Only expose CLI and integrity details when asked for technical status, integrity, snapshots, backup, or CLI help.

## Forbidden

Do not edit `.me/**` directly.
Do not use Codex memory as Current ME.
Do not use Codex memory to determine workspace counts, whether this is the first cognition, whether guidance was shown, or canonical state.
Do not bulk-import References as cognitions.
Do not treat Procedures as cognitions.
Do not save Codex output into ME automatically.
Do not invent relationship objects.
Do not force synthesis.
Do not show snapshots, fsck, bundle, index, or other maintenance details unless the user asks for technical status.
