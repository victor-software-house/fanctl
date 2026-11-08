//! Boards fanctl has a header table for, and which fan header drives which pwm
//! channel of the `nct6687` driver on each.

use std::fmt;

use serde::{Deserialize, Serialize};

/// A board with a header table.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Board {
    /// MSI PRO X870E-P WIFI.
    #[serde(rename = "MS-7E70")]
    Ms7e70,
}

/// One fan header and the pwm channel that drives it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Header {
    /// The name printed on the board, such as `CPU_FAN1`.
    pub name: &'static str,
    /// The driver's channel: `pwmN` and its `pwmN_auto_point*` files.
    pub channel: Channel,
}

/// A pwm channel number of the `nct6687` driver, from 1.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct Channel(u8);

impl Channel {
    /// The channel's number, the `N` in `pwmN`.
    #[must_use]
    pub const fn number(self) -> u8 {
        self.0
    }
}

impl fmt::Display for Channel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "pwm{}", self.0)
    }
}

/// The MS-7E70's headers with a curve, in channel order.
///
/// Source: the channel table of Fred78290/nct6687d#229, taken from the fan
/// tables of MSI BIOS 2A52 for this board. EZ-Connect (EC channel 3) is left
/// out: the driver has no pwm attribute for it.
const MS_7E70: [Header; 8] = [
    Header {
        name: "CPU_FAN1",
        channel: Channel(1),
    },
    Header {
        name: "PUMP_SYS1",
        channel: Channel(2),
    },
    Header {
        name: "SYS_FAN1",
        channel: Channel(3),
    },
    Header {
        name: "SYS_FAN2",
        channel: Channel(4),
    },
    Header {
        name: "SYS_FAN3",
        channel: Channel(5),
    },
    Header {
        name: "SYS_FAN4",
        channel: Channel(6),
    },
    Header {
        name: "SYS_FAN5",
        channel: Channel(7),
    },
    Header {
        name: "SYS_FAN6",
        channel: Channel(8),
    },
];

impl Board {
    /// Every board fanctl has a header table for.
    pub const ALL: [Self; 1] = [Self::Ms7e70];

    /// The model the config file names, such as `MS-7E70`.
    #[must_use]
    pub const fn model(self) -> &'static str {
        match self {
            Self::Ms7e70 => "MS-7E70",
        }
    }

    /// The board's DMI `board_name`, as `/sys/class/dmi/id/board_name` reads.
    #[must_use]
    pub const fn dmi_board_name(self) -> &'static str {
        match self {
            Self::Ms7e70 => "PRO X870E-P WIFI (MS-7E70)",
        }
    }

    /// The board's headers with a curve, in channel order.
    #[must_use]
    pub const fn headers(self) -> &'static [Header] {
        match self {
            Self::Ms7e70 => &MS_7E70,
        }
    }

    /// The header with this name, or `None` when the board has none.
    #[must_use]
    pub fn header(self, name: &str) -> Option<Header> {
        self.headers()
            .iter()
            .find(|header| header.name == name)
            .copied()
    }

    /// The board whose DMI `board_name` this is.
    #[must_use]
    pub fn from_dmi(board_name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|board| board.dmi_board_name() == board_name)
    }
}

impl fmt::Display for Board {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.model())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ms_7e70_table_matches_the_driver_pr() {
        let table: Vec<(&str, u8)> = Board::Ms7e70
            .headers()
            .iter()
            .map(|header| (header.name, header.channel.number()))
            .collect();
        assert_eq!(
            table,
            [
                ("CPU_FAN1", 1),
                ("PUMP_SYS1", 2),
                ("SYS_FAN1", 3),
                ("SYS_FAN2", 4),
                ("SYS_FAN3", 5),
                ("SYS_FAN4", 6),
                ("SYS_FAN5", 7),
                ("SYS_FAN6", 8),
            ]
        );
    }

    #[test]
    fn an_unknown_header_has_no_channel() {
        assert_eq!(Board::Ms7e70.header("EZ_CONN"), None);
        assert_eq!(Board::Ms7e70.header("cpu_fan1"), None);
    }

    #[test]
    fn the_board_is_found_by_its_dmi_name_only() {
        assert_eq!(
            Board::from_dmi("PRO X870E-P WIFI (MS-7E70)"),
            Some(Board::Ms7e70)
        );
        assert_eq!(Board::from_dmi("MAG X870 TOMAHAWK WIFI (MS-7E51)"), None);
    }
}
