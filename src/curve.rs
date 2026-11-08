//! A fan curve: 7 points of temperature and duty, as the config states it and
//! as the driver stores it, and the order that writes one point at a time.

use std::fmt;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Points per curve; the chip holds exactly 7.
pub const POINTS: usize = 7;

/// The highest temperature a point may hold, in degrees C. The driver clamps
/// above it: the chip's byte above 127 has no known meaning.
pub const TEMP_MAX: u8 = 127;

/// A point's temperature, in whole degrees C, 0 to [`TEMP_MAX`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "u8", into = "u8")]
pub struct Temp(u8);

impl Temp {
    /// The temperature in degrees C.
    #[must_use]
    pub const fn degrees(self) -> u8 {
        self.0
    }
}

impl TryFrom<u8> for Temp {
    type Error = RangeError;

    fn try_from(degrees: u8) -> Result<Self, RangeError> {
        if degrees > TEMP_MAX {
            return Err(RangeError::Temp(degrees));
        }
        Ok(Self(degrees))
    }
}

impl From<Temp> for u8 {
    fn from(temp: Temp) -> Self {
        temp.0
    }
}

/// A point's duty, in tenths of a percent, 0 to 1000.
///
/// The config states duty in percent with at most one decimal, which maps to
/// the driver's 0-255 by rounding to the nearest step. One decimal is enough
/// to name every step, so a duty read back prints as a percent that maps to
/// the same step.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "f64", into = "f64")]
pub struct Duty(u16);

impl Duty {
    /// The full duty, 100 %.
    pub const MAX: Self = Self(1000);

    /// The driver's 0-255 step nearest to this duty.
    #[must_use]
    pub fn pwm(self) -> u8 {
        let step = (u32::from(self.0) * 255 + 500) / 1000;
        u8::try_from(step).unwrap_or(u8::MAX)
    }

    /// The duty, to one decimal, that maps to this 0-255 step.
    #[must_use]
    pub fn from_pwm(pwm: u8) -> Self {
        let tenths = (u32::from(pwm) * 1000 + 127) / 255;
        Self(u16::try_from(tenths).unwrap_or(Self::MAX.0))
    }

    /// The duty in tenths of a percent.
    #[must_use]
    pub const fn tenths(self) -> u16 {
        self.0
    }
}

impl TryFrom<f64> for Duty {
    type Error = RangeError;

    fn try_from(percent: f64) -> Result<Self, RangeError> {
        let tenths = (percent * 10.0).round();
        if !(0.0..=1000.0).contains(&tenths) {
            return Err(RangeError::Duty(percent));
        }
        if (percent * 10.0 - tenths).abs() > 1e-6 {
            return Err(RangeError::DutyPrecision(percent));
        }
        Ok(Self(whole_tenths(tenths)))
    }
}

/// A float already rounded and checked to lie in 0..=1000, as an integer.
#[expect(
    clippy::as_conversions,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "f64 has no TryFrom to an integer; the caller rounds and range-checks first"
)]
fn whole_tenths(tenths: f64) -> u16 {
    tenths as u16
}

impl From<Duty> for f64 {
    fn from(duty: Duty) -> Self {
        Self::from(duty.0) / 10.0
    }
}

impl fmt::Display for Duty {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (whole, tenth) = (self.0 / 10, self.0 % 10);
        if tenth == 0 {
            write!(f, "{whole}")
        } else {
            write!(f, "{whole}.{tenth}")
        }
    }
}

/// A value outside what a point can hold.
#[derive(Clone, Copy, Debug, PartialEq, Error)]
pub enum RangeError {
    /// A temperature above [`TEMP_MAX`].
    #[error("temperature {0} C is above {TEMP_MAX} C")]
    Temp(u8),
    /// A duty outside 0-100 %.
    #[error("duty {0} % is outside 0 to 100 %")]
    Duty(f64),
    /// A duty with more than one decimal.
    #[error("duty {0} % has more than one decimal")]
    DutyPrecision(f64),
}

/// One point of a curve.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Point {
    /// Degrees C.
    pub temp: Temp,
    /// Percent.
    pub duty: Duty,
}

/// A fan curve as the config states it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Curve(pub [Point; POINTS]);

impl Curve {
    /// Where the curve falls, or `None` when neither its temperatures nor its
    /// duties fall from one point to the next. Equal neighbours are allowed.
    #[must_use]
    pub fn fall(&self) -> Option<Fall> {
        let neighbours = self.0.windows(2).zip(2..);
        for (pair, point) in neighbours {
            let [before, after] = pair else {
                continue;
            };
            if after.temp < before.temp {
                return Some(Fall {
                    point,
                    sequence: Sequence::Temp,
                });
            }
            if after.duty < before.duty {
                return Some(Fall {
                    point,
                    sequence: Sequence::Pwm,
                });
            }
        }
        None
    }

