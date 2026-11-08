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
6. **Read back.**
   After writing, fanctl reads every point and exits non-zero on any mismatch, naming the fan and the point.
7. **No daemon.**
   The chip runs the curves itself.
8. **Releases are GitHub Release binaries only**, static musl builds, as for rgbctl.

## Rules

- ctl's shared rules apply: always a pull request, never `--no-verify`, ctl-core owns presentation, every flag has a `FANCTL_` variable, and nothing local leaks into a tracked file.
- The pull request's CI verifies. Do not build or test before a push.
- A behaviour change moves code, tests, the operator docs, this file and `docs/` together (the ctl bundle contract).
