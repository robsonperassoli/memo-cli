# memo-cli

`memo` is a Rust command-line tool that will store shared agent memory in SQLite.

This repository is scaffolding. The binary accepts `--help` and `--version` only. Storage, subcommands, and query syntax are not implemented.

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
