use clap::Parser;

/// Shared agent memory stored in SQLite.
#[derive(Parser)]
#[command(name = "memo", version, about)]
struct Cli {}

fn main() {
    let _cli = Cli::parse();
}

#[cfg(test)]
mod tests {
    use clap::CommandFactory;

    use super::Cli;

    #[test]
    fn version_and_help_flags_are_present() {
        let mut command = Cli::command();
        command.build();

        assert!(command.get_version().is_some());
        assert!(command
            .get_arguments()
            .any(|arg| arg.get_long() == Some("help")));
    }
}
