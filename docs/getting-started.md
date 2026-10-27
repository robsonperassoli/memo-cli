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

`memo --version` prints `memo 0.1.0` for this release.

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

## Walkthrough

One database, from the first write through search, update, SQL, and delete. The file is `./memo.db`, so this does not touch the default database. Delete it first. On an empty database the first memory is id `1`. `--scope` defaults to `global`.

```sh
rm -f ./memo.db

memo --db ./memo.db add "The deploy window is weekdays." \
  --title "Deploy window" --kind fact

memo --db ./memo.db add "Production deploys go through the staging gate." \
  --title "Deploy gate" --kind decision --scope my-project --tags deploy,process

memo --db ./memo.db list

memo --db ./memo.db search deploy --scope my-project

memo --db ./memo.db get 2

memo --db ./memo.db update 2 \
  --content "Production deploys go through staging, then production."

memo --db ./memo.db sql "SELECT id, scope, title FROM memories ORDER BY id"

memo --db ./memo.db forget 1

memo --db ./memo.db scopes
```

The two adds print `added 1` and `added 2`. `list` is newest first:

```
2  decision  my-project  Deploy gate
1  fact  global  Deploy window
```

`search --scope my-project` includes that scope and `global`, so both rows match `deploy`. `get 2` prints the fields (`id`, `scope`, `kind`, `title`, `tags`, `source_agent`, `created_at`, `updated_at`, `expires_at`) and then the body. `update` prints `updated 2`. The SQL statement reads the `memories` view, including the new body stored under the same title:

```
id  scope       title
1   global      Deploy window
2   my-project  Deploy gate
```

`forget 1` prints `forgot 1`. `scopes` still lists `global` with count `0`, then `my-project` with the remaining memory.

Commands and flags are in [Commands](commands.md). Search syntax and `memo sql` are in [Query syntax](query-syntax.md). The default database path is in [Data model](data-model.md).