    /// The values the driver stores for this curve.
    #[must_use]
    pub fn registers(&self) -> Registers {
        Registers {
            temp: self.0.map(|point| point.temp.degrees()),
            pwm: self.0.map(|point| point.duty.pwm()),
        }
    }
}

/// The first point lower than the one before it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fall {
    /// The point, from 1.
    pub point: usize,
    /// Which of its values falls.
    pub sequence: Sequence,
}

impl fmt::Display for Fall {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let what = match self.sequence {
            Sequence::Temp => "temperature",
            Sequence::Pwm => "duty",
        };
        write!(
            f,
            "point {} has a lower {what} than point {}",
            self.point,
            self.point - 1
        )
    }
}

/// One of a curve's two sequences of 7 values.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Sequence {
    /// The temperatures, `pwmN_auto_pointM_temp`.
    Temp,
    /// The duties, `pwmN_auto_pointM_pwm`.
    Pwm,
}

impl Sequence {
    /// Both sequences, in the order fanctl writes them.
    pub const ALL: [Self; 2] = [Self::Temp, Self::Pwm];

    /// The attribute suffix, `temp` or `pwm`.
    #[must_use]
    pub const fn suffix(self) -> &'static str {
        match self {
            Self::Temp => "temp",
            Self::Pwm => "pwm",
        }
    }
}

/// A curve as the driver stores it: degrees C and 0-255 steps.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Registers {
    /// Each point's temperature, in degrees C.
    pub temp: [u8; POINTS],
    /// Each point's duty, 0-255.
    pub pwm: [u8; POINTS],
}

impl Registers {
    /// One sequence's 7 values.
    #[must_use]
    pub const fn sequence(&self, sequence: Sequence) -> [u8; POINTS] {
        match sequence {
            Sequence::Temp => self.temp,
            Sequence::Pwm => self.pwm,
        }
    }

    /// One point's stored values, with `index` from 0, or `None` past the
    /// last point.
    #[must_use]
    pub fn point(&self, index: usize) -> Option<StoredPoint> {
        Some(StoredPoint {
            temp: *self.temp.get(index)?,
            pwm: *self.pwm.get(index)?,
        })
    }

    /// Where the stored curve falls, as [`Curve::fall`].
    #[must_use]
    pub fn fall(&self) -> Option<Fall> {
        Sequence::ALL.into_iter().find_map(|sequence| {
            falls_at(self.sequence(sequence)).map(|point| Fall { point, sequence })
        })
    }

    /// The curve in the config's terms.
    ///
    /// # Errors
    ///
    /// [`RangeError::Temp`] when a point holds a temperature above
    /// [`TEMP_MAX`].
    pub fn curve(&self) -> Result<Curve, RangeError> {
        let mut points = [Point {
            temp: Temp(0),
            duty: Duty(0),
        }; POINTS];
        let stored = self.temp.into_iter().zip(self.pwm);
        let slots = points.iter_mut().zip(stored);
        for (point, (temp, pwm)) in slots {
            *point = Point {
                temp: Temp::try_from(temp)?,
                duty: Duty::from_pwm(pwm),
            };
        }
        Ok(Curve(points))
    }
}

/// One point as the driver stores it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct StoredPoint {
    /// Degrees C.
    pub temp: u8,
    /// Duty, 0-255.
    pub pwm: u8,
}

impl fmt::Display for StoredPoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} C, duty {}", self.temp, self.pwm)
    }
}

/// The first point, from 1, whose value is lower than the one before it.
fn falls_at(values: [u8; POINTS]) -> Option<usize> {
    values
        .windows(2)
        .zip(2..)
        .find_map(|(pair, point)| (pair.get(1) < pair.first()).then_some(point))
}

