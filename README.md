# fanctl

Declarative fan curves for the Nuvoton NCT6687D-R on MSI boards, on Linux.
A config file names each fan by its board header and gives its curve as 7 points of temperature and duty.
`fanctl apply` writes those curves into the chip through the `nct6687` hwmon driver, and the chip then runs them on its own.
No daemon, no MSI software, no fancontrol.

Status: in development; no release yet.

## Supported hardware

- Board: MSI PRO X870E-P WIFI (MS-7E70).
  Each board has its own table that maps headers such as `CPU_FAN1` or `SYS_FAN2` to the driver's pwm channels; a header the table does not list is an error.
- Driver: the out-of-tree [`nct6687`](https://github.com/Fred78290/nct6687d) driver with the `pwmN_auto_pointM_temp` and `pwmN_auto_pointM_pwm` attributes ([Fred78290/nct6687d#228](https://github.com/Fred78290/nct6687d/pull/228)).
  fanctl finds the device by its hwmon `name`, never by a fixed `hwmonN` number.

## What it does

- `fanctl check <config>` validates a config without touching the hardware.
- `fanctl apply <config>` writes every point of every configured curve, then reads all of them back and fails on any mismatch.
- `fanctl show` prints the curves the driver holds now, in the config's own format.

## Config

```yaml
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
```

Each fan takes exactly 7 points: `temp` in whole degrees C (0 to 127) and `duty` in percent (0 to 100, one decimal at most).
Neither may fall from one point to the next.
A header the file does not name keeps its curve.

## Permissions

`show` and `check` run as any user. `apply` writes attributes the driver creates for root only, so it runs as root, for example from a boot service.

## License

MIT
