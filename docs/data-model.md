# Data model

`memo` keeps shared agent memory in one SQLite file.

## Location

Precedence, highest first:

1. `--db PATH`
2. `MEMO_DB`
3. `$XDG_DATA_HOME/memo/memo.db`
4. `~/.local/share/memo/memo.db` (when `XDG_DATA_HOME` is unset or empty)

```sh
memo --db /tmp/memo.db list
MEMO_DB=/tmp/memo.db memo list
XDG_DATA_HOME=/tmp/xdg memo list
```

The third command creates `/tmp/xdg/memo/memo.db`. Parent directories are created when a command opens the database, not when the path is only resolved.

An empty `MEMO_DB` is ignored, and so is an empty `XDG_DATA_HOME`. If `HOME`, `XDG_DATA_HOME`, and `MEMO_DB` are all unset, opening the default path fails with `memo: could not locate the database directory: set HOME, XDG_DATA_HOME, or MEMO_DB`. `--db` without a value is a clap usage error.

## Connection and migrations

Opening the database sets `journal_mode=WAL`, a 5 second busy timeout, and `foreign_keys=ON`. A locked database is retried for up to 5 seconds while the file is opened. Schema changes run in an immediate transaction and are recorded in `PRAGMA user_version` (currently `2`). Opening a database that is already at this version does not change the schema. A newer `user_version` is an error: `memo: database schema version <found> is newer than supported version <supported>`.

## Scopes and the global scope

`scope` is a text name on each memory. `memo add` uses `global` when `--scope` is omitted. There is no separate global table. `global` is the scope name `"global"`.

| Command | Scope behavior |
| --- | --- |
| `memo add` | `--scope` defaults to `global`. |
| `memo list --scope S` | Exact match. `global` is not added. |
| `memo search --scope S` | Scope `S` and `global`. |
| `memo search --only-scope S` | Scope `S` only. |
| `memo search` | Every scope. |
| `memo scopes` | One row per scope that has unexpired memories, and `global` even when its count is 0. `global` is listed first. |

`--scope` and `--only-scope` cannot be combined. The same normalized content may be stored once per scope. Two scopes means two rows. Any agent may update or delete any row; there is no owner.

`memo scopes` counts only unexpired memories. `last_updated` is the latest `updated_at` among those rows, or null (`-` in text) when the count is 0. A fresh database prints:

```
global  0  -
```

## `memories`

`memories` is a view. It is the stable surface for [`memo sql`](commands.md). Rows are stored in `memory_rows`, which also holds `content_hash`. The view does not expose `content_hash`.

| Column | |
| --- | --- |
| `id` | Integer primary key. |
| `title` | Nullable text. An empty title from `memo update --title ""` is stored as null. |
| `content` | Markdown, stored as written. |
| `kind` | `fact`, `decision`, or `note`. Default `note`. |
| `scope` | Text. Default `global`. |
| `tags` | JSON array of tag names, as text. `[]` when there are none. |
| `source_agent` | Nullable. Set by `memo add --agent`. Informational only. |
| `created_at`, `updated_at` | ISO-8601 UTC with milliseconds (`YYYY-MM-DDTHH:MM:SS.sssZ`). |
| `expires_at` | Nullable, same timestamp format. |

The view includes expired rows. `memo get`, `memo list`, `memo search`, and `memo scopes` hide a row once `expires_at` is in the past. `memo update` and `memo forget` still accept that id. A SQL caller that should skip expired rows filters `expires_at`:

```sql
SELECT id, title
FROM memories
WHERE expires_at IS NULL OR expires_at > '2026-10-27T00:00:00.000Z'
```

Compare `expires_at` to the same ISO-8601 UTC text memo stores. `memo sql --json` returns `tags` as a string, for example `"[\"deploy\",\"process\"]"`, not as a nested array.

`(scope, content_hash)` is unique on `memory_rows`. The hash is SHA-256 hex of the body after trimming and collapsing internal whitespace to a single space. `memo add` of that same normalized body in the same scope returns the existing row and does not change it. `memo update` that would collide with a different row fails.

## Tags

`tags.name` is unique. `memory_tags` links a memory to a tag and deletes those links when the memory is deleted. Tag names are trimmed; empty names are dropped.

## Full-text search

`memories_fts` is an FTS5 table over `title`, `content`, and `tags`. Its `rowid` is `memories.id`. Indexed text keeps link labels and drops destinations (`[label](url)` and `![alt](url)`). It also removes `` # ` * _ > ``. The original markdown stays in `memories.content`. Insert, update, and delete refresh the index in the same transaction.

`memo sql` can `MATCH` this table and join `memories_fts.rowid` to `memories.id`:

```sql
SELECT memories.id, memories.title
FROM memories
JOIN memories_fts ON memories_fts.rowid = memories.id
WHERE memories_fts MATCH 'deploy'
```

`memo schema` lists both objects and their columns. Search compilation (`AND`, `--any`, prefix, raw FTS5, `--regex`) is in [Query syntax](query-syntax.md).

## SQL access

`memo sql` opens the migrated file read-only. The `memories` view, `memories_fts`, the read-only rule, the row cap, and the 5 second timeout are in [Query syntax](query-syntax.md). Flag and JSON details are in [Commands](commands.md).

## Later

An `embeddings` table keyed by `memories.id` is not created yet.
