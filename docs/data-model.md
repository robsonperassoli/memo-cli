# Data model

`memo` keeps shared agent memory in one SQLite file.

## Location

The default path is `$XDG_DATA_HOME/memo/memo.db`. When `XDG_DATA_HOME` is unset or empty, the path is `~/.local/share/memo/memo.db`. Parent directories are created the first time the file is opened.

Overrides, highest precedence first:

1. `--db PATH`
2. `MEMO_DB`

An empty `MEMO_DB` is ignored. The database is not created just by resolving the path.

## Connection and migrations

Opening the database sets `journal_mode=WAL`, a 5 second busy timeout, and `foreign_keys=ON`. Schema changes run in an immediate transaction and are recorded in `PRAGMA user_version` (currently `1`). Opening a database that is already at this version does not change the schema. A newer `user_version` is an error.

## `memories`

| Column | |
| --- | --- |
| `id` | Integer primary key. |
| `title` | Nullable text. |
| `content` | Markdown, stored as written. |
| `kind` | `fact`, `decision`, or `note`. Default `note`. |
| `scope` | Text. Default `global`. |
| `source_agent` | Nullable. Informational only. |
| `created_at`, `updated_at` | ISO-8601 UTC (`YYYY-MM-DDTHH:MM:SS.sssZ`). |
| `expires_at` | Nullable, same timestamp format. |
| `content_hash` | SHA-256 hex of the whitespace-normalized content. |

`(scope, content_hash)` is unique. Normalization trims the content and collapses internal whitespace to a single space. Inserting that same normalized content in the same scope returns the existing row and does not change it. The same content in another scope is a different row.

There is no owner. Any agent may update or delete any row.

Reads skip rows whose `expires_at` is at or before the current time unless the caller asks for expired rows. Expired rows can still be updated or deleted by id.

## Tags

`tags.name` is unique. `memory_tags` links a memory to a tag and deletes those links when the memory is deleted. Tag names are trimmed; empty names are dropped.

## Full-text search

`memories_fts` is an FTS5 table over `title`, `content`, and `tags`. Its `rowid` is `memories.id`. Indexed text keeps link labels and drops destinations (`[label](url)` and `![alt](url)`). It also removes `` # ` * _ > ``. The original markdown stays in `memories.content`. Insert, update, and delete refresh the index in the same transaction.

## Later

An `embeddings` table keyed by `memories.id` is not created yet.
