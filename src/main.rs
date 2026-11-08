//! The fanctl binary.

use ctl_core::prelude::{App, ExitCode};
use fanctl::cli::Cli;

fn main() -> ExitCode {
    App::<Cli>::new("fanctl")
        .view(|cli| cli.output.view())
        .run(fanctl::run::execute)
}
