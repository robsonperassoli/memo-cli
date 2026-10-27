use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use memo::resolve_db_path;

/// Shared agent memory stored in SQLite.
#[derive(Parser)]
#[command(name = "memo", version, about)]
struct Cli {
    /// SQLite database path. Overrides `MEMO_DB`.
    ///
    /// When omitted, memo uses `$XDG_DATA_HOME/memo/memo.db`, or
    /// `~/.local/share/memo/memo.db` if `XDG_DATA_HOME` is unset.
    #[arg(long, global = true, value_name = "PATH")]
    db: Option<PathBuf>,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    if let Err(err) = resolve_db_path(cli.db.as_deref()) {
        eprintln!("memo: {err}");
        return ExitCode::from(1);
    }
    ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use clap::CommandFactory;

    use super::Cli;

    #[test]
    fn version_help_and_db_flags_are_present() {
        let mut command = Cli::command();
        command.build();

        assert!(command.get_version().is_some());
        let longs: Vec<_> = command
            .get_arguments()
            .filter_map(|arg| arg.get_long())
            .collect();
        assert!(longs.contains(&"help"));
        assert!(longs.contains(&"db"));
    }
}
