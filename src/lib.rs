//! fanctl writes declared fan curves into the Nuvoton NCT6687D-R through the
//! `nct6687` hwmon driver, and reads them back.
//!
//! A config ([`config`]) names a [`board`] and gives a 7-point curve
//! ([`curve`]) for each fan header to set. [`apply`] reads every configured
//! curve from the driver's sysfs attributes ([`hwmon`]), writes every point in
//! an order the driver accepts, and reads every point back. The chip then
//! runs the curves on its own; no process stays behind.
//!
//! The binary is the product; this library exists so its parts are tested
//! and documented one by one.

pub mod apply;
pub mod board;
pub mod cli;
pub mod config;
pub mod curve;
pub mod hwmon;
pub mod outcome;
pub mod run;
