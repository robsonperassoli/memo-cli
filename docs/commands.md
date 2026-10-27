# Commands

```
memo [OPTIONS] <COMMAND>
```

Commands: `add`, `get`, `update`, `forget`, `list`, `search`, `scopes`, `schema`, `sql`, and `help`.

```sh
memo --help
memo -h
memo help add
memo --version
```

`--help` prints the long help. `-h` prints the short summary. `memo help <command>` prints that command's help. `--version` (`-V`) prints `memo 0.1.0` for this release.

Examples use `--db ./memo.db` and do not write the default database. Each section below recreates that file. On an empty database the first memory is id `1`.

## Global options

`--db` and `--json` are global. They may appear before or after the subcommand.

`--db PATH` selects the SQLite file and overrides `MEMO_DB`. When it is omitted, memo uses `$XDG_DATA_HOME/memo/memo.db`, or `~/.local/share/memo/memo.db` if `XDG_DATA_HOME` is unset. See [Data model](data-model.md).

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
| `created_at` | ISO-8601 UTC with milliseconds |
| `updated_at` | ISO-8601 UTC with milliseconds |
| `expires_at` | ISO-8601 UTC with milliseconds, or `null` |

`add` also includes `deduplicated` (boolean). `search` also includes `rank` (number) and `snippet` (string). `list` prints an array of memory objects. The other commands document their own arrays below.

```sh
rm -f ./memo.db
memo --db ./memo.db list
```

An empty database prints no rows.

## Exit codes

Success is exit `0`. Operational errors go to stderr as `memo: ...` and exit `1`. A clap usage error exits `2`.

| Situation | Exit | Stderr |
| --- | --- | --- |
| Unknown id, including an expired id on `get` | 1 | `memo: memory <id> not found` |
| `update` with no change | 1 | `memo: nothing to update` |
| `search` with no terms and no `--regex` | 1 | `memo: provide search terms or --regex` |
| Invalid FTS5 query | 1 | `memo: invalid search query: ...` |
| Invalid `--regex` | 1 | `memo: invalid regular expression: ...` |
| Invalid `--expires` | 1 | `memo: invalid timestamp: ...` |
| Empty `--tag` or `--scope` | 1 | `memo: tag must not be empty` or `memo: scope must not be empty` |
| `add` in a terminal with no body | 1 | `memo: content is required (argument, stdin, or --file)` |
| `add` body that collides on `update` | 1 | `memo: a memory with the same content already exists in scope <scope>` |
| Write SQL, including `ATTACH` and assigning `PRAGMA` | 1 | `memo: SQL access is read-only` |
| Empty SQL | 1 | `memo: SQL query must not be empty` |
| More than one SQL statement | 1 | `memo: database error: Multiple statements provided` |
| SQL longer than 5 seconds | 1 | `memo: query exceeded the 5s time limit` |
| SQL results past `--max-rows` | 0 | `memo: results truncated to N rows` |
| `--scope` with `--only-scope` | 2 | the arguments cannot be used together |
| `--file` with a positional body, or with `--content` | 2 | the arguments cannot be used together |

```sh
# expect 1
memo --db ./memo.db get 999
```

```sh
# expect 1
memo --db ./memo.db update 1
```

```sh
# expect 2
memo --db ./memo.db search deploy --scope my-project --only-scope my-project
```

Search syntax errors and the SQL failures are in [Query syntax](query-syntax.md).

## memo add

```
memo add [CONTENT] [--file PATH] [--title TEXT] [--kind KIND] [--scope SCOPE] [--tags TAGS] [--agent NAME] [--expires DATE]
```

Store a memory. Markdown in the body is stored unchanged.

### Flags

| Flag | Meaning |
| --- | --- |
| `CONTENT` | Body. `-` reads stdin. Omitted when `--file` is set or when stdin is not a terminal. |
| `--file PATH` | Read the body from a file. Cannot be combined with `CONTENT`. |
| `--title TEXT` | Optional title. |
| `--kind KIND` | `fact`, `decision`, or `note`. Default `note`. |
| `--scope SCOPE` | Scope name. Default `global`. |
| `--tags TAGS` | Comma-separated tags. Names are trimmed; empty names are dropped. |
| `--agent NAME` | Stored as `source_agent`. Informational. |
| `--expires DATE` | `YYYY-MM-DD` (UTC midnight) or an RFC 3339 timestamp. Stored as canonical UTC with milliseconds. |

