# Getting started

## Install a release

The installers download the latest GitHub Release and put `memo` on `PATH` in the Cargo bin directory (`$CARGO_HOME/bin`, otherwise `~/.cargo/bin`).

macOS and Linux:

```sh
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/robsonperassoli/memo-cli/releases/latest/download/memo-cli-installer.sh | sh
```

Windows (PowerShell):

```powershell
powershell -ExecutionPolicy Bypass -c "irm https://github.com/robsonperassoli/memo-cli/releases/latest/download/memo-cli-installer.ps1 | iex"
```

Each release also publishes an archive per target, a `.sha256` file per archive, and `sha256.sum`. The targets and file names are in [Releasing](releasing.md).

```sh
memo --version
```

## Install from source

The repository is a Cargo workspace. The package is `memo/` and the binary is `memo`. `rust-toolchain.toml` selects the stable channel.

```sh
git clone https://github.com/robsonperassoli/memo-cli.git
cd memo-cli
cargo install --path memo
```

`cargo install` copies `memo` to `$CARGO_HOME/bin` (by default `~/.cargo/bin`). To build without installing:

```sh
cargo build --release
./target/release/memo --version
```

## Quickstart

Examples in this page use `--db ./memo.db`, so they do not write the default database. Delete `./memo.db` first when you want the ids below. On an empty database the first memory is id `1`.

`--scope` defaults to `global`. This stores one global fact and one project decision:

```sh
memo --db ./memo.db add "The deploy window is weekdays." \
  --title "Deploy window" --kind fact

memo --db ./memo.db add "Production deploys go through the staging gate." \
  --title "Deploy gate" --kind decision --scope my-project --tags deploy,process
```

```
added 1
added 2
```

List is newest first. Each line is id, kind, scope, and title:

```sh
memo --db ./memo.db list
```

```
2  decision  my-project  Deploy gate
1  fact  global  Deploy window
```

`search --scope` includes that scope and `global`:

```sh
memo --db ./memo.db search deploy --scope my-project
```

```
1  fact  global  Deploy window
2  decision  my-project  Deploy gate
```

Print one memory:

```sh
memo --db ./memo.db get 2
```

```
id: 2
scope: my-project
kind: decision
title: Deploy gate
tags: deploy, process
source_agent: -
created_at: 2026-10-27T02:24:46.230Z
updated_at: 2026-10-27T02:24:46.230Z
expires_at: -

Production deploys go through the staging gate.
```

`created_at` and `updated_at` are the time of the insert. The remaining commands, flags, and JSON shapes are in [Commands](commands.md).

## Where the file is

Without `--db`, memo opens `$XDG_DATA_HOME/memo/memo.db`, or `~/.local/share/memo/memo.db` when `XDG_DATA_HOME` is unset. `MEMO_DB` overrides that path. Precedence and the schema are in [Data model](data-model.md). The file and its parent directories are created the first time a command opens the database.
