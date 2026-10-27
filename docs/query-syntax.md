# Query syntax

`memo search` matches the full-text index of title, content, and tags. Expired memories are omitted. The examples in the search sections share `./memo.db` and run in order:

```sh
rm -f ./memo.db

memo --db ./memo.db add "The deploy window is weekdays." \
  --title "Deploy window" --kind fact

memo --db ./memo.db add "Production deploys go through the staging gate." \
  --title "Deploy gate" --kind decision --scope my-project --tags deploy,process
```

That prints `added 1` and `added 2`. Flags that do not change the query text are listed with [memo search](commands.md).

## AND

Plain terms are quoted and combined with `AND`. A single argument that contains whitespace is a phrase. Every term has to match.

```sh
memo --db ./memo.db search deploy gate
memo --db ./memo.db search "staging gate"
```

`deploy gate` matches only id `2` (the global row has `deploy` and not `gate`). `"staging gate"` matches that same row as a phrase.

## --any

`--any` joins plain terms with `OR` instead of `AND`. It does not rewrite a raw FTS5 query.

```sh
memo --db ./memo.db search window gate --any
```

Both rows match: id `1` contains `window`, id `2` contains `gate`.

## Prefix

A trailing `*` is a prefix, so `deploy*` matches the token `deploy` and a longer token such as `deployment`. The `*` is recognized before markdown characters are removed from the term.

```sh
memo --db ./memo.db search 'deploy*'
```

```
2  decision  my-project  Deploy gate
1  fact  global  Deploy window
```

The index drops link destinations and the symbols `` # ` * _ > ``. Plain terms go through the same cleanup, so `#deploy` matches a body stored as `# deploy`. Link labels are kept and destinations are not: `[docs](https://example.com/deploy)` is indexed as `docs`, not as the URL. `memories.content` is stored unchanged.

A term that is only those symbols, or a blank term, is an error (`memo: invalid search query: ...`) and exits `1`.

## Raw FTS5

A query is passed to FTS5 unchanged when it uses quotes, parentheses, a column filter (`title:`, `content:`, or `tags:`), or the operators `AND`, `OR`, `NOT`, and `NEAR` together with other tokens. A bare operator word with no other tokens is a plain word, so `memo search or` looks for the word `or`.

### OR

```sh
memo --db ./memo.db search 'deploy OR window'
```

Both rows match.

### NOT

```sh
memo --db ./memo.db search 'deploy NOT window'
```

Only id `2` matches. Id `1` contains `window`, so `NOT` removes it.

### Phrases

Quotes mark a phrase. The quoted form is raw FTS5, unlike a plain multi-word argument, which memo quotes itself.

```sh
memo --db ./memo.db search '"staging gate"'
```

Id `2` matches.

### NEAR

`NEAR(left right, N)` keeps matches whose terms are at most `N` tokens apart.

```sh
memo --db ./memo.db search 'NEAR(staging gate, 1)'
```

Id `2` matches. The older `NEAR/N` spelling is a syntax error on this SQLite build. It exits `1` and does not panic:

```sh
# expect 1
memo --db ./memo.db search 'red NEAR/2 apple'
```

```
memo: invalid search query: fts5: syntax error near "/"
```

Column filters use the indexed columns `title`, `content`, and `tags`:

```sh
memo --db ./memo.db search 'title:Deploy'
```

## --regex

`--regex PATTERN` is a Rust regular expression applied to the title and the content joined by a newline. It does not use the index and it does not see tags. It works alone or together with terms and the filters below. An invalid pattern exits `1` with `memo: invalid regular expression: ...`.

Regex-only results are ordered newest first, with `rank` `0`. The snippet is a window around the match: 48 bytes on each side, on a character boundary. When both terms and `--regex` are set, a row has to match the full-text query and the pattern. `--limit` is applied after that filter.

```sh
memo --db ./memo.db search --regex 'weekdays|staging'
memo --db ./memo.db search deploy --regex 'gate' --kind decision --limit 10
```

```
2  decision  my-project  Deploy gate Production deploys go through the staging gate.
1  fact  global  Deploy window The deploy window is weekdays.
```

