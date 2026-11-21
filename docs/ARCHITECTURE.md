# Architecture

## The image

```
┌───────────────────────────┐      ┌──────────────────────────────┐
│ kernel (mmcblk0p31)       │      │ system (mmcblk0p52), ext4    │
│  vendor kernel, stock,    │ ───▶ │  mounted read-only as /      │
│  as the ROM ships it      │      │  /init = alarmd              │
└───────────────────────────┘      └──────────────────────────────┘
     bootloader appends: skip_initramfs root=PARTUUID=<system> init=/init
recovery_*, erecovery_*, vbmeta, vendor, … — not written by the image
```

The kernel is reused as it is (ADR 0002), and `kernel` keeps the stock image byte for
byte. The bootloader loads no ramdisk from it: it mounts `system` as `/` and runs `/init`
from there, so that is where our binary goes (ADR 0006). The phone has no `boot`.

`/init` is one static binary, which mounts `devtmpfs`, `sysfs` and `proc` itself, since
nothing else in the image will. Until our image outgrows it, the ROM's own files stay on
`system` beside it, with the ROM's init renamed to `/init.rom`.

## The program

```
crates/
  alarm-core/   logic only: time, alarms, scheduling, the charge stop, and the hardware traits
  battery/      the adapter for the battery and the charger, over sysfs
  clock/        the adapter for the clock, over the system time the kernel sets from the RTC
  framebuffer/  the adapter for the panel: /dev/fb0, mapped once, drawn into a page and panned
  indicator/    the adapter for the low-battery warning: the LED class device `red`, blinked
                by the PMU through its timer trigger (ADR 0013, D1)
  input/        the adapter for the side keys and touch: the evdev nodes, found by name, and
                the touch firmware the kernel asks for, answered from odm
  modem/        the adapter for the modem: its partitions mounted, teecd started, the boot
                over his_modem, and the radio off over appvcom (ADR 0012, D3)
  pid1/         what every /init takes from the kernel: mounts, the kernel log, a child
                on a terminal, reaping, the reboot into recovery, the power-off, a
                partition found by name
  power/        the adapters for powering off with a power-on alarm, a CLOCK_POWEROFF_ALARM
                timer then reboot(2), and for the boot's reason, read from /proc/cmdline
  storage/      the adapter for the settings: the first 8192 bytes of reserved4, a slot
                written, flushed and read back (ADR 0011)
  suspend/      the adapters for the RTC's wake alarm and for suspending, over rtc0's
                wakealarm and /sys/power/state
  vibrator/     the adapter for the vibrator: the LED class device `vibrator`, timed by
                its transient trigger
  alarmd/       the binary: opens the adapters, draws the time and the alarm, waits on input
                until the next minute or until the panel is due off, saves a changed alarm
                as the panel goes off, suspends while it is off, powers off after the
                window off the cable, starts dark on a boot by the alarm, boots the
                modem with its radio off, pulses the vibrator while it rings, blinks
                the red LED while the battery is low off the cable, is PID 1
  init-probe/   a probe /init that runs the charge stop and logs the power nodes
  debug-init/   the debug /init: the ROM's shell on a USB serial gadget (ADR 0009)
```

The other adapters arrive with the roadmap phases that need them.

`framebuffer` maps `fb0` once and holds it open for the life of the process: the vendor
driver frees the buffer on the last close, and an `mmap` after that can panic the kernel.
It turns the panel off with `FBIOBLANK` on that descriptor, never by closing it.

### `alarm-core`

| Module | What it holds |
|---|---|
| `time` | `UnixSeconds`, `TimeOfDay`, `Weekday`, `UtcOffset` — civil time without a tz database |
| `alarm` | `Alarm`, `Weekdays`, and when an alarm next fires after a given instant |
| `hardware` | the traits `Clock`, `WakeAlarm`, `Suspend`, `PowerOff`, `BootReason`, `Vibrator`, `Indicator`, `Battery`, `Display`, `Input`, `Settings`, `Modem`, and `HardwareError` |
| `boot` | `Boot`: why the phone started — the alarm, a press, the charger or anything else; only the alarm starts with the panel dark — ADR 0010, D4 |
| `window` | `Window`: two minutes suspended after the panel goes off, then *power off*, unless an alarm rings, is snoozed or is due within two minutes of the end; the RTC instant to suspend until; `power_on_at`, two minutes before the alarm — ADR 0010, D2 and D3 |
| `input` | `InputEvent`: a touch at a panel pixel, or a press of `VolumeUp`, `VolumeDown` or `Power` |
| `schedule` | `arm_next`: read the clock, pick the earliest alarm, arm or disarm the RTC |
| `settings` | `Choices` and the record that holds them: two 4 KiB slots, each framed by magic, version, length, sequence and CRC-32; `load` takes the newest valid slot, `save` writes the other one — ADR 0011 |
| `battery` | `ChargeLimit`, `BatteryReading`, `stop_at_limit`, and the `Gauge`: a `Level` of low at the 20 % mark, full from the limit, middle between, and the capacity when it was read — ADR 0008, D3 |
| `face` | `Frame`, `Face`, `Screen` and `render`: a bar along the bottom with a bell, an hourglass and a gear for the alarm, timer and settings screens, the open one lit; the alarm screen adds the time in seven-segment digits, the battery in the top-right corner as one to three bars and a percentage, red when low, and the alarm below; a ringing alarm fills the panel amber with no bar; `target` maps a tap to a tab by thirds of the bar, to the alarm digit it steps, or to nothing on an empty screen |
| `ringer` | `Ringer`: one daily alarm, armed or not, quiet, ringing or snoozed for 9 minutes, and the instant it next rings; `show` draws the open screen, and `listen` sends a tab tap to the `Light` and any other tap or press to the `Ringer` |
| `light` | `Light`: the panel lit for 10 s by a press, kept lit by a touch or a ringing alarm, off at a press or the timeout — ADR 0008, D2; the ringer hears input only while it is lit; the open `Screen`, back to the alarm screen when the panel goes off or an alarm rings |
| `vibration` | `Vibration`: a 1 s pulse every 2 s while the alarm rings, stopped at once by a snooze or a dismissal; a vibrator that fails is left alone until the next ring |
| `warning` | `Warning`: the indicator lit while the battery reads at or below the 20 % mark off the cable, cleared otherwise and on a battery that cannot be read; written on the first turn and then only on a change, since a write restarts the PMU's blink; an indicator that fails is left alone until the battery changes — ADR 0013, D2 |

