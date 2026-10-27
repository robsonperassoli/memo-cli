# Commands

Global flags:

- `--db PATH` selects the database file and overrides `MEMO_DB`. The default path is in [Data model](data-model.md). The file is created on first use.
- `--json` prints JSON instead of text. A memory object has `id`, `scope`, `kind`, `title`, `content`, `tags`, `source_agent`, `created_at`, `updated_at`, and `expires_at`. `memo list --json` prints an array of those objects. `memo add --json` also includes `deduplicated`. `memo search --json` adds `rank` and `snippet`. `memo scopes --json` prints `scope`, `count`, and `last_updated`. `memo schema --json` prints `name`, `type`, `description`, and `columns`. `memo sql --json` prints an array of row objects.

Errors are written to stderr and the process exits non-zero. An unknown id is an error. Invalid arguments exit with clap's usage error.

## memo add

`memo add [CONTENT|-] [--file PATH] [--title TEXT] [--kind fact|decision|note] [--scope SCOPE] [--tags a,b] [--agent NAME] [--expires DATE]`

The body comes from the positional argument, from stdin when the argument is `-` or when stdin is a pipe and no argument is given, or from `--file`. Markdown is stored unchanged. Only one of those sources may be set. Omitted `--scope` means `global`. Kind defaults to `note`. `--tags` is a comma-separated list. `--expires` is `YYYY-MM-DD` (UTC midnight) or an RFC 3339 timestamp.

The same whitespace-normalized content in the same scope returns the existing row. Text output is `deduplicated <id>`; JSON sets `"deduplicated": true`. A new row prints `added <id>` and `"deduplicated": false`.

## memo get

`memo get <id>`

Print one memory. Expired and unknown ids are not found.

## memo update

`memo update <id> [--content CONTENT|-] [--file PATH] [--title TEXT] [--kind fact|decision|note] [--scope SCOPE] [--add-tag TAG] [--remove-tag TAG] [--expires DATE]`

Changes the given fields, sets `updated_at`, and refreshes the full-text index. `--content -` reads stdin. `--add-tag` and `--remove-tag` may be repeated. At least one change is required. An unknown id is not found.

## memo forget

`memo forget <id>`

Delete a memory. Text output is `forgot <id>`. JSON prints the deleted memory. An unknown id is not found.

## memo list

`memo list [--tag TAG] [--scope SCOPE] [--kind fact|decision|note] [--limit N]`

Newest memories first. Each text line is the id, kind, scope, and either the title or a shortened single line of the content. JSON rows contain the full body. Expired rows are omitted.

## memo search

`memo search <terms...> [--scope SCOPE | --only-scope SCOPE] [--any] [--regex PATTERN] [--kind fact|decision|note] [--tag TAG] [--limit N]`

Plain terms are combined with AND, or with OR when `--any` is set. A trailing `*` is a prefix. Quotes, `OR`, `NEAR`, and column filters are passed to FTS5 as written. Details are in [Query syntax](query-syntax.md).

`--scope` includes that scope and `global`. `--only-scope` includes that scope alone. Without either flag, every scope is searched. The two flags cannot be combined.

`--regex` is a Rust regular expression over the title and content. It can be used with or without terms. Expired rows are omitted. An invalid query or regular expression is an error.

Each text line is the id, kind, scope, and a snippet. JSON rows are memory objects plus `rank` (FTS5 bm25; lower is a stronger match) and `snippet`.

## memo scopes

`memo scopes`

Prints each scope, how many unexpired memories it contains, and the latest `updated_at`. `global` is always listed. JSON is an array of `scope`, `count`, and `last_updated` (`null` when the scope has no unexpired memories).

## memo schema

`memo schema`

Lists the queryable objects: the `memories` view and the `memories_fts` table. Each column has a short description. See [Data model](data-model.md).

## memo sql

`memo sql "<query>" [--max-rows N]`

Runs one read-only statement. The connection is opened with `SQLITE_OPEN_READ_ONLY` and `PRAGMA query_only`. `INSERT`, `UPDATE`, `DELETE`, `DROP`, and `ATTACH` fail with `SQL access is read-only`.

Text output is a table. JSON output is an array of objects keyed by column name. The default cap is 1000 rows. A cap that leaves rows unread prints `results truncated to N rows` on stderr and still exits 0. A query that runs longer than 5 seconds is cancelled.

`memories.tags` is a JSON array in a text column. The view includes expired rows. `memories_fts` supports `MATCH`; join its `rowid` to `memories.id`.
