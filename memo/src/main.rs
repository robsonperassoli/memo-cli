mod run;

use std::process::ExitCode;

fn main() -> ExitCode {
    match run::run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::from(1)
        }
    }
}