The panel is the legacy framebuffer, `/dev/fb0` under our `devtmpfs`, with no DRM (ADR
0004).

`suspend` arms the RTC by writing `0`, then the epoch, to `wakealarm`, and reads it back:
the kernel takes an epoch at or before its now as a disarm and reports no error, so only
the read-back shows it. Writing `mem` to `/sys/power/state` returns after the wake, and
fails when the kernel aborts the suspend. Each time the panel goes off, `alarmd` asks
`Window` what to arm and suspends. On *power off* with the cable out, `power` sets a
`CLOCK_POWEROFF_ALARM` timer 60 s after the power-on instant, since the kernel arms the
PMU 60 s before the timer, and calls `reboot(2)`; with no alarm it zeroes the timer. On
the cable, when the cable cannot be read, or when the power-off fails, it arms the next
alarm or disarms and suspends instead (ADR 0010, D2). A press that wakes it arrives as
the power key's input event, read within a second of the resume.

`input` finds `hisi_gpio_key`, `hisi_on` and `huawei,ts_kit` by name in
`/sys/class/input`, and opens each when it appears rather than once at start-up: the touch
panel registers only after its firmware request is answered, which on Android is
`ueventd`'s job. A thread of `alarmd` mounts `odm` read-only and answers each request in
`/sys/class/firmware` from `etc/firmware/`, refusing one it has no file for. The touch
controller sleeps while the panel is off, so touch works only while `fb0` is held open.

`vibrator` drives `hisi_vibrator`, the PMU's motor, which the driver only switches on and
off. At start-up it checks that `trigger` offers `transient` and writes `transient`, which
resets `duration`; a pulse writes `duration`, then `state` 1, then `activate` 1, and the
kernel's timer switches the motor off. A stop writes `activate` 0.

`indicator` drives `/sys/class/leds/red`, which the PMU blinks with no wake of the AP. At
start-up it checks that `trigger` and `brightness` exist. A warning writes `trigger`
`timer`, then `brightness` 1, then `delay_off` 2000, the node the timer trigger adds: a
write to `brightness` sets the PMU's blink back to steady, and only the last delay written
survives (ADR 0013, D1). A clear writes `brightness` 0. The power-off turns it off on its
own, through the driver's `shutdown`.

`modem` boots the modem as the debug `/init` did by hand, on a thread of `alarmd`: a
`tmpfs` on `/mnt` and on `/data`, a link in `/dev/block/by-name` to every partition, as
`ueventd` makes them, since the kernel's NV driver opens `modemnvm_*` there and otherwise
jams the NV channel that every suspend checks, the six partitions found by name and
mounted with the ROM's options less `context=`, `/vendor/bin/teecd` started and held, `1`
written to `modem_sysboot_start`, and `balong_power`'s `state` polled for 30 s until it
reads `MODEM_STATE_READY`. It then sends `AT+CFUN=0` on `/dev/appvcom` and reads `AT+CFUN?`
back for `+CFUN: 0`, reading each answer to its `OK` or refusal, past the modem's echo and
queued reports, within 5 s, and tries again for 60 s. `appvcom` is opened blocking and read
on a thread of its own, since a non-blocking read answers `EPERM`. The first suspend waits for that thread, so the kernel never
suspends mid-boot. A failure is logged and leaves the alarm working at the higher draw
(ADR 0012, D3).

### The loop, as intended

1. Load the settings from `reserved4` (ADR 0011); read the clock; draw the time.
2. Arm the RTC for the earliest alarm (`schedule::arm_next`).
3. Turn the panel off and suspend; after two minutes without a press, arm the power-on
   alarm and power off (ADR 0010).
4. On wake — RTC or power button — decide which: ring for the alarm, or show the time.
5. Snooze or dismiss from touch or the volume keys, then back to 2.

## Host tooling

```
scripts/        the repository's guards, the boot image round trip and the serial shell
tests/          their tests
```

`scripts/boot_image.py` unpacks a header-v0 image into its sections and a `header.json`, and
repacks it; it refuses an image it could not reproduce byte for byte, and reproduces the
stock image in `kernel`.

## Debugging without a console

There is no serial port. The first image built is a debug `/init` that serves the ROM's
`mksh` on a CDC-ACM USB gadget, `/dev/ttyGS0` on the phone and `/dev/cu.usbmodem*` on the
Mac, starts the shell again when it ends, and reboots into TWRP when it ends with
`/mnt/recovery` present; `scripts/serial_shell.py` runs one command
on it and pushes a file to `/mnt`. Every `/init` logs to `/dev/kmsg`, which
pstore keeps across a reboot; `dfx` keeps a panic in `/init`. → ADR 0009; `docs/ROADMAP.md`,
P2.
