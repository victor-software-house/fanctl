//! A model of the `nct6687` driver's curve attributes, as its store path in
//! Fred78290/nct6687d#228 behaves (`nct6687_curve_temp_store`,
//! `nct6687_curve_pwm_store`, `nct6687_store_curve_point`):
//!
//! - a temperature is parsed as millidegrees, clamped to 0..=127000, and
//!   rounded to whole degrees; a duty outside 0..=255 is `EINVAL`;
//! - a channel under manual control is `EBUSY`;
//! - the point is set on the curve the device holds now, and a curve whose
//!   temperatures or duties then fall is `EINVAL`, with nothing written.
//!
//! A stuck attribute takes a write and keeps its old value, as a device
//! whose read-back differs.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::{fs, io};

use fanctl::curve::{POINTS, Sequence};
use fanctl::hwmon::Attributes;

/// The model's state: every attribute file's text, by name.
#[derive(Debug, Default)]
pub(crate) struct Driver {
    files: RefCell<BTreeMap<String, String>>,
    /// Channels under manual control.
    pub(crate) manual: BTreeSet<u8>,
    /// Attributes that take a write and keep their value.
    pub(crate) stuck: BTreeSet<String>,
    /// Every write the model accepted, in order.
    pub(crate) writes: RefCell<Vec<Write>>,
}

/// One write the model accepted.
#[derive(Debug)]
pub(crate) struct Write {
    /// The attribute file.
    pub(crate) name: String,
    /// The text written.
    pub(crate) value: String,
}

/// One curve attribute, as the driver names it.
struct CurveAttribute {
    channel: u8,
    /// From 0.
    index: usize,
    sequence: Sequence,
}

impl CurveAttribute {
    /// `pwmN_auto_pointM_temp` or `_pwm`, or `None` for any other name.
    fn parse(name: &str) -> Option<Self> {
        let rest = name.strip_prefix("pwm")?;
        let (channel, rest) = rest.split_once("_auto_point")?;
        let (point, suffix) = rest.split_once('_')?;
        let sequence = match suffix {
            "temp" => Sequence::Temp,
            "pwm" => Sequence::Pwm,
            _ => return None,
        };
        let point: usize = point.parse().ok()?;
        Some(Self {
            channel: channel.parse().ok()?,
            index: point.checked_sub(1).filter(|&index| index < POINTS)?,
            sequence,
        })
    }
}

/// The sequence the driver checks beside the one written.
const fn other(sequence: Sequence) -> Sequence {
    match sequence {
        Sequence::Temp => Sequence::Pwm,
        Sequence::Pwm => Sequence::Temp,
    }
}

fn invalid() -> io::Error {
    io::Error::from(io::ErrorKind::InvalidInput)
}

fn falls(values: &[i64]) -> bool {
    values.windows(2).any(|pair| pair[1] < pair[0])
}

impl Driver {
    /// The files of one device directory of a fixture tree.
    pub(crate) fn load(dir: &Path) -> Self {
        let entries =
            fs::read_dir(dir).unwrap_or_else(|error| panic!("list {}: {error}", dir.display()));
        let mut files = BTreeMap::new();
        for entry in entries {
            let entry = entry.unwrap_or_else(|error| panic!("list {}: {error}", dir.display()));
            let text = fs::read_to_string(entry.path())
                .unwrap_or_else(|error| panic!("read {}: {error}", entry.path().display()));
            files.insert(
                entry.file_name().to_string_lossy().into_owned(),
                text.trim_end().to_owned(),
            );
        }
        Self {
            files: RefCell::new(files),
            ..Self::default()
        }
    }

    /// The firmware curves of the MS-7E70 fixture.
    pub(crate) fn ms_7e70() -> Self {
        Self::load(&super::testdata("sysfs/ms-7e70/class/hwmon/hwmon3"))
    }

    /// Set a file's text directly, as the firmware would.
    pub(crate) fn set(&self, name: &str, text: &str) {
        self.files
            .borrow_mut()
            .insert(name.to_owned(), text.to_owned());
    }

    /// Remove every file whose name starts with `prefix`.
    pub(crate) fn remove(&self, prefix: &str) {
        self.files
            .borrow_mut()
            .retain(|name, _| !name.starts_with(prefix));
    }

    /// A file's text.
    pub(crate) fn get(&self, name: &str) -> Option<String> {
        self.files.borrow().get(name).cloned()
    }

    /// One sequence of a channel's curve, as stored values.
    fn sequence(&self, channel: u8, sequence: Sequence) -> io::Result<Vec<i64>> {
        let suffix = sequence.suffix();
        (1..=POINTS)
            .map(|point| {
                self.read(&format!("pwm{channel}_auto_point{point}_{suffix}"))?
                    .parse::<i64>()
                    .ok()
                    .ok_or_else(invalid)
            })
            .collect()
    }
}

impl Attributes for Driver {
    fn read(&self, name: &str) -> io::Result<String> {
        self.get(name)
            .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))
    }

    fn write(&self, name: &str, value: &str) -> io::Result<()> {
        let attribute = CurveAttribute::parse(name)
            .ok_or_else(|| io::Error::from(io::ErrorKind::PermissionDenied))?;
        let value = value.trim().parse::<i64>().ok().ok_or_else(invalid)?;
        let stored = match attribute.sequence {
            Sequence::Temp => (value.clamp(0, 127_000) + 500) / 1000 * 1000,
            Sequence::Pwm if (0..=255).contains(&value) => value,
            Sequence::Pwm => return Err(invalid()),
        };
        if self.manual.contains(&attribute.channel) {
            return Err(io::Error::from(io::ErrorKind::ResourceBusy));
        }
        let mut written = self.sequence(attribute.channel, attribute.sequence)?;
        written[attribute.index] = stored;
        let beside = self.sequence(attribute.channel, other(attribute.sequence))?;
        if falls(&written) || falls(&beside) {
            return Err(invalid());
        }
        self.writes.borrow_mut().push(Write {
            name: name.to_owned(),
            value: value.to_string(),
        });
        if !self.stuck.contains(name) {
            self.set(name, &stored.to_string());
        }
        Ok(())
    }
}
