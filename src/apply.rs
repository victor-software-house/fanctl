//! The write cycle: read every configured curve, write every point in an
//! order the driver accepts, read every point back, compare.

use std::{fmt, io};

use serde::Serialize;
use thiserror::Error;

use crate::board::{Board, Channel, Header};
use crate::config::Config;
use crate::curve::{
    Curve, Fall, POINTS, RangeError, Registers, Sequence, StoredPoint, write_order,
};
use crate::hwmon::{Attributes, ReadError, attribute, format_value, read_registers};

/// One fan and its curve.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Fan {
    /// The board header.
    pub header: &'static str,
    /// The driver's channel.
    pub channel: Channel,
    /// The curve.
    pub curve: Curve,
}

/// The configured fans, in the board's channel order.
///
/// A [`Config`] names only headers its board has, so every name resolves.
#[must_use]
pub fn fans(config: &Config) -> Vec<Fan> {
    config
        .board
        .headers()
        .iter()
        .filter_map(|&Header { name, channel }| {
            config.fans.get(name).map(|&curve| Fan {
                header: name,
                channel,
                curve,
            })
        })
        .collect()
}

/// Why an apply stopped.
#[derive(Debug, Error)]
pub enum ApplyError {
    /// A curve could not be read; nothing was written.
    #[error("{0}; nothing was written")]
    Read(#[from] ReadError),
    /// The driver holds a falling curve, so it would refuse every write;
    /// nothing was written.
    #[error(
        "{header} ({channel}): the driver holds a curve where {fall}, and refuses every write \
         to it; nothing was written"
    )]
    CurrentFalls {
        /// The header.
        header: &'static str,
        /// Its channel.
        channel: Channel,
        /// Where the stored curve falls.
        fall: Fall,
    },
    /// The driver refused a write. Writes before it stay written.
    #[error(
        "{header} ({channel}): writing {value} to {attribute} failed: {reason}; fanctl stopped \
         there, and the points written before it stay written"
    )]
    Write {
        /// The header.
        header: &'static str,
        /// Its channel.
        channel: Channel,
        /// The attribute file.
        attribute: String,
        /// The text written.
        value: String,
        /// The error the driver returned, explained.
        reason: String,
    },
    /// Points that do not read back as written.
    #[error("{} points do not read back as written:\n{}", .0.len(), list(.0))]
    Mismatch(Vec<Mismatch>),
}

/// A point that reads back other than written.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Mismatch {
    /// The header.
    pub header: &'static str,
    /// Its channel.
    pub channel: Channel,
    /// The point, from 1.
    pub point: usize,
    /// The values written.
    pub wrote: StoredPoint,
    /// The values read back.
    pub read: StoredPoint,
}

impl fmt::Display for Mismatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} ({}) point {}: wrote {}; reads {}",
            self.header, self.channel, self.point, self.wrote, self.read
        )
    }
}

fn list(mismatches: &[Mismatch]) -> String {
    mismatches
        .iter()
        .map(|mismatch| format!("  {mismatch}"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// What the error of a refused write means.
fn reason(error: &io::Error) -> String {
    if error.kind() == io::ErrorKind::ResourceBusy {
        format!("{error}: the channel is under manual control, so the chip runs no curve on it")
    } else if error.kind() == io::ErrorKind::InvalidInput {
        format!(
            "{error}: the driver refused the value as out of range or leaving the curve falling"
        )
    } else {
        error.to_string()
    }
}

/// Write every configured curve and verify it.
///
/// Every current curve is read and checked before the first write. Each
/// sequence is then written in [`write_order`], every point included, and
/// every point is read back at the end.
///
/// # Errors
///
/// [`ApplyError`]: a curve that cannot be read or already falls stops the
/// apply before any write; a refused write stops it at that write; points
/// that read back otherwise are listed together.
pub fn apply(device: &dyn Attributes, config: &Config) -> Result<Vec<Fan>, ApplyError> {
    let fans = fans(config);
    let mut current = Vec::with_capacity(fans.len());
    for fan in &fans {
        let registers = read_registers(device, fan.header, fan.channel)?;
        if let Some(fall) = registers.fall() {
            return Err(ApplyError::CurrentFalls {
                header: fan.header,
                channel: fan.channel,
                fall,
            });
        }
        current.push(registers);
    }
    let writes = fans.iter().zip(&current);
    for (fan, current) in writes {
        write_curve(device, fan, current)?;
    }
    let mut mismatches = vec![];
    for fan in &fans {
        let read = read_registers(device, fan.header, fan.channel)?;
        mismatches.extend(compare(fan, &fan.curve.registers(), &read));
    }
    if mismatches.is_empty() {
        Ok(fans)
    } else {
        Err(ApplyError::Mismatch(mismatches))
    }
}

fn write_curve(device: &dyn Attributes, fan: &Fan, current: &Registers) -> Result<(), ApplyError> {
    let target = fan.curve.registers();
    for sequence in Sequence::ALL {
        let values = target.sequence(sequence);
        let order = write_order(current.sequence(sequence), values);
        for point in order {
            let Some(&value) = values.get(point) else {
                continue;
            };
            let name = attribute(fan.channel, point, sequence);
            let text = format_value(value, sequence);
            device
                .write(&name, &text)
                .map_err(|error| ApplyError::Write {
                    header: fan.header,
                    channel: fan.channel,
                    attribute: name,
                    value: text,
                    reason: reason(&error),
                })?;
        }
    }
    Ok(())
}

fn compare(fan: &Fan, wrote: &Registers, read: &Registers) -> Vec<Mismatch> {
    (0..POINTS)
        .filter_map(|point| {
            let (wrote, read) = (wrote.point(point)?, read.point(point)?);
            (wrote != read).then_some(Mismatch {
                header: fan.header,
                channel: fan.channel,
                point: point + 1,
                wrote,
                read,
            })
        })
        .collect()
}

/// Why the current curves could not be shown.
#[derive(Debug, Error)]
pub enum ShowError {
    /// A curve could not be read.
    #[error(transparent)]
    Read(#[from] ReadError),
    /// A point holds a value the config cannot state.
    #[error("{header} ({channel}): {source}")]
    Range {
        /// The header.
        header: &'static str,
        /// Its channel.
        channel: Channel,
        /// The value.
        source: RangeError,
    },
}

/// The curves the driver holds now, as a config. A header whose
/// attributes do not exist, because the driver exposes no curve for it, is
/// left out.
///
/// # Errors
///
/// [`ShowError`] when an existing attribute cannot be read or holds a value
/// the config cannot state.
pub fn show(device: &dyn Attributes, board: Board) -> Result<Config, ShowError> {
    let mut config = Config {
        board,
        fans: std::collections::BTreeMap::new(),
    };
    for &Header { name, channel } in board.headers() {
        let first = attribute(channel, 0, Sequence::Temp);
        if device
            .read(&first)
            .is_err_and(|error| error.kind() == io::ErrorKind::NotFound)
        {
            continue;
        }
        let curve = read_registers(device, name, channel)?
            .curve()
            .map_err(|source| ShowError::Range {
                header: name,
                channel,
                source,
            })?;
        config.fans.insert(name.to_owned(), curve);
    }
    Ok(config)
}