Text output joins the snippet onto one line. `memo search --json` keeps the newline inside `snippet`.

## Scope

`--scope S` returns scope `S` and `global`. `--only-scope S` returns scope `S` only. Omitting both searches every scope. The two flags cannot be combined (exit `2`).

```sh
memo --db ./memo.db search deploy --scope my-project
memo --db ./memo.db search deploy --only-scope my-project
memo --db ./memo.db search deploy
```

The first command prints ids `1` and `2`. The second prints id `2` only. The third prints both. `memo list --scope` does not add `global`; that difference is in [Data model](data-model.md).

`--kind` and `--tag` restrict matches. `--limit N` caps the number of rows.

## Ranking

FTS5 hits are ordered by `bm25`. A lower `rank` is a stronger match. Text output follows that order. `memo search --json` adds `rank` and `snippet` to each memory object. On the two-row database, `memo search deploy` returns id `1` and then id `2`.

```sh
memo --db ./memo.db search deploy
memo --db ./memo.db --json search deploy --limit 1
```

Regex-only hits use rank `0` and are ordered newest first, not by `bm25`.

## SQL access

`memo sql` runs one read-only statement. The file is migrated if needed, then opened with `SQLITE_OPEN_READ_ONLY` and `PRAGMA query_only`. The command's flags and JSON shape are in [Commands](commands.md). These examples use the same `./memo.db`.

### memories view

`memories` is the stable view. Columns: `id`, `title`, `content`, `kind`, `scope`, `tags`, `source_agent`, `created_at`, `updated_at`, `expires_at`. `tags` is a JSON array stored as text. The view includes expired rows; `content_hash` is not a column. Filter `expires_at` when a query should skip expired rows. Compare it to the same ISO-8601 UTC text memo stores (`YYYY-MM-DDTHH:MM:SS.sssZ`).

```sh
memo --db ./memo.db sql \
  "SELECT id, scope, title, tags FROM memories ORDER BY id"

memo --db ./memo.db --json sql \
  "SELECT id, tags FROM memories WHERE id = 2"
```

Text columns are padded and separated by two spaces. JSON is an array of objects. Keys are sorted alphabetically, and `tags` stays a string:

```json
[{"id":2,"tags":"[\"deploy\",\"process\"]"}]
```

### memories_fts

`memories_fts` is the FTS5 table over indexed `title`, `content`, and `tags`. `MATCH` it and join `rowid` to `memories.id`.

```sh
memo --db ./memo.db sql \
  "SELECT memories.id, memories.title
   FROM memories
   JOIN memories_fts ON memories_fts.rowid = memories.id
   WHERE memories_fts MATCH 'deploy'
   ORDER BY memories.id"
```

```
id  title
1   Deploy window
2   Deploy gate
```

### Read-only

`INSERT`, `UPDATE`, `DELETE`, `DROP`, `ATTACH`, and a `PRAGMA` that assigns a value fail with `memo: SQL access is read-only` and exit `1`. A read-only `PRAGMA` is allowed. More than one statement fails with `memo: database error: Multiple statements provided`. An empty query fails with `memo: SQL query must not be empty`.

```sh
# expect 1
memo --db ./memo.db sql "DELETE FROM memory_rows"
```

### Row cap

`--max-rows` defaults to `1000`. When another row exists past the cap, memo keeps the first N rows, prints `memo: results truncated to N rows` on stderr, and exits `0`.

```sh
memo --db ./memo.db sql "WITH RECURSIVE c(n) AS (
  SELECT 1 UNION ALL SELECT n + 1 FROM c WHERE n < 30
) SELECT n FROM c" --max-rows 2
```

Stdout is the header and the values `1` and `2`. Stderr is `memo: results truncated to 2 rows`.

### Timeout

A query that runs longer than 5 seconds is cancelled. The process exits `1` with `memo: query exceeded the 5s time limit`.

```sh
# expect 1
memo --db ./memo.db sql "WITH RECURSIVE c(n) AS (
  SELECT 1 UNION ALL SELECT n + 1 FROM c WHERE n < 100000000
) SELECT sum(n) FROM c"
```
