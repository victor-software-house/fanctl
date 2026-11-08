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

use fanctl::hwmon::Attributes;

const POINTS: u8 = 7;

/// The model's state: every attribute file's text, by name.
#[derive(Debug, Default)]
pub(crate) struct Driver {
    files: RefCell<BTreeMap<String, String>>,
    /// Channels under manual control.
    pub(crate) manual: BTreeSet<u8>,
    /// Attributes that take a write and keep their value.
    pub(crate) stuck: BTreeSet<String>,
    /// Every write the model accepted, in order.
    pub(crate) writes: RefCell<Vec<(String, String)>>,
}

/// One curve attribute: channel, point from 1, and whether it is a
/// temperature.
fn parse_name(name: &str) -> Option<(u8, u8, bool)> {
    let rest = name.strip_prefix("pwm")?;
    let (channel, rest) = rest.split_once("_auto_point")?;
    let (point, kind) = rest.split_once('_')?;
    let is_temp = match kind {
        "temp" => true,
        "pwm" => false,
        _ => return None,
    };
    Some((channel.parse().ok()?, point.parse().ok()?, is_temp))
}

fn invalid() -> io::Error {
    io::Error::from(io::ErrorKind::InvalidInput)
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
    fn sequence(&self, channel: u8, is_temp: bool) -> io::Result<Vec<i64>> {
        let kind = if is_temp { "temp" } else { "pwm" };
        (1..=POINTS)
            .map(|point| {
                self.read(&format!("pwm{channel}_auto_point{point}_{kind}"))?
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
        let (channel, point, is_temp) =
            parse_name(name).ok_or_else(|| io::Error::from(io::ErrorKind::PermissionDenied))?;
        let value = value.trim().parse::<i64>().ok().ok_or_else(invalid)?;
        let stored = if is_temp {
            (value.clamp(0, 127_000) + 500) / 1000 * 1000
        } else if (0..=255).contains(&value) {
            value
        } else {
            return Err(invalid());
        };
        if self.manual.contains(&channel) {
            return Err(io::Error::from(io::ErrorKind::ResourceBusy));
        }
        let mut sequence = self.sequence(channel, is_temp)?;
        let index = usize::from(point - 1);
        sequence[index] = stored;
        let other = self.sequence(channel, !is_temp)?;
        if sequence.windows(2).any(|pair| pair[1] < pair[0])
            || other.windows(2).any(|pair| pair[1] < pair[0])
        {
            return Err(invalid());
        }
        self.writes
            .borrow_mut()
            .push((name.to_owned(), value.to_string()));
        if !self.stuck.contains(name) {
            self.set(name, &stored.to_string());
        }
        Ok(())
    }
}
