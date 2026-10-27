# memo-cli

`memo` is a Rust command-line tool that stores shared agent memory in SQLite.

The binary accepts `--help`, `--version`, and `--db`. Subcommands are not implemented yet. The database path, schema, and storage rules are in [docs/data-model.md](docs/data-model.md).

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
