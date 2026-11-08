//! The write cycle against a model of the driver, which refuses each write
//! the driver refuses.

use fanctl::apply::{ApplyError, Mismatch, apply, show};
use fanctl::board::Board;
use fanctl::config::{self, Config};
use fanctl::curve::StoredPoint;
use fanctl::hwmon::Attributes;

use crate::common::driver::Driver;
use crate::common::{testdata, testdata_text};

fn workstation() -> Config {
    config::load(&testdata("configs/workstation.yaml"))
        .unwrap_or_else(|error| panic!("load the workstation config: {error}"))
}

#[test]
fn the_model_refuses_a_write_that_leaves_the_curve_falling() {
    let driver = Driver::ms_7e70();
    let error = driver.write("pwm1_auto_point1_temp", "55000").unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
    assert_eq!(
        driver.get("pwm1_auto_point1_temp").as_deref(),
        Some("20000")
    );
    assert!(driver.write("pwm1_auto_point1_pwm", "256").is_err());
    assert!(driver.write("pwm1_auto_point7_temp", "120000").is_ok());
}

#[test]
fn the_workstation_config_applies_through_the_driver() {
    let driver = Driver::ms_7e70();
    let fans = apply(&driver, &workstation()).unwrap();
    let headers: Vec<&str> = fans.iter().map(|fan| fan.header).collect();
    assert_eq!(headers, ["CPU_FAN1", "PUMP_SYS1", "SYS_FAN1", "SYS_FAN2"]);
    let writes = driver.writes.borrow();
    assert_eq!(writes.len(), 4 * 14, "every point is written");
    let first: Vec<&str> = writes
        .iter()
        .take(7)
        .map(|write| write.name.as_str())
        .collect();
    assert_eq!(
        first,
        [
            "pwm1_auto_point5_temp",
            "pwm1_auto_point4_temp",
            "pwm1_auto_point3_temp",
            "pwm1_auto_point2_temp",
            "pwm1_auto_point1_temp",
            "pwm1_auto_point6_temp",
            "pwm1_auto_point7_temp",
        ]
    );
    assert_eq!(
        writes.first().map(|write| write.value.as_str()),
        Some("72000"),
        "a temperature is written in millidegrees"
    );
    assert_eq!(driver.get("pwm1_auto_point2_pwm").as_deref(), Some("126"));
    assert_eq!(driver.get("pwm3_auto_point3_pwm").as_deref(), Some("186"));
    assert_eq!(
        driver.get("pwm5_auto_point1_temp").as_deref(),
        Some("20000")
    );
}

#[test]
fn show_reads_back_what_apply_wrote() {
    let driver = Driver::ms_7e70();
    assert_eq!(
        show(&driver, Board::Ms7e70).unwrap().to_yaml(),
        testdata_text("show/firmware.yaml")
    );
    apply(&driver, &workstation()).unwrap();
    let shown = show(&driver, Board::Ms7e70).unwrap();
    assert_eq!(shown.to_yaml(), testdata_text("show/after-apply.yaml"));
    let writes = driver.writes.borrow().len();
    apply(&driver, &shown).unwrap();
    assert_eq!(driver.writes.borrow().len(), writes + 8 * 14);
    assert_eq!(show(&driver, Board::Ms7e70).unwrap(), shown);
}

#[test]
fn a_channel_under_manual_control_stops_the_apply_at_its_first_write() {
    let mut driver = Driver::ms_7e70();
    driver.manual.insert(1);
    let error = apply(&driver, &workstation()).unwrap_err();
    let ApplyError::Write {
        header, attribute, ..
    } = &error
    else {
        panic!("a write error: {error}");
    };
    assert_eq!(*header, "CPU_FAN1");
    assert_eq!(attribute, "pwm1_auto_point5_temp");
    assert!(error.to_string().contains("manual control"), "{error}");
    assert!(driver.writes.borrow().is_empty());
}

#[test]
fn a_point_that_does_not_hold_is_named_with_both_values() {
    let mut driver = Driver::ms_7e70();
    driver.stuck.insert("pwm3_auto_point1_pwm".to_owned());
    let error = apply(&driver, &workstation()).unwrap_err();
    let ApplyError::Mismatch(mismatches) = &error else {
        panic!("a mismatch: {error}");
    };
    assert_eq!(
        mismatches,
        &[Mismatch {
            header: "SYS_FAN1",
            channel: Board::Ms7e70.header("SYS_FAN1").unwrap().channel,
            point: 1,
            wrote: StoredPoint { temp: 55, pwm: 153 },
            read: StoredPoint { temp: 55, pwm: 127 },
        }]
    );
    assert!(
        error
            .to_string()
            .contains("SYS_FAN1 (pwm3) point 1: wrote 55 C, duty 153; reads 55 C, duty 127"),
        "{error}"
    );
}

#[test]
fn a_falling_stored_curve_stops_the_apply_before_any_write() {
    let driver = Driver::ms_7e70();
    driver.set("pwm4_auto_point3_pwm", "100");
    let error = apply(&driver, &workstation()).unwrap_err();
    assert!(
        matches!(
            error,
            ApplyError::CurrentFalls {
                header: "SYS_FAN2",
                ..
            }
        ),
        "{error}"
    );
    assert!(error.to_string().contains("nothing was written"), "{error}");
    assert!(driver.writes.borrow().is_empty());
}

#[test]
fn a_header_without_attributes_stops_the_apply_before_any_write() {
    let driver = Driver::ms_7e70();
    driver.remove("pwm2_");
    let error = apply(&driver, &workstation()).unwrap_err();
    assert!(matches!(error, ApplyError::Read(_)), "{error}");
    assert!(
        error
            .to_string()
            .contains("PUMP_SYS1 (pwm2): pwm2_auto_point1_temp does not exist"),
        "{error}"
    );
    assert!(driver.writes.borrow().is_empty());
    let shown = show(&driver, Board::Ms7e70).unwrap();
    assert!(!shown.fans.contains_key("PUMP_SYS1"));
}
