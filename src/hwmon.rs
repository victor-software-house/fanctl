//! The `nct6687` driver's hwmon attributes in sysfs, and how fanctl finds
//! them.

use std::path::{Path, PathBuf};
use std::{fs, io};

use thiserror::Error;

use crate::board::{Board, Channel};
use crate::curve::{POINTS, Registers, Sequence};

/// The hwmon `name` the driver registers.
pub const DRIVER: &str = "nct6687";

/// Reads and writes a device's attribute files by name.
///
/// [`Hwmon`] is the real device. The trait exists so the write cycle runs
/// against a model of the driver in tests, which refuses the writes the
/// driver refuses.
pub trait Attributes {
    /// The attribute's contents, without the trailing newline.
    ///
    /// # Errors
    ///
    /// The I/O error the read returned.
    fn read(&self, name: &str) -> io::Result<String>;

    /// Write one value to the attribute.
    ///
    /// # Errors
    ///
    /// The I/O error the write returned; for the driver, `EINVAL`, `EBUSY` or
    /// `EIO` when it refuses the value.
    fn write(&self, name: &str, value: &str) -> io::Result<()>;
}

/// The driver's device directory, such as `/sys/class/hwmon/hwmon3`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hwmon {
    dir: PathBuf,
}

/// Why the device or the board was not found.
#[derive(Debug, Error)]
pub enum FindError {
    /// Listing the hwmon class failed.
    #[error("listing {path}: {source}")]
    List {
        /// The directory listed.
        path: String,
        /// The I/O error.
        source: io::Error,
    },
    /// No hwmon device is named `nct6687`.
    #[error("no hwmon device is named {DRIVER} under {0}; the nct6687 module may not be loaded")]
    Missing(String),
    /// More than one is.
    #[error("more than one hwmon device is named {DRIVER}: {first} and {second}")]
    Ambiguous {
        /// The first device directory, in path order.
        first: String,
        /// The second.
        second: String,
    },
    /// The board's DMI name could not be read.
    #[error("reading the board name from {path}: {source}")]
    Dmi {
        /// The file read.
        path: String,
        /// The I/O error.
        source: io::Error,
    },
    /// The board is not one fanctl has a table for.
    #[error("the board is {0:?}, which fanctl has no header table for")]
    UnknownBoard(String),
}

impl Hwmon {
    /// A device at a known directory.
    #[must_use]
    pub const fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    /// The one device under `<sysfs>/class/hwmon` whose `name` is
    /// [`DRIVER`]. The `hwmonN` number changes between boots, so it is never
    /// assumed.
    ///
    /// # Errors
    ///
    /// [`FindError::List`] when the class cannot be listed,
    /// [`FindError::Missing`] when no device matches, and
    /// [`FindError::Ambiguous`] when two do.
    pub fn find(sysfs: &Path) -> Result<Self, FindError> {
        let class = sysfs.join("class/hwmon");
        let entries = fs::read_dir(&class).map_err(|source| FindError::List {
            path: class.display().to_string(),
            source,
        })?;
        let mut dirs: Vec<PathBuf> = entries
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|dir| {
                fs::read_to_string(dir.join("name")).is_ok_and(|name| name.trim_end() == DRIVER)
            })
            .collect();
        dirs.sort();
        match dirs.as_slice() {
            [] => Err(FindError::Missing(class.display().to_string())),
            [dir] => Ok(Self::new(dir.clone())),
            [first, second, ..] => Err(FindError::Ambiguous {
                first: first.display().to_string(),
                second: second.display().to_string(),
            }),
        }
    }

    /// The device directory.
    #[must_use]
    pub fn dir(&self) -> &Path {
        &self.dir
    }
}

impl Attributes for Hwmon {
    fn read(&self, name: &str) -> io::Result<String> {
        fs::read_to_string(self.dir.join(name)).map(|text| text.trim_end().to_owned())
    }

    fn write(&self, name: &str, value: &str) -> io::Result<()> {
        fs::write(self.dir.join(name), value)
    }
}

