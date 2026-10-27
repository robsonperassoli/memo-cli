# Commands

Global flags:

- `--db PATH` selects the database file and overrides `MEMO_DB`. The default path is in [Data model](data-model.md). The file is created on first use.
- `--json` prints JSON instead of text. A memory object has `id`, `scope`, `kind`, `title`, `content`, `tags`, `source_agent`, `created_at`, `updated_at`, and `expires_at`. `memo list --json` prints an array of those objects. `memo add --json` also includes `deduplicated`.

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
