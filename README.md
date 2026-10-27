# memo-cli

`memo` is a Rust command-line tool that stores shared agent memory in SQLite.

## Install

macOS and Linux:

```sh
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/robsonperassoli/memo-cli/releases/latest/download/memo-cli-installer.sh | sh
```

Windows (PowerShell):

```powershell
powershell -ExecutionPolicy Bypass -c "irm https://github.com/robsonperassoli/memo-cli/releases/latest/download/memo-cli-installer.ps1 | iex"
```

Install from source, a short quickstart, and where the database file goes are in [Getting started](docs/getting-started.md).

## Documentation

- [Getting started](docs/getting-started.md) — release installers, install from source, and an end-to-end walkthrough
- [Commands](docs/commands.md) — `add`, `get`, `update`, `forget`, `list`, `search`, `scopes`, `schema`, and `sql`, with every flag
- [Query syntax](docs/query-syntax.md) — FTS5 (`AND`, `--any`, prefix, `OR` / `NOT` / phrases / `NEAR`), `--regex`, ranking, and read-only SQL
- [Data model](docs/data-model.md) — `MEMO_DB`, `XDG_DATA_HOME`, scopes, the global scope, and the `memories` view
- [Skill](docs/skill.md) — install the agent skill
- [Releasing](docs/releasing.md) — tag a version and publish a GitHub Release

Documentation index: [docs/README.md](docs/README.md).
