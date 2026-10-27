# memo-cli

`memo` is a Rust command-line tool that stores shared agent memory in SQLite.

`memo add`, `get`, `update`, `forget`, `list`, `search`, and `scopes` read and write the database. Paths, schema, and storage rules are in [docs/data-model.md](docs/data-model.md). Commands are in [docs/commands.md](docs/commands.md).

## Install

macOS and Linux:

```sh
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/robsonperassoli/memo-cli/releases/latest/download/memo-cli-installer.sh | sh
```

Windows (PowerShell):

```powershell
powershell -ExecutionPolicy Bypass -c "irm https://github.com/robsonperassoli/memo-cli/releases/latest/download/memo-cli-installer.ps1 | iex"
```

Documentation index: [docs/README.md](docs/README.md).
