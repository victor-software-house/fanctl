//! Each command, from parsed arguments to its [`Outcome`].

use std::path::Path;

use anyhow::{Result, bail};

use crate::apply::{self, fans};
use crate::cli::{Cli, Command};
use crate::config;
use crate::hwmon::{self, Hwmon};
use crate::outcome::Outcome;

/// The operator contract, `fanctl instructions`.
pub const INSTRUCTIONS: &str = include_str!("instructions.md");

/// Run one command.
///
/// # Errors
///
/// A config that does not load, a device or board that is not found, a
/// board that differs from the config's, or any failure to read, write, or
/// verify a curve.
pub fn execute(cli: Cli) -> Result<Outcome> {
    match cli.command {
        Command::Check { config } => {
            let loaded = config::load(&config)?;
            Ok(Outcome::Checked {
                config: config.display().to_string(),
                board: loaded.board,
                fans: fans(&loaded),
            })
        }
        Command::Apply { config } => {
            let loaded = config::load(&config)?;
            let board = hwmon::board(&cli.sysfs)?;
            if board != loaded.board {
                bail!(
                    "{} is for {}, but this board is {board} ({}); nothing was written",
                    config.display(),
                    loaded.board,
                    board.dmi_board_name()
                );
            }
            let device = Hwmon::find(&cli.sysfs)?;
            let fans = apply::apply(&device, &loaded)?;
            Ok(Outcome::Applied {
                device: display(device.dir()),
                board,
                fans,
            })
        }
        Command::Show => {
            let board = hwmon::board(&cli.sysfs)?;
            let device = Hwmon::find(&cli.sysfs)?;
            let config = apply::show(&device, board)?;
            Ok(Outcome::Shown {
                device: display(device.dir()),
                config,
            })
        }
        Command::Instructions => Ok(Outcome::Instructions {
            markdown: INSTRUCTIONS.to_owned(),
        }),
    }
}

fn display(path: &Path) -> String {
    path.display().to_string()
}
