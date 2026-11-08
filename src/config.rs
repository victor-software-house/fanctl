//! The config file: the board, and a curve for each fan header to set.
//!
//! ```yaml
//! board: MS-7E70
//! fans:
//!   CPU_FAN1:
//!     - { temp: 55, duty: 40 }
//!     - { temp: 59, duty: 49.6 }
//!     - { temp: 63, duty: 59.2 }
//!     - { temp: 68, duty: 71.2 }
//!     - { temp: 72, duty: 80.8 }
//!     - { temp: 76, duty: 90.4 }
//!     - { temp: 80, duty: 100 }
//! ```
//!
//! Each fan takes exactly 7 points. `temp` is whole degrees C, 0 to 127;
//! `duty` is percent, 0 to 100, with at most one decimal. Neither may fall
//! from one point to the next. A header the file does not name keeps the
//! curve it has.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

use ctl_core::input::{Input, InputError};
use garde::Validate;
use serde::{Deserialize, Serialize};

use crate::board::Board;
use crate::curve::Curve;

/// The whole config file.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Validate)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// The board whose header table names the fans.
    #[garde(skip)]
    pub board: Board,
    /// Each fan's curve, by header name.
    #[garde(custom(fans_fit(self.board)))]
    pub fans: BTreeMap<String, Curve>,
}

fn fans_fit(board: Board) -> impl FnOnce(&BTreeMap<String, Curve>, &()) -> garde::Result {
    move |fans, ()| {
        if fans.is_empty() {
            return Err(garde::Error::new("name at least one fan"));
        }
        let mut problems = vec![];
        for (name, curve) in fans {
            if board.header(name).is_none() {
                let known: Vec<&str> = board.headers().iter().map(|header| header.name).collect();
                problems.push(format!(
                    "`{name}` is not a fan header on {board}; it has {}",
                    known.join(", ")
                ));
            } else if let Some(fall) = curve.fall() {
                problems.push(format!("`{name}`: {fall}"));
            }
        }
        if problems.is_empty() {
            Ok(())
        } else {
            Err(garde::Error::new(problems.join("; ")))
        }
    }
}

impl Config {
    /// The file as `fanctl show` prints it: one line per point, so it can be
    /// pasted back as a config.
    #[must_use]
    pub fn to_yaml(&self) -> String {
        let mut text = String::new();
        let _ = writeln!(text, "board: {}", self.board);
        let _ = writeln!(text, "fans:");
        for (name, curve) in &self.fans {
            let _ = writeln!(text, "  {name}:");
            for point in curve.0 {
                let _ = writeln!(
                    text,
                    "    - {{ temp: {}, duty: {} }}",
                    point.temp.degrees(),
                    point.duty
                );
            }
        }
        text
    }
}

/// Read and validate one config file, YAML whatever its extension.
///
/// # Errors
///
/// [`InputError`] when the file cannot be read, is not valid YAML, has an
/// unknown field or board, a fan without exactly 7 points, a value out of
/// range, a header the board does not have, or a curve that falls.
pub fn load(path: &Path) -> Result<Config, InputError> {
    Input::read(path)?.load::<Config>()
}

#[cfg(test)]
mod tests {
    use ctl_core::indoc;

    use super::*;

    fn parse(text: &str) -> Result<Config, InputError> {
        Input::new("fans.yaml", text).load::<Config>()
    }

    const EXAMPLE: &str = indoc! {"
        board: MS-7E70
        fans:
          CPU_FAN1:
            - { temp: 55, duty: 40 }
            - { temp: 59, duty: 49.6 }
            - { temp: 63, duty: 59.2 }
            - { temp: 68, duty: 71.2 }
            - { temp: 72, duty: 80.8 }
            - { temp: 76, duty: 90.4 }
            - { temp: 80, duty: 100 }
    "};

    #[test]
    fn the_documented_example_loads_and_prints_back_as_itself() {
        let config = parse(EXAMPLE).unwrap();
        assert_eq!(config.board, Board::Ms7e70);
        assert_eq!(
            config.fans["CPU_FAN1"].registers().pwm,
            [102, 126, 151, 182, 206, 231, 255]
        );
        assert_eq!(config.to_yaml(), EXAMPLE);
        assert_eq!(parse(&config.to_yaml()).unwrap(), config);
    }

    #[test]
    fn an_unknown_header_is_refused() {
        let text = EXAMPLE.replace("CPU_FAN1", "EZ_CONN");
        let error = parse(&text).unwrap_err().to_string();
        assert!(
            error.contains("`EZ_CONN` is not a fan header on MS-7E70"),
            "{error}"
        );
    }

    #[test]
    fn a_falling_curve_is_refused() {
        let text = EXAMPLE.replace("temp: 63", "temp: 58");
        let error = parse(&text).unwrap_err().to_string();
        assert!(
            error.contains("`CPU_FAN1`: point 3 has a lower temperature than point 2"),
            "{error}"
        );
    }

    #[test]
    fn a_curve_without_seven_points_is_refused() {
        let text = EXAMPLE.replace("    - { temp: 80, duty: 100 }\n", "");
        assert!(parse(&text).is_err());
    }

    #[test]
    fn out_of_range_values_are_refused() {
        for (from, to) in [
            ("temp: 80", "temp: 128"),
            ("duty: 100 ", "duty: 100.5 "),
            ("duty: 40 ", "duty: 40.25 "),
        ] {
            let text = EXAMPLE.replace(from, to);
            assert!(parse(&text).is_err(), "{to}");
        }
    }

    #[test]
    fn an_unknown_board_or_field_is_refused() {
        assert!(parse(&EXAMPLE.replace("MS-7E70", "MS-7E51")).is_err());
        assert!(parse(&format!("{EXAMPLE}extra: 1\n")).is_err());
        assert!(parse("board: MS-7E70\nfans: {}\n").is_err());
    }
}
