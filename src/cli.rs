//! The command grammar.

use std::path::PathBuf;

use clap::{Parser, Subcommand};
use ctl_core::{EnvPrefix, OutputArgs};

/// Write declared fan curves into the NCT6687D-R through the nct6687 driver.
#[derive(Debug, Parser)]
#[command(
    version,
    about = "Write declared fan curves into the NCT6687D-R through the nct6687 driver",
    arg_required_else_help = true,
    env_prefix = "FANCTL"
)]
pub struct Cli {
    /// Where sysfs is mounted; the driver's device and the board's DMI name
    /// are read below it.
    #[arg(long, global = true, default_value = "/sys", value_hint = clap::ValueHint::DirPath)]
    pub sysfs: PathBuf,

    #[command(flatten)]
    #[expect(
        missing_docs,
        reason = "clap flattens these; their fields carry the help"
    )]
    pub output: OutputArgs,

    #[command(subcommand)]
    #[expect(missing_docs, reason = "each variant documents its subcommand")]
    pub command: Command,
}

/// The subcommands.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Validate a config without touching the hardware.
    Check {
        /// The config file.
        #[arg(value_hint = clap::ValueHint::FilePath)]
        config: PathBuf,
    },
    /// Write every curve in a config, then read every point back.
    Apply {
        /// The config file.
        #[arg(value_hint = clap::ValueHint::FilePath)]
        config: PathBuf,
    },
    /// Print the curves the driver holds now, as a config.
    Show,
    /// Print the operator contract for this version.
    Instructions,
}

#[cfg(test)]
mod tests {
    use clap::CommandFactory;

    use super::*;

    #[test]
    fn the_grammar_is_consistent() {
        Cli::command().debug_assert();
    }

    #[test]
    fn apply_and_check_need_a_config() {
        assert!(Cli::try_parse_from(["fanctl", "apply"]).is_err());
        assert!(Cli::try_parse_from(["fanctl", "check"]).is_err());
        let cli = Cli::try_parse_from(["fanctl", "apply", "fans.yaml"]).unwrap();
        assert_eq!(cli.sysfs, PathBuf::from("/sys"));
        assert!(
            matches!(cli.command, Command::Apply { config } if config == PathBuf::from("fans.yaml"))
        );
    }
}
