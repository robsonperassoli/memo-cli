# Data model

`memo` keeps shared agent memory in one SQLite file.

## Location

The default path is `$XDG_DATA_HOME/memo/memo.db`. When `XDG_DATA_HOME` is unset or empty, the path is `~/.local/share/memo/memo.db`. Parent directories are created the first time the file is opened.

Overrides, highest precedence first:

1. `--db PATH`
2. `MEMO_DB`

An empty `MEMO_DB` is ignored. The database is not created just by resolving the path.

## Connection and migrations

Opening the database sets `journal_mode=WAL`, a 5 second busy timeout, and `foreign_keys=ON`. Schema changes run in an immediate transaction and are recorded in `PRAGMA user_version` (currently `2`). Opening a database that is already at this version does not change the schema. A newer `user_version` is an error.

## `memories`

`memories` is a view. It is the stable surface for [`memo sql`](commands.md). Rows are stored in `memory_rows`, which also holds `content_hash`. The view does not expose `content_hash`.

| Column | |
| --- | --- |
| `id` | Integer primary key. |
| `title` | Nullable text. |
| `content` | Markdown, stored as written. |
| `kind` | `fact`, `decision`, or `note`. Default `note`. |
| `scope` | Text. Default `global`. |
| `tags` | JSON array of tag names, as text. `[]` when there are none. |
| `source_agent` | Nullable. Informational only. |
| `created_at`, `updated_at` | ISO-8601 UTC (`YYYY-MM-DDTHH:MM:SS.sssZ`). |
| `expires_at` | Nullable, same timestamp format. |

The view includes expired rows. Filter on `expires_at` when a query should skip them. `memo` commands still hide expired rows unless an update or delete names the id.

`(scope, content_hash)` is unique on `memory_rows`. Normalization trims the content and collapses internal whitespace to a single space. Inserting that same normalized content in the same scope returns the existing row and does not change it. The same content in another scope is a different row.

There is no owner. Any agent may update or delete any row.

## Tags

`tags.name` is unique. `memory_tags` links a memory to a tag and deletes those links when the memory is deleted. Tag names are trimmed; empty names are dropped.

## Full-text search

`memories_fts` is an FTS5 table over `title`, `content`, and `tags`. Its `rowid` is `memories.id`. Indexed text keeps link labels and drops destinations (`[label](url)` and `![alt](url)`). It also removes `` # ` * _ > ``. The original markdown stays in `memories.content`. Insert, update, and delete refresh the index in the same transaction.

`memo sql` can `MATCH` this table and join `memories_fts.rowid` to `memories.id`. `memo schema` lists both objects and their columns.

## Later

An `embeddings` table keyed by `memories.id` is not created yet.