/// The board this machine is, from `<sysfs>/class/dmi/id/board_name`.
///
/// # Errors
///
/// [`FindError::Dmi`] when the file cannot be read, and
/// [`FindError::UnknownBoard`] for a board with no table.
pub fn board(sysfs: &Path) -> Result<Board, FindError> {
    let path = sysfs.join("class/dmi/id/board_name");
    let name = fs::read_to_string(&path).map_err(|source| FindError::Dmi {
        path: path.display().to_string(),
        source,
    })?;
    let name = name.trim_end();
    Board::from_dmi(name).ok_or_else(|| FindError::UnknownBoard(name.to_owned()))
}

/// The attribute file of one point: `pwmN_auto_pointM_temp` or `_pwm`, with
/// `point` from 0.
#[must_use]
pub fn attribute(channel: Channel, point: usize, sequence: Sequence) -> String {
    format!(
        "pwm{}_auto_point{}_{}",
        channel.number(),
        point + 1,
        sequence.suffix()
    )
}

/// A value the driver wrote to sysfs, in the units of its sequence: degrees C
/// for a temperature, which the driver writes in millidegrees, and 0-255 for
/// a duty.
///
/// # Errors
///
/// A message naming the attribute when the text is not such a value.
pub fn parse_value(name: &str, text: &str, sequence: Sequence) -> Result<u8, String> {
    let bad = || format!("{name} reads {text:?}, which is not a value the driver writes");
    let value = text.parse::<u32>().ok().ok_or_else(bad)?;
    let value = match sequence {
        Sequence::Temp if value % 1000 == 0 => value / 1000,
        Sequence::Temp => return Err(bad()),
        Sequence::Pwm => value,
    };
    u8::try_from(value).ok().ok_or_else(bad)
}

/// The text fanctl writes for a value: millidegrees for a temperature.
#[must_use]
pub fn format_value(value: u8, sequence: Sequence) -> String {
    match sequence {
        Sequence::Temp => (u32::from(value) * 1000).to_string(),
        Sequence::Pwm => value.to_string(),
    }
}

/// Why a curve could not be read.
#[derive(Debug, Error)]
#[error("{header} ({channel}): {problem}")]
pub struct ReadError {
    /// The header read.
    pub header: String,
    /// Its channel.
    pub channel: Channel,
    /// What went wrong, naming the attribute.
    pub problem: String,
}

/// Read one channel's whole curve.
///
/// # Errors
///
/// [`ReadError`] naming the attribute that could not be read or did not hold
/// a value; a missing attribute means the driver has no curve for the
/// header.
pub fn read_registers(
    device: &dyn Attributes,
    header: &str,
    channel: Channel,
) -> Result<Registers, ReadError> {
    let mut registers = Registers {
        temp: [0; POINTS],
        pwm: [0; POINTS],
    };
    for sequence in Sequence::ALL {
        let values = match sequence {
            Sequence::Temp => &mut registers.temp,
            Sequence::Pwm => &mut registers.pwm,
        };
        let slots = values.iter_mut().enumerate();
        for (point, value) in slots {
            let name = attribute(channel, point, sequence);
            let problem = |problem: String| ReadError {
                header: header.to_owned(),
                channel,
                problem,
            };
            let text = device.read(&name).map_err(|error| {
                problem(if error.kind() == io::ErrorKind::NotFound {
                    format!("{name} does not exist; the driver exposes no curve for this header")
                } else {
                    format!("reading {name}: {error}")
                })
            })?;
            *value = parse_value(&name, &text, sequence).map_err(problem)?;
        }
    }
    Ok(registers)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_parse_in_the_units_of_their_sequence() {
        assert_eq!(parse_value("t", "55000", Sequence::Temp), Ok(55));
        assert!(parse_value("t", "55500", Sequence::Temp).is_err());
        assert!(parse_value("t", "-1000", Sequence::Temp).is_err());
        assert_eq!(parse_value("p", "255", Sequence::Pwm), Ok(255));
        assert!(parse_value("p", "256", Sequence::Pwm).is_err());
        assert_eq!(format_value(55, Sequence::Temp), "55000");
        assert_eq!(format_value(102, Sequence::Pwm), "102");
    }

    #[test]
    fn attribute_names_count_points_from_one() {
        let channel = crate::board::Board::Ms7e70
            .header("SYS_FAN2")
            .map(|header| header.channel);
        assert_eq!(
            channel.map(|channel| attribute(channel, 6, Sequence::Pwm)),
            Some("pwm4_auto_point7_pwm".to_owned())
        );
    }
}
