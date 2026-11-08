//! What each command returns: one serializable value, with its human
//! presentation. JSON serializes the value itself.

use ctl_core::{Document, Fields, Present, Text};
use serde::Serialize;

use crate::apply::Fan;
use crate::board::Board;
use crate::config::Config;

/// A command's result.
#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Outcome {
    /// `fanctl check`: the config is valid.
    Checked {
        /// The config file.
        config: String,
        /// Its board.
        board: Board,
        /// Its fans, in channel order.
        fans: Vec<Fan>,
    },
    /// `fanctl apply`: every curve written and read back as written.
    Applied {
        /// The driver's device directory.
        device: String,
        /// The board.
        board: Board,
        /// The fans written, in channel order.
        fans: Vec<Fan>,
    },
    /// `fanctl show`.
    Shown {
        /// The driver's device directory.
        device: String,
        /// The curves the driver holds, as a config.
        config: Config,
    },
    /// `fanctl instructions`.
    Instructions {
        /// The operator contract.
        markdown: String,
    },
}

impl Present for Outcome {
    fn present(&self) -> Document {
        match self {
            Self::Checked {
                config,
                board,
                fans,
            } => Document::new()
                .paragraph(
                    Text::new()
                        .success("valid")
                        .then("  ")
                        .value(config)
                        .then(format!("  {board}")),
                )
                .fields(fan_fields(fans)),
            Self::Applied {
                device,
                board,
                fans,
            } => Document::new()
                .paragraph(
                    Text::new()
                        .success("applied")
                        .then("  ")
                        .value(device)
                        .then(format!("  {board}")),
                )
                .fields(fan_fields(fans)),
            Self::Shown { config, .. } => Document::new().verbatim(config.to_yaml()),
            Self::Instructions { markdown } => Document::new().verbatim(markdown.clone()),
        }
    }
}

fn fan_fields(fans: &[Fan]) -> Fields {
    fans.iter().fold(Fields::new(), |fields, fan| {
        let points: Vec<String> = fan
            .curve
            .0
            .iter()
            .map(|point| format!("{} C {} %", point.temp.degrees(), point.duty))
            .collect();
        fields.row(
            format!("{} ({})", fan.header, fan.channel),
            Text::plain(points.join(", ")),
        )
    })
}
