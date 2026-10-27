# Commands

```
memo [OPTIONS] <COMMAND>
```

Commands: `add`, `get`, `update`, `forget`, `list`, `search`, `scopes`, `schema`, `sql`, and `help`.

```sh
memo --help
memo add --help
memo -h
memo --version
```

`--help` prints the long help. `-h` prints the short summary. `--version` (`-V`) prints `memo 0.1.0` for this release. `memo help <command>` prints that command's help.

Examples use `--db ./memo.db`. Delete that file before a section that names specific ids. On an empty database the first memory is id `1`.

## Global options

Both options are global, so they may appear before or after the subcommand.

`--db PATH` selects the SQLite file and overrides `MEMO_DB`. When it is omitted, memo uses `$XDG_DATA_HOME/memo/memo.db`, or `~/.local/share/memo/memo.db` if `XDG_DATA_HOME` is unset. See [Data model](data-model.md).

```sh
memo --db ./memo.db list
```

`--json` prints one JSON value instead of text. Object keys are sorted alphabetically. A memory object has these fields:

| Field | JSON |
| --- | --- |
| `id` | number |
| `scope` | string |
| `kind` | `fact`, `decision`, or `note` |
| `title` | string or `null` |
| `content` | string |
| `tags` | array of strings |
| `source_agent` | string or `null` |
| `created_at` | ISO-8601 UTC |
| `updated_at` | ISO-8601 UTC |
| `expires_at` | ISO-8601 UTC or `null` |

`memo add --json` also includes `deduplicated`. `memo search --json` adds `rank` and `snippet`. `memo list --json` and `memo scopes --json` print arrays. `memo schema --json` and `memo sql --json` are described with those commands.

```sh
memo --db ./memo.db --json add "The cache TTL is 30 seconds." \
  --title "Cache TTL" --kind fact --scope my-project --tags cache
```

```json
{"content":"The cache TTL is 30 seconds.","created_at":"2026-10-27T02:23:57.662Z","deduplicated":false,"expires_at":null,"id":1,"kind":"fact","scope":"my-project","source_agent":null,"tags":["cache"],"title":"Cache TTL","updated_at":"2026-10-27T02:23:57.662Z"}
```

The timestamps above are one run's values.

## Exit status

Operational errors go to stderr as `memo: ...` and exit `1`. A usage error from clap exits `2` and prints the usage to stderr. Success is `0`, including a query that reports truncation.

| Situation | Exit | Message |
| --- | --- | --- |
| Unknown id | 1 | `memo: memory <id> not found` |
| `update` with no change | 1 | `memo: nothing to update` |
| `search` with no terms and no `--regex` | 1 | `memo: provide search terms or --regex` |
| Invalid FTS5 query | 1 | `memo: invalid search query: ...` |
| Invalid `--regex` | 1 | `memo: invalid regular expression: ...` |
| Invalid `--expires` | 1 | `memo: invalid timestamp: ...` |
| Empty `--tag` or `--scope` | 1 | `memo: tag must not be empty` or `memo: scope must not be empty` |
| `add` in a terminal with no body | 1 | `memo: content is required (argument, stdin, or --file)` |
| Write SQL | 1 | `memo: SQL access is read-only` |
| SQL longer than 5 seconds | 1 | `memo: query exceeded the 5s time limit` |
| `--scope` with `--only-scope` | 2 | clap: the arguments cannot be used together |
| `--file` with a positional body, or with `--content` | 2 | clap: the arguments cannot be used together |

## memo add

```
memo add [CONTENT] [--file PATH] [--title TEXT] [--kind KIND] [--scope SCOPE] [--tags TAGS] [--agent NAME] [--expires DATE]
```

Store a memory. Markdown in the body is stored unchanged.

The body is the positional `CONTENT`, stdin when `CONTENT` is `-`, stdin when `CONTENT` and `--file` are both omitted and stdin is not a terminal, or the file given by `--file`. `--file` cannot be combined with `CONTENT`. In a terminal, omitting both the argument and `--file` fails with `content is required`.

`--kind` is `fact`, `decision`, or `note`. The default is `note`. `--scope` defaults to `global`. `--tags` is a comma-separated list; names are trimmed and empty names are dropped. `--agent` is stored as `source_agent` and is informational. `--title` is optional.

`--expires` is `YYYY-MM-DD` or an RFC 3339 timestamp. A date is UTC midnight. The stored value is canonical UTC with millisecond precision, so `2030-01-02` is stored as `2030-01-02T00:00:00.000Z` and `2030-06-01T15:30:00+02:00` as `2030-06-01T13:30:00.000Z`.

Text output is `added <id>` for a new row. The same whitespace-normalized content in the same scope returns the existing row, leaves it unchanged, and prints `deduplicated <id>`. Normalization trims the body and collapses internal whitespace; the stored markdown is not rewritten. The same body in another scope is a new row. JSON sets `"deduplicated": true` or `false`.

```sh
memo --db ./memo.db add "Production deploys go through the staging gate." \
  --title "Deploy gate" \
  --kind decision \
  --scope my-project \
  --tags deploy,process \
  --agent planner \
  --expires 2030-01-02
```

