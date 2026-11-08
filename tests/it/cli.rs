//! The binary against copies of the fixture sysfs trees under `testdata/`.

use std::process::Output;

use ctl_core::fixture::Suite;

use crate::common::{fanctl, sysfs_copy, testdata, testdata_text};

fn run(args: &[&str], sysfs: &std::path::Path) -> Output {
    fanctl()
        .args(args)
        .arg("--sysfs")
        .arg(sysfs)
        .output()
        .unwrap_or_else(|error| panic!("start fanctl: {error}"))
}

fn text(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

#[test]
fn check_needs_no_hardware() {
    let config = testdata("configs/workstation.yaml");
    let output = fanctl()
        .args(["check", "--format", "json", "--sysfs", "/nonexistent"])
        .arg(&config)
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["type"], "checked", "{json}");
    assert_eq!(json["board"], "MS-7E70", "{json}");
    assert_eq!(json["fans"][0]["header"], "CPU_FAN1", "{json}");
    assert_eq!(json["fans"][0]["channel"], 1, "{json}");
    assert_eq!(json["fans"][0]["curve"][1]["duty"], 49.6, "{json}");
}

#[test]
fn show_prints_the_driver_curves_as_a_config() {
    let suite = Suite::new("cli");
    let sysfs = sysfs_copy(&suite, "ms-7e70");
    let output = run(&["show"], sysfs.path());
    assert!(output.status.success(), "{output:?}");
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim_end(),
        testdata_text("show/firmware.yaml").trim_end()
    );
}

#[test]
fn apply_writes_every_point_and_show_reads_them_back() {
    let suite = Suite::new("cli");
    let sysfs = sysfs_copy(&suite, "ms-7e70");
    let config = testdata("configs/workstation.yaml");
    let output = run(&["apply", &config.to_string_lossy()], sysfs.path());
    assert!(output.status.success(), "{}", text(&output));
    assert_eq!(sysfs.read("class/hwmon/hwmon3/pwm1_auto_point2_pwm"), "126");
    assert_eq!(
        sysfs.read("class/hwmon/hwmon3/pwm4_auto_point1_temp"),
        "55000"
    );
    let shown = run(&["show"], sysfs.path());
    assert!(shown.status.success(), "{shown:?}");
    let shown = String::from_utf8_lossy(&shown.stdout).into_owned();
    assert_eq!(
        shown.trim_end(),
        testdata_text("show/after-apply.yaml").trim_end()
    );
    let pasted = sysfs.file("pasted.yaml", &shown);
    let output = run(&["apply", &pasted.to_string_lossy()], sysfs.path());
    assert!(output.status.success(), "{}", text(&output));
}

#[test]
fn apply_refuses_a_config_for_another_board() {
    let suite = Suite::new("cli");
    let sysfs = sysfs_copy(&suite, "other-board");
    let config = testdata("configs/workstation.yaml");
    let output = run(&["apply", &config.to_string_lossy()], sysfs.path());
    assert!(!output.status.success());
    assert!(
        text(&output).contains("which fanctl has no header table for"),
        "{}",
        text(&output)
    );
}

#[test]
fn the_device_is_found_by_name_and_only_once() {
    let suite = Suite::new("cli");
    let missing = sysfs_copy(&suite, "no-device");
    let output = run(&["show"], missing.path());
    assert!(!output.status.success());
    assert!(
        text(&output).contains("no hwmon device is named nct6687"),
        "{}",
        text(&output)
    );
    let two = sysfs_copy(&suite, "two-devices");
    let output = run(&["show"], two.path());
    assert!(!output.status.success());
    assert!(
        text(&output).contains("more than one hwmon device is named nct6687"),
        "{}",
        text(&output)
    );
}

#[test]
fn json_output_carries_no_colour_codes_even_when_colour_is_forced() {
    let output = fanctl()
        .args(["instructions", "--format", "json"])
        .env("CLICOLOR_FORCE", "1")
        .env("FORCE_COLOR", "3")
        .env("COLORTERM", "truecolor")
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    assert!(!output.stdout.contains(&0x1b), "ESC in JSON output");
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(value.to_string().contains("fanctl agent instructions"));
}