### Input

The body comes from one of three places:

- the positional argument
- stdin, when the argument is `-`, or when the argument and `--file` are both omitted and stdin is not a terminal
- `--file`

In a terminal, omitting the argument and `--file` exits `1` with `memo: content is required (argument, stdin, or --file)`.

### JSON

One memory object plus `deduplicated`. A new row is `"deduplicated": false`. Text output is `added <id>`.

The same whitespace-normalized content in the same scope returns the existing row, does not change it, prints `deduplicated <id>`, and sets `"deduplicated": true`. Normalization trims the body and collapses internal whitespace. The stored markdown is not rewritten. The same body in another scope is a new row.

### Exit codes

Exit `1` for a missing body in a terminal, an invalid `--expires`, or an unreadable `--file`. Exit `2` when `--file` is combined with `CONTENT`. A duplicate body on `add` is exit `0`.

### Examples

```sh
rm -f ./memo.db notes.md

memo --db ./memo.db add "Production deploys go through the staging gate." \
  --title "Deploy gate" \
  --kind decision \
  --scope my-project \
  --tags deploy,process \
  --agent planner \
  --expires 2030-01-02

memo --db ./memo.db add - --title "Rollback gotcha" --kind note --scope my-project <<'EOF'
Redeploy the previous image.
EOF

printf 'Piped body.\n' | memo --db ./memo.db add --title "From a pipe" --scope my-project

printf 'File body.\n' > notes.md
memo --db ./memo.db add --file notes.md --title "API conventions" --kind fact --scope my-project --tags api

memo --db ./memo.db add "Production deploys go through the staging gate." --scope my-project

memo --db ./memo.db add "offset expiry" --expires 2030-06-01T15:30:00+02:00

memo --db ./memo.db --json sql "SELECT id, expires_at FROM memories WHERE id = 1 OR content = 'offset expiry' ORDER BY id"
```

The first add prints `added 1`. The repeated staging-gate sentence prints `deduplicated 1`. The expiry query prints:

```json
[{"expires_at":"2030-01-02T00:00:00.000Z","id":1},{"expires_at":"2030-06-01T13:30:00.000Z","id":5}]
```

`2030-01-02` is UTC midnight. `2030-06-01T15:30:00+02:00` is stored as `2030-06-01T13:30:00.000Z`. JSON keys are sorted, so `expires_at` comes before `id`.

```sh
memo --db ./memo.db --json add "The cache TTL is 30 seconds." \
  --title "Cache TTL" --kind fact --scope my-project --tags cache
```

The object contains the memory fields and `"deduplicated": false`. `created_at` and `updated_at` are the insert time.

## memo get

```
memo get <ID>
```

### Flags

`ID` is the memory id. Global `--db` and `--json` apply. There are no other flags.

### JSON

One memory object. Text prints `id`, `scope`, `kind`, `title`, `tags`, `source_agent`, `created_at`, `updated_at`, and `expires_at`, then a blank line and the content. A missing title, tag list, `source_agent`, or `expires_at` is `-`. Tags in text are comma-separated.

### Exit codes

An unknown id and an expired id both exit `1` with `memo: memory <id> not found`.

### Examples

```sh
rm -f ./memo.db
memo --db ./memo.db add "Production deploys go through the staging gate." \
  --title "Deploy gate" --kind decision --scope my-project --tags deploy,process
memo --db ./memo.db get 1
memo --db ./memo.db --json get 1
```

## memo update

```
memo update <ID> [--content CONTENT] [--file PATH] [--title TEXT] [--kind KIND] [--scope SCOPE] [--add-tag TAG] [--remove-tag TAG] [--expires DATE]
```

Change the given fields, set `updated_at`, and refresh the full-text index. At least one change is required.

### Flags

