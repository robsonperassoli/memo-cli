# Query syntax

`memo search` matches the full-text index of title, content, and tags. Expired memories are omitted. FTS5 hits are ordered by `bm25`. A lower `rank` is a stronger match. The flags that do not change the query text (`--scope`, `--only-scope`, `--kind`, `--tag`, `--limit`) are in [Commands](commands.md).

Examples below assume the two rows from [Getting started](getting-started.md):

```sh
memo --db ./memo.db add "The deploy window is weekdays." \
  --title "Deploy window" --kind fact

memo --db ./memo.db add "Production deploys go through the staging gate." \
  --title "Deploy gate" --kind decision --scope my-project --tags deploy,process
```

## Terms

Plain terms are quoted and combined with `AND`. `--any` combines them with `OR`. A single argument that contains whitespace is a phrase.

```sh
memo --db ./memo.db search deploy gate
memo --db ./memo.db search window gate --any
memo --db ./memo.db search "staging gate"
```

`search deploy gate` matches only the project row, because both words are in that body. `search window gate --any` matches both rows. `search "staging gate"` matches the phrase.

A trailing `*` is a prefix, so `deploy*` matches the token `deploy` and a longer token such as `deployment`. The `*` is recognized before markdown characters are removed from the term.

```sh
memo --db ./memo.db search 'deploy*'
```

```
2  decision  my-project  Deploy gate
1  fact  global  Deploy window
```

The index drops link destinations and the symbols `` # ` * _ > ``. Plain terms go through the same cleanup, so `#deploy` matches a body stored as `# deploy`. Link labels are kept and destinations are not: `[docs](https://example.com/deploy)` is indexed as `docs`, not as the URL. The characters stored in `memories.content` are not changed.

A term that is only those symbols, or a term that is empty after trimming, is an error (`memo: invalid search query: ...`).

## Raw FTS5

A query is passed to FTS5 unchanged when it uses quotes, parentheses, a column filter (`title:`, `content:`, or `tags:`), or the operators `AND`, `OR`, `NOT`, and `NEAR` together with other tokens. `--any` does not rewrite that query. A bare operator word with no other tokens is a plain word, so `memo search or` looks for the word `or`.

```sh
memo --db ./memo.db search '"staging gate"'
memo --db ./memo.db search 'deploy OR window'
memo --db ./memo.db search 'title:Deploy'
memo --db ./memo.db search 'NEAR(staging gate, 1)'
```

`NEAR(left right, N)` keeps matches whose terms are at most `N` tokens apart. `NEAR(staging gate, 1)` matches the project row. The older `NEAR/N` spelling is a syntax error on this SQLite build. `memo search 'red NEAR/2 apple'` prints `memo: invalid search query: fts5: syntax error near "/"` and exits `1`. It does not panic.

Column filters use the indexed columns `title`, `content`, and `tags`.

## Scope

`--scope S` returns scope `S` and `global`. `--only-scope S` returns scope `S` only. Omitting both searches every scope. The two flags cannot be combined.

```sh
memo --db ./memo.db search deploy --scope my-project
memo --db ./memo.db search deploy --only-scope my-project
memo --db ./memo.db search deploy
```

On the two-row database, the first command prints ids `1` and `2`. The second prints id `2` only. `list --scope` does not add `global`; that difference is in [Data model](data-model.md).

`--kind` and `--tag` restrict matches. `--limit N` caps the number of rows.

## Regular expressions

`--regex PATTERN` is a Rust regular expression applied to the title and the content joined by a newline, not to the index and not to tags. It works alone or together with terms and the filters above. An invalid pattern exits `1` with `memo: invalid regular expression: ...`.

Regex-only results are ordered newest first, with `rank` `0`. The snippet is a window around the match, about 48 characters on each side. When both terms and `--regex` are set, a row has to match the full-text query and the pattern. The limit is applied after that filter.

```sh
memo --db ./memo.db search --regex 'weekdays|staging'
memo --db ./memo.db search deploy --regex 'gate' --kind decision --limit 10
```

```
2  decision  my-project  Deploy gate Production deploys go through the staging gate.
1  fact  global  Deploy window The deploy window is weekdays.
```

Text output joins the snippet onto one line. `memo search --json --regex 'weekdays|staging'` keeps the newline inside `snippet`.