```
added 1
```

Read stdin explicitly, or from a pipe when no body argument is given:

```sh
memo --db ./memo.db add - --title "Rollback gotcha" --kind note --scope my-project <<'EOF'
Redeploy the previous image.
EOF

printf 'Piped body.\n' | memo --db ./memo.db add --title "From a pipe" --scope my-project
```

Read a file:

```sh
memo --db ./memo.db add --file notes.md --title "API conventions" --kind fact --scope my-project --tags api
```

Inserting the staging-gate sentence again in `my-project` prints `deduplicated 1`.

## memo get

```
memo get <ID>
```

Print one memory that has not expired. Text prints the fields, a blank line, and the content. A missing title, tag list, `source_agent`, or `expires_at` is `-`. JSON prints one memory object.

An unknown id and an expired id both fail with `memo: memory <id> not found`.

```sh
memo --db ./memo.db get 1
memo --db ./memo.db --json get 1
```

## memo update

```
memo update <ID> [--content CONTENT] [--file PATH] [--title TEXT] [--kind KIND] [--scope SCOPE] [--add-tag TAG] [--remove-tag TAG] [--expires DATE]
```

Change the given fields, set `updated_at`, and refresh the full-text index. At least one change is required. Unlike `add`, a pipe is not read unless `--content -` or `--file` is set.

`--content` replaces the body. `-` reads stdin. `--file` reads the replacement body from a file and cannot be combined with `--content`. `--title` replaces the title; an empty value clears it. `--kind` and `--scope` replace those fields. `--add-tag` and `--remove-tag` may be repeated. Tag names are trimmed; an empty name is ignored; adding a tag that is already present does not duplicate it. `--expires` sets a new expiry using the same date and RFC 3339 forms as `add`. There is no flag that clears `expires_at`.

Text output is `updated <id>`. JSON prints the updated memory object. An unknown id is not found. Update and forget still accept an expired id. Changing the body or the scope to match another row's normalized content in the destination scope fails with `memo: a memory with the same content already exists in scope <scope>`.

```sh
memo --db ./memo.db update 1 \
  --content "The cache TTL is 60 seconds." \
  --title "Cache TTL" \
  --kind fact \
  --scope my-project \
  --add-tag ttl \
  --remove-tag process \
  --expires 2031-01-01

printf 'Replaced from stdin.\n' | memo --db ./memo.db update 1 --content -

printf 'Replaced from a file.\n' > notes.md
memo --db ./memo.db update 1 --file notes.md

memo --db ./memo.db update 1 --title ""
```

## memo forget

```
memo forget <ID>
```

Delete a memory, including an expired one. Text output is `forgot <id>`. JSON prints the deleted memory object. An unknown id is not found. A later `get` of that id is not found.

Text prints `forgot 1`:

```sh
memo --db ./memo.db forget 1
```

JSON prints the deleted memory object. This is the same deletion, not a second one:

```sh
memo --db ./memo.db --json forget 1
```

## memo list

```
memo list [--tag TAG] [--scope SCOPE] [--kind KIND] [--limit N]
```

List unexpired memories, newest first. `--scope` is an exact scope match and does not add `global`. `--tag` and `--kind` (`fact`, `decision`, or `note`) filter further. `--limit` caps the number of rows. The flags combine.

Each text line is the id, kind, scope, and a preview, separated by two spaces. The preview is the title when the title is set, otherwise the content. Whitespace collapses to single spaces, and the preview is cut at 72 characters with `...`. JSON is an array of memory objects with the full body. Expired rows are omitted.

```sh
memo --db ./memo.db list
memo --db ./memo.db list --scope my-project --kind decision --tag deploy --limit 20
memo --db ./memo.db --json list --scope global
```

`list --scope global` does not include `my-project`. An empty database still opens the file and prints nothing. `memo scopes` on that file prints `global  0  -`.

## memo search

```
memo search [TERM]... [--scope SCOPE | --only-scope SCOPE] [--any] [--regex PATTERN] [--kind KIND] [--tag TAG] [--limit N]
```

Search indexed title, content, and tags. Expired rows are omitted. How terms, prefixes, `--any`, raw FTS5, and `--regex` are compiled is in [Query syntax](query-syntax.md).

`--scope SCOPE` searches that scope and `global`. `--only-scope SCOPE` searches that scope alone. Omitting both searches every scope. The two flags cannot be combined (exit 2). `--kind` and `--tag` restrict matches. `--limit` caps the rows. With terms and no `--regex`, the limit is applied to the ranked results. With `--regex`, filters run before the limit.

At least one `TERM` or `--regex` is required.

Each text line is the id, kind, scope, and a one-line snippet. JSON rows are memory objects plus `rank` and `snippet`. FTS5 `bm25` rank is lower for a stronger match. Regex-only hits use rank `0`, are ordered newest first, and take the snippet from the title and content around the match. Text output collapses whitespace in the snippet; the JSON `snippet` keeps newlines.