| Flag | Meaning |
| --- | --- |
| `ID` | Memory id. Expired ids are accepted. |
| `--content CONTENT` | Replacement body. `-` reads stdin. Cannot be combined with `--file`. |
| `--file PATH` | Read the replacement body from a file. |
| `--title TEXT` | Replacement title. An empty value clears it (`null` in JSON, `-` in text). |
| `--kind KIND` | `fact`, `decision`, or `note`. |
| `--scope SCOPE` | Replacement scope. |
| `--add-tag TAG` | Tag to add. May be repeated. Empty names are ignored. A tag already present is not duplicated. |
| `--remove-tag TAG` | Tag to remove. May be repeated. |
| `--expires DATE` | Replacement expiry, same forms as `add`. Nothing clears `expires_at`. |

### Input

Unlike `add`, a pipe is not the body unless `--content -` or `--file` is set.

### JSON

One memory object for the updated row. Text output is `updated <id>`.

### Exit codes

Exit `1` when no flag changes anything (`memo: nothing to update`), the id does not exist, `--expires` is invalid, or the new body and scope collide with a different row (`memo: a memory with the same content already exists in scope <scope>`). Exit `2` when `--content` and `--file` are both set.

### Examples

```sh
rm -f ./memo.db notes.md
memo --db ./memo.db add "Production deploys go through the staging gate." \
  --title "Deploy gate" --kind decision --scope my-project --tags deploy,process

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
memo --db ./memo.db --json update 1 --add-tag extra
```

Each text update prints `updated 1`. The last command prints the memory object. `title` is `null` because the previous update cleared it.

## memo forget

```
memo forget <ID>
```

### Flags

`ID` is the memory id. Expired ids are accepted. Global `--db` and `--json` apply.

### JSON

The deleted memory object. Text output is `forgot <id>`.

### Exit codes

An unknown id exits `1` with `memo: memory <id> not found`. A later `get` of that id is the same error.

### Examples

```sh
rm -f ./memo.db
memo --db ./memo.db add "keep until the json example"
memo --db ./memo.db add "drop this row"
memo --db ./memo.db forget 2
memo --db ./memo.db --json forget 1
```

`forget 2` prints `forgot 2`. The JSON command prints the object for id `1` and deletes it.

## memo list

```
memo list [--tag TAG] [--scope SCOPE] [--kind KIND] [--limit N]
```

List unexpired memories, newest first.

### Flags

| Flag | Meaning |
| --- | --- |
| `--tag TAG` | Only memories with this tag. |
| `--scope SCOPE` | Exact scope match. Does not add `global`. |
| `--kind KIND` | `fact`, `decision`, or `note`. |
| `--limit N` | Maximum number of rows. |

The filters combine.

### JSON

An array of memory objects, full body included. Text lines are `id`, `kind`, `scope`, and a preview, separated by two spaces. The preview is the title when one is set, otherwise the content. Whitespace collapses to single spaces. The preview is cut at 72 characters with `...`.

### Exit codes

Exit `1` when `--tag` or `--scope` is empty or only whitespace. An empty result is exit `0` and prints nothing.

### Examples

```sh
rm -f ./memo.db
memo --db ./memo.db add "The deploy window is weekdays." --title "Deploy window" --kind fact
memo --db ./memo.db add "Production deploys go through the staging gate." \
  --title "Deploy gate" --kind decision --scope my-project --tags deploy,process

memo --db ./memo.db list
memo --db ./memo.db list --scope my-project --kind decision --tag deploy --limit 20
memo --db ./memo.db list --scope global
memo --db ./memo.db --json list --scope global --limit 1
```

`list` prints id `2` and then id `1`. `--scope my-project` prints only id `2`. `--scope global` prints only id `1`. JSON prints a one-element array.

## memo search

```
memo search [TERM]... [--scope SCOPE | --only-scope SCOPE] [--any] [--regex PATTERN] [--kind KIND] [--tag TAG] [--limit N]
```

Search indexed title, content, and tags. How terms are compiled is in [Query syntax](query-syntax.md).

### Flags

| Flag | Meaning |
| --- | --- |
| `TERM` | Keyword or raw FTS5. Repeatable. A trailing `*` is a prefix. |
| `--scope SCOPE` | This scope plus `global`. |
| `--only-scope SCOPE` | Only this scope. Cannot be combined with `--scope`. |
| `--any` | Join plain terms with `OR` instead of `AND`. |
| `--regex PATTERN` | Rust regular expression over title and content. |
| `--kind KIND` | `fact`, `decision`, or `note`. |
| `--tag TAG` | Only memories with this tag. |
| `--limit N` | Maximum number of rows. |

