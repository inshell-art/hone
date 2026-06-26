# ME Workspace

ME is a local application operated through Codex App.

## Everyday Use

- When the user has a thought, preserve the exact words.
- Add it to ME only after the user approves keeping it.
- For exact `Start ME`, call `me welcome --json` and output `renderedMarkdown` verbatim.
- For a simple empty-workspace greeting, call `me welcome --json` and reply with `Hi. ME is ready.` plus `Add this thought to ME:`.
- Use `me welcome --json` for "What can I do here?" and present the canonical welcome.
- Use `me context` when the user asks Codex to inspect, compare, or compose from ME.
- Reading and composition do not change ME.
- Codex output never enters ME automatically.
- For welcome and greetings, do not use memory, create files, call `me context`, attach files, or expose maintenance commands.

## Technical Rules

- This directory is a ME workspace, not a software repository.
- Use `me ... --json` for deterministic operations.
- Never edit `.me/**` directly.
- Never edit `views/**` directly.
- Do not use host memory as Current ME.
- Do not bulk-import References as cognitions.
- Do not treat Procedures as cognitions.
- App, Run, Association, Proposal, and Synthesis commands are legacy compatibility only.
- Publishing, external sharing, and network access are outside ME.

<!-- ME:USER-BEGIN -->
<!-- ME:USER-END -->
