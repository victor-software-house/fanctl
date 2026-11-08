# fanctl

Linux-only ctl CLI that writes declared fan curves into the NCT6687D-R through the `nct6687` hwmon driver, built on ctl-core.
This file is the development contract.
The operator contract will be `fanctl instructions` and `skills/fanctl/SKILL.md`.

## Decisions

1. **Device lookup.**
   The driver's device is the one directory under `/sys/class/hwmon/` whose `name` reads `nct6687`.
   Never hard-code a `hwmonN` number; it changes between boots.
   No such device, or more than one, is an error.
2. **The driver is the only writer.**
   fanctl reads and writes the driver's sysfs attributes and nothing else: no I/O ports, no EC access, no module parameters.
3. **Headers, not channels.**
   A config names fans by board header (`CPU_FAN1`, `PUMP_SYS1`, `SYS_FAN1` …).
   A table per board maps each header to its pwm channel; a header the table does not list is an error.
4. **Validate before writing.**
   A config whose temperatures or duties fall from one point to the next, or that is out of range, is refused before anything is written.
5. **Write order follows the driver.**
   The driver checks each point write alone, against the curve as it stands, and refuses one that leaves the curve falling.
   fanctl orders the writes so every intermediate curve stays non-decreasing.
   The rule, with its proof, is on `curve::write_order`; a test runs it over every pair of non-decreasing sequences of 7 values from 0 to 4.
   Every point is written, even one that holds its value, because the driver writes back after resume only the curves written since it loaded.
6. **Read back.**
   After writing, fanctl reads every point and exits non-zero on any mismatch, naming the fan and the point.
7. **Duty in percent, one decimal.**
   The config states duty in percent with at most one decimal; it maps to the driver's 0-255 by rounding to the nearest step.
   One decimal names every step, so `show` prints a percent that maps back to the same step, which may differ from the percent the config gave (49.6 and 49.4 are both step 126).
8. **No daemon, no resume hook.**
   The chip runs the curves itself.
   The driver (`nct6687_resume`, `nct6687_restore_curves`) writes back after S3 resume the curves written through its attributes since it loaded, so a boot-time `fanctl apply` covers resume too.
9. **Releases are GitHub Release binaries only**, a static musl build for `x86_64`.
   The crate is not published to crates.io (`publish = false`).
   `.github/workflows/release.yml` calls verctl's reusable release workflow, and `.ctl/ver.yaml` declares the package with `publisher: none`, so a release is a tag, a GitHub Release, and one tarball.
   Each change that ships behaviour adds a `.changeset/` fragment; merging the Version PR releases.
   No arm64 build ships: the boards with a table are x86_64 desktop boards.

## Layout

- `src/board.rs` holds each board's header table, with its source; `src/curve.rs` the point types, the duty mapping and the write order; `src/config.rs` the config file.
- `src/hwmon.rs` finds the device and reads and writes its attributes through the `Attributes` trait; `src/apply.rs` runs the write cycle and `show`.
- `src/run.rs` runs each command; `src/outcome.rs` is what a command returns, as JSON and as a human document.
- None of these needs Linux to build or test: the device is plain files under `--sysfs`.
- `src/instructions.md` (`fanctl instructions`) and `skills/fanctl/SKILL.md` are renders of `.ctl/operator/`: one shared `contract.md.jinja`, its command table from ctl-core's `Surface` over the Clap grammar, and the skill's `version` from `CARGO_PKG_VERSION`.
  `tests/it/docs.rs` byte-compares both committed files with the render; edit the templates, never the renders, and regenerate with `UPDATE_OPERATOR_DOCS=1 cargo test docs` in CI's toolchain.
- `testdata/sysfs/` holds fixture sysfs trees: `ms-7e70` with firmware-like curves on all 8 channels, and trees with no device, two devices, and another board.
  `testdata/configs/workstation.yaml` holds the curves deployed on `workstation`; `testdata/show/` the expected `show` output before and after applying it.
- `tests/it/common/driver.rs` models the driver's store path, so `tests/it/apply.rs` runs the write cycle against the writes the driver refuses.
- The repository setup comes from the shared Copier template (project type `rust-crate`); `.copier-answers.yml` records its tag.
- Later slices (firmware NVRAM settings, BIOS profiles, sensor labels; see the board design) go in their own modules beside these, each with its own subcommand.

## Rules

- ctl's shared rules apply: always a pull request, never `--no-verify`, ctl-core owns presentation, every flag has a `FANCTL_` variable, and nothing local leaks into a tracked file.
- The pull request's CI verifies. Do not build or test before a push.
- A behaviour change moves code, tests, the operator docs, this file and `docs/` together (the ctl bundle contract).
