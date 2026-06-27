# ME CLI

The CLI is the local engine, administrative interface, and automation
contract. Normal everyday use is through Codex App.

## Create

```bash
me start
me start --workspace ~/Research-ME
me start --no-open --json
me new ~/ME
me new ~/ME-demo --demo
```

## Product Welcome

```bash
me --workspace ~/ME welcome
me --workspace ~/ME welcome --json
me --workspace ~/ME home
me --workspace ~/ME home --json
me --workspace ~/ME guide
```

## Read Context

```bash
printf 'Draft a reply using ME.\n' | me --workspace ~/ME context --stdin --limit 20 --json
me --workspace ~/ME search "authorship" --limit 20 --json
```

Read commands do not create canonical objects, append journal entries,
or advance `.me/refs/current`.

## Change ME

```bash
printf 'A Thought.\n' | me --workspace ~/ME thought capture --stdin --kind idea --json
printf '{"action":"add-cognition","approved":true}' | me --workspace ~/ME cognition add --thought <thought-id> --decision-stdin --json
```

Thought capture is casual. Cognition creation is not: `me cognition add`
requires `approved: true` in the Decision and should run only after the
user explicitly approves keeping the captured thought.

Use `--file`, `--task`, or `--decision` when a real file is part of the workflow.

## Technical State

```bash
me --workspace ~/ME status --json
me --workspace ~/ME current --json
me --workspace ~/ME fsck --json
me --workspace ~/ME index rebuild --json
```

Technical commands may show hashes and workspace internals. Product
welcome commands should not.

## Backup

```bash
me --workspace ~/ME bundle create /tmp/me.bundle.tar
me bundle verify /tmp/me.bundle.tar
me bundle restore /tmp/me.bundle.tar /tmp/restored-me
```