/// The order, as indices from 0, in which to write `target` over `current`
/// one value at a time so that every intermediate sequence stays
/// non-decreasing. Both sequences must be non-decreasing.
///
/// The driver checks each write alone, against the curve as it stands, and
/// refuses one that leaves the curve falling. So the values that rise go
/// first, from the last point down: each lands at or below its right
/// neighbour, which has either risen already to a target no lower, or stays
/// at a current value above this one's target; and above its left
/// neighbour, which still holds a current value below this one's. The rest
/// follow from the first point up: each lands at or above its left
/// neighbour, already at its target, and at or below its right neighbour,
/// which holds its target or a current value above this one's.
///
/// Every index appears once, including those already at their target: the
/// driver restores after resume only the curves written since it loaded, so
/// fanctl writes each point even when it holds the value.
#[must_use]
pub fn write_order(current: [u8; POINTS], target: [u8; POINTS]) -> [usize; POINTS] {
    let rising = (0..POINTS)
        .rev()
        .filter(|&index| target.get(index) > current.get(index));
    let rest = (0..POINTS).filter(|&index| target.get(index) <= current.get(index));
    let indices = rising.chain(rest);
    let mut order = [0; POINTS];
    let slots = order.iter_mut().zip(indices);
    for (slot, index) in slots {
        *slot = index;
    }
    order
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(temp: u8, tenths: u16) -> Point {
        Point {
            temp: Temp(temp),
            duty: Duty(tenths),
        }
    }

    /// Every non-decreasing sequence of 7 values from `0..=top`.
    fn non_decreasing(top: u8) -> Vec<[u8; POINTS]> {
        let mut all = vec![];
        let mut values = [0; POINTS];
        loop {
            all.push(values);
            let Some(last) = (0..POINTS).rev().find(|&index| values[index] < top) else {
                return all;
            };
            let next = values[last] + 1;
            for value in &mut values[last..] {
                *value = next;
            }
        }
    }

    #[test]
    fn every_write_order_keeps_every_intermediate_sequence_non_decreasing() {
        let sequences = non_decreasing(4);
        assert_eq!(sequences.len(), 330);
        for current in &sequences {
            for target in &sequences {
                let order = write_order(*current, *target);
                let mut sorted = order;
                sorted.sort_unstable();
                assert_eq!(sorted, [0, 1, 2, 3, 4, 5, 6], "{current:?} -> {target:?}");
                let mut state = *current;
                for index in order {
                    state[index] = target[index];
                    assert_eq!(
                        falls_at(state),
                        None,
                        "{current:?} -> {target:?}: {state:?} after writing point {}",
                        index + 1
                    );
                }
                assert_eq!(state, *target);
            }
        }
    }

    #[test]
    fn rising_values_go_first_from_the_last_point_down() {
        let current = [20, 40, 50, 60, 70, 85, 100];
        let target = [55, 59, 63, 68, 72, 76, 80];
        assert_eq!(write_order(current, target), [4, 3, 2, 1, 0, 5, 6]);
    }

    #[test]
    fn every_duty_step_round_trips_through_its_printed_percent() {
        for pwm in 0..=u8::MAX {
            let duty = Duty::from_pwm(pwm);
            let printed: f64 = duty.to_string().parse().unwrap();
            assert_eq!(Duty::try_from(printed).unwrap().pwm(), pwm, "{duty}");
        }
    }

    #[test]
    fn the_deployed_duties_map_to_the_deployed_steps() {
        let cpu = [40.0, 49.6, 59.2, 71.2, 80.8, 90.4, 100.0];
        let system = [60.0, 66.4, 72.8, 80.8, 87.2, 93.6, 100.0];
        let pwm = |percents: [f64; POINTS]| {
            percents.map(|percent| Duty::try_from(percent).unwrap().pwm())
        };
        assert_eq!(pwm(cpu), [102, 126, 151, 182, 206, 231, 255]);
        assert_eq!(pwm(system), [153, 169, 186, 206, 222, 239, 255]);
    }

    #[test]
    fn out_of_range_values_are_refused() {
        assert_eq!(Temp::try_from(128), Err(RangeError::Temp(128)));
        assert!(Temp::try_from(127).is_ok());
        assert!(matches!(Duty::try_from(100.1), Err(RangeError::Duty(_))));
        assert!(matches!(Duty::try_from(-0.1), Err(RangeError::Duty(_))));
        assert!(matches!(Duty::try_from(f64::NAN), Err(RangeError::Duty(_))));
        assert!(matches!(
            Duty::try_from(49.65),
            Err(RangeError::DutyPrecision(_))
        ));
    }

    #[test]
    fn a_falling_point_is_named() {
        let mut points = [point(55, 400); POINTS];
        assert_eq!(Curve(points).fall(), None);
        points[3] = point(54, 400);
        assert_eq!(
            Curve(points).fall(),
            Some(Fall {
                point: 4,
                sequence: Sequence::Temp
            })
        );
        points[3] = point(55, 390);
        assert_eq!(
            Curve(points).fall().map(|fall| fall.to_string()),
            Some("point 4 has a lower duty than point 3".to_owned())
        );
    }

    #[test]
    fn a_stored_falling_curve_is_named() {
        let registers = Registers {
            temp: [10, 20, 30, 40, 50, 60, 70],
            pwm: [10, 20, 30, 25, 50, 60, 70],
        };
        assert_eq!(
            registers.fall(),
            Some(Fall {
                point: 4,
                sequence: Sequence::Pwm
            })
        );
    }
}
