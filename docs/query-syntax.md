# Query syntax

`memo search` matches the full-text index of title, content, and tags. Expired memories are omitted. Hits are ordered by FTS5 `bm25` (a lower rank is a stronger match).

## Terms

Plain terms are combined with `AND`. `--any` combines them with `OR`.

```
memo search deploy rollback
memo search deploy rollback --any
```

A term that contains whitespace is a phrase. A trailing `*` is a prefix, so `deploy*` matches `deployment`.

The index drops link destinations and the symbols `` # ` * _ > ``. Plain terms go through the same cleanup, and a trailing `*` is kept as the prefix operator. `#deploy` therefore matches a heading stored as `# deploy`. The characters in `memories.content` are not changed.

## Raw FTS5

A query is passed to FTS5 unchanged when it uses quotes, parentheses, a column filter (`title:`, `content:`, or `tags:`), or the operators `AND`, `OR`, `NOT`, and `NEAR` together with other tokens. `--any` does not rewrite that query.

```
memo search '"red apple"'
memo search 'deploy OR rollback'
memo search 'NEAR(red apple, 2)'
```

`NEAR(left right, N)` keeps matches whose terms are at most `N` tokens apart. An invalid FTS5 query, including the older `NEAR/N` spelling, prints an error and exits non-zero. It does not panic.

## Filters

`--scope S` returns scope `S` and `global`. `--only-scope S` returns scope `S` only. Omitting both searches every scope. The two flags cannot be combined.

`--kind` and `--tag` restrict matches. `--limit N` caps the number of rows.

## Regular expressions

`--regex PATTERN` is a Rust regular expression applied to the title and the content, not to the index. It works alone or together with terms and the filters above. An invalid pattern is an error.

JSON hits include `rank` and `snippet`. Regex-only hits use rank `0` and a snippet around the match.
