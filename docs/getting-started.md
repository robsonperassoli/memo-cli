# Getting started

Install the latest `memo` binary from GitHub Releases.

macOS and Linux:

```sh
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/robsonperassoli/memo-cli/releases/latest/download/memo-cli-installer.sh | sh
```

Windows (PowerShell):

```powershell
powershell -ExecutionPolicy Bypass -c "irm https://github.com/robsonperassoli/memo-cli/releases/latest/download/memo-cli-installer.ps1 | iex"
```

The installer puts `memo` in the Cargo bin directory (`$CARGO_HOME/bin`, otherwise `~/.cargo/bin`). Each release also has archives and sha256 checksums.

Database location and overrides are in [Data model](data-model.md). The binary does not create the database until storage is opened. No read or write commands exist yet.
