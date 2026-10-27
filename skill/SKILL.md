---
name: memo
description: Search shared agent memory before a task, and record durable facts, decisions, conventions, gotchas, and infrastructure facts with the memo CLI.
---

# memo

Search shared memory before acting. Save only durable knowledge. Any agent may update or delete any memory.

## Search first

Search the project scope before a task. `--scope` includes that scope and `global`. Plain terms are combined with AND. Use `--json` when parsing the output. Expired memories are omitted.

```
memo search deploy rollback --scope my-project
memo search --json deploy* --scope my-project --kind decision --limit 10
```

## What to save

Save durable facts, decisions with their rationale, conventions, gotchas, and infrastructure facts. Do not save transient task state, secrets, or credentials.

Set `--kind` (`fact`, `decision`, or `note`), `--tags`, `--scope`, and `--title`. Omitted `--scope` is `global`. Kind defaults to `note`.

```
memo add "Production deploys go through the staging gate." --title "Deploy gate" --kind decision --scope my-project --tags deploy,process
memo add --json "The cache TTL is 30 seconds." --title "Cache TTL" --kind fact --scope my-project --tags cache
```

Use `--file` or stdin for multi-line markdown. `-` reads stdin. A pipe is read when no body argument is given. Markdown is stored unchanged.

```
memo add --file notes.md --title "API conventions" --kind fact --scope my-project --tags api
memo add - --title "Rollback gotcha" --kind note --scope my-project --tags deploy
```

## Keep memories current

Update a memory when the fact changes. Forget a memory that is no longer true. There is no owner; any agent may edit any memory.

```
memo update <id> --content "The cache TTL is 60 seconds." --add-tag cache
memo get --json <id>
memo list --json --scope my-project --tag deploy --limit 20
memo forget <id>
```

`--content -` reads stdin. `--file` reads a replacement body.

## SQL

Run `memo schema` before `memo sql`. Query the `memories` view. `memories_fts` supports `MATCH`; join `memories_fts.rowid` to `memories.id`. `memo sql` is read-only.

```
memo schema
memo sql --json "SELECT id, title, tags FROM memories WHERE scope = 'my-project'"
```