At least one `TERM` or `--regex` is required. Expired rows are omitted.

### JSON

An array of memory objects plus `rank` and `snippet`. FTS5 `bm25`: a lower `rank` is a stronger match. Regex-only hits use rank `0`. Text lines are `id`, `kind`, `scope`, and a one-line snippet. JSON `snippet` keeps newlines; text collapses them.

### Exit codes

Exit `1` when no terms and no `--regex` are given, the FTS5 query is invalid, or `--regex` is invalid. Exit `2` when `--scope` and `--only-scope` are both set. An empty match list is exit `0`.

### Examples

```sh
rm -f ./memo.db
memo --db ./memo.db add "The deploy window is weekdays." --title "Deploy window" --kind fact
memo --db ./memo.db add "Production deploys go through the staging gate." \
  --title "Deploy gate" --kind decision --scope my-project --tags deploy,process

memo --db ./memo.db search deploy gate
memo --db ./memo.db search window gate --any
memo --db ./memo.db search deploy --scope my-project
memo --db ./memo.db search deploy --only-scope my-project
memo --db ./memo.db search 'deploy*'
memo --db ./memo.db search --regex 'staging' --kind decision --tag deploy --limit 10
memo --db ./memo.db --json search deploy --scope my-project --limit 5
```

`search deploy gate` prints only id `2`. `--any` prints both. `--scope my-project` prints both. `--only-scope my-project` prints id `2`. The JSON command prints an array whose objects include `rank` and `snippet`.

## memo scopes

```
memo scopes
```

### Flags

No flags beyond global `--db` and `--json`.

### JSON

An array of objects with `count` (number), `last_updated` (string or `null`), and `scope` (string). Keys are sorted, so they appear in that order. Text lines are the scope, the count of unexpired memories, and the latest `updated_at`, or `-` when the count is 0. `global` is always listed, first, then other scopes by name.

### Exit codes

Exit `0` even when the only row is `global` with count `0`.

### Examples

```sh
rm -f ./memo.db
memo --db ./memo.db scopes
```

```
global  0  -
```

```sh
memo --db ./memo.db add "Production deploys go through the staging gate." \
  --title "Deploy gate" --kind decision --scope my-project --tags deploy
memo --db ./memo.db scopes
memo --db ./memo.db --json scopes
```

Text lists `global` with count `0`, then `my-project` with count `1` and its `updated_at`. JSON is a two-element array.

## memo schema

```
memo schema
```

### Flags

No flags beyond global `--db` and `--json`.

### JSON

An array of objects with `columns`, `description`, `name`, and `type` (`view` or `table`). Each column has `description`, `name`, and `type`. Those keys are sorted. Column order inside `columns` follows the schema. Text prints the name, the kind, the description, and each column description.

### Exit codes

Exit `0` after the database is created.

### Examples

```sh
rm -f ./memo.db
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

Run one read-only statement. The `memories` view, `memories_fts`, the read-only rule, the row cap, and the timeout are in [Query syntax](query-syntax.md).

### Flags

| Flag | Meaning |
| --- | --- |
| `QUERY` | One SQL statement. Writes are rejected. |
| `--max-rows N` | Maximum number of rows. Default `1000`. |

### JSON

An array of row objects keyed by column name. Keys are sorted alphabetically, so they can differ from the `SELECT` list. `NULL` is `null`. Text output pads columns with two spaces between them. `NULL` is blank, newlines collapse to spaces, and a blob is `<N bytes>`. `memories.tags` stays a JSON array encoded as text, not a nested array.

### Exit codes

Exit `1` for an empty query, a write, more than one statement, or a query cancelled after 5 seconds. Exit `0` when results are truncated; the notice is on stderr.

### Examples

```sh
rm -f ./memo.db
memo --db ./memo.db add "The deploy window is weekdays." --title "Deploy window" --kind fact
memo --db ./memo.db sql "SELECT id, title FROM memories"
memo --db ./memo.db --json sql "SELECT title, id FROM memories"
```

The JSON row is `{"id":1,"title":"Deploy window"}`. `id` is first because the keys are sorted.