```sh
memo --db ./memo.db search deploy rollback
memo --db ./memo.db search deploy rollback --any
memo --db ./memo.db search deploy --scope my-project
memo --db ./memo.db search deploy --only-scope my-project
memo --db ./memo.db search 'deploy*'
memo --db ./memo.db search --regex 'TTL is [0-9]+' --kind fact --tag cache --limit 10
memo --db ./memo.db --json search deploy --scope my-project --limit 5
```

On the [quickstart](getting-started.md) database, `search deploy --scope my-project` prints both rows, and `search deploy --only-scope my-project` prints only id `2`.

## memo scopes

```
memo scopes
```

List every scope that has unexpired memories, plus `global` even when it has none. Each row has the scope name, the count of unexpired memories, and the latest `updated_at`. `global` is first, then the other scopes by name. A scope with no unexpired rows shows `-` for the time.

JSON is an array of objects with `scope`, `count`, and `last_updated` (`null` when there are no unexpired memories). Keys are sorted, so the fields appear as `count`, `last_updated`, `scope`.

```sh
memo --db ./memo.db scopes
memo --db ./memo.db --json scopes
```

An empty database prints:

```
global  0  -
```

How `global` interacts with `add`, `list`, and `search` is in [Data model](data-model.md).

## memo schema

```
memo schema
```

Show the objects `memo sql` should query: the `memories` view and the `memories_fts` table. Text prints the name, the kind (`view` or `table`), a one-line description, and each column's description. JSON is an array of objects with `name`, `type`, `description`, and `columns`. Each column has `name`, `type`, and `description`. Those object keys are sorted alphabetically. Column order inside `columns` follows the schema.

```sh
memo --db ./memo.db schema
memo --db ./memo.db --json schema
```

```
memories (view)
Stable memory rows. tags is a JSON array of tag names.
  id            Integer primary key.
  title         Optional title.
  content       Markdown body, stored as written.
  kind          fact, decision, or note.
  scope         Scope name. The default scope is global.
  tags          JSON array of tag names, stored as text.
  source_agent  Recorded agent name, or null.
  created_at    Creation time, ISO-8601 UTC.
  updated_at    Last update time, ISO-8601 UTC.
  expires_at    Expiry time, ISO-8601 UTC, or null.

memories_fts (table)
Full-text index. MATCH this table and join rowid to memories.id.
  title    Indexed title.
  content  Indexed content. Markdown symbols are removed.
  tags     Indexed tag names.
```

## memo sql

```
memo sql <QUERY> [--max-rows N]
```

Run one read-only statement against the database. The file is migrated first if needed, then opened with `SQLITE_OPEN_READ_ONLY` and `PRAGMA query_only`. A statement that writes, including `INSERT`, `UPDATE`, `DELETE`, `DROP`, `ATTACH`, and a `PRAGMA` that assigns a value, fails with `memo: SQL access is read-only`. A read-only `PRAGMA` is allowed (`PRAGMA user_version` prints `2` on a current database).

More than one statement fails with `memo: database error: Multiple statements provided`. An empty query fails with `memo: SQL query must not be empty`.

`--max-rows` defaults to `1000`. When another row exists past the cap, memo keeps the first N rows, prints `memo: results truncated to N rows` on stderr, and exits `0`. A query that runs longer than 5 seconds is cancelled with `memo: query exceeded the 5s time limit`.

Text output is a table. Columns are padded and separated by two spaces. `NULL` is blank, newlines in text collapse to spaces, and a blob is `<N bytes>`. JSON is an array of objects, one per row, keyed by column name. Keys are sorted alphabetically, so they can differ from the `SELECT` order. `memories.tags` stays a JSON array encoded as text, not a nested JSON array.

The `memories` view includes expired rows. `memo list` and `memo search` hide those rows; a SQL query does not, unless the statement filters `expires_at`. `content_hash` is not a column of the view. It lives on the internal `memory_rows` table.

```sh
memo --db ./memo.db schema

memo --db ./memo.db sql \
  "SELECT id, title, scope, tags FROM memories WHERE scope = 'my-project' ORDER BY id"

memo --db ./memo.db --json sql \
  "SELECT id, tags FROM memories WHERE id = 1"

memo --db ./memo.db sql \
  "SELECT memories.id, memories.title
   FROM memories
   JOIN memories_fts ON memories_fts.rowid = memories.id
   WHERE memories_fts MATCH 'deploy'
   ORDER BY memories.id"

memo --db ./memo.db sql \
  "SELECT id, title FROM memories
   WHERE expires_at IS NULL OR expires_at > '2026-10-27T00:00:00.000Z'
   ORDER BY id"

memo --db ./memo.db sql "SELECT n FROM (
  WITH RECURSIVE c(n) AS (
    SELECT 1 UNION ALL SELECT n + 1 FROM c WHERE n < 30
  ) SELECT n FROM c
)" --max-rows 2
```

The last command prints two rows and, on stderr, `memo: results truncated to 2 rows`.

`SELECT title, id` is returned in JSON as `{"id":1,"title":"..."}` because the keys are sorted. The column descriptions and the rest of the storage rules are in [Data model](data-model.md).
