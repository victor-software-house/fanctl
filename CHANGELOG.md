# Changelog

## fanctl 0.0.1

### Added

- First release. `fanctl` writes declared fan curves into the Nuvoton NCT6687D-R of an MSI PRO X870E-P WIFI (MS-7E70) through the `nct6687` hwmon driver.
`fanctl check` validates a config of 7-point curves by board header, `fanctl apply` writes every point in an order the driver accepts and reads each one back, and `fanctl show` prints the curves the driver holds as a config.
Releases are static musl binaries for x86_64 Linux ([`144830d`](https://github.com/victor-software-house/fanctl/commit/144830d3faf29153802c7ad2d7263f9d87f3c8e0)).

