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
  input/        the adapter for the side keys and touch: the evdev nodes, found by name, and
                the touch firmware the kernel asks for, answered from odm
  pid1/         what every /init takes from the kernel: mounts, the kernel log, a child
                on a terminal, reaping, the reboot into recovery
  alarmd/       the binary: opens the adapters, draws the time and the alarm, waits on input
                until the next minute, is PID 1
  init-probe/   a probe /init that runs the charge stop and logs the power nodes
  debug-init/   the debug /init: the ROM's shell on a USB serial gadget (ADR 0009)
```

The other adapters arrive with the roadmap phases that need them.

`framebuffer` maps `fb0` once and holds it open for the life of the process: the vendor
driver frees the buffer on the last close, and an `mmap` after that can panic the kernel.

### `alarm-core`

| Module | What it holds |
|---|---|
| `time` | `UnixSeconds`, `TimeOfDay`, `Weekday`, `UtcOffset` — civil time without a tz database |
| `alarm` | `Alarm`, `Weekdays`, and when an alarm next fires after a given instant |
| `hardware` | the traits `Clock`, `WakeAlarm`, `Suspend`, `Vibrator`, `Battery`, `Display`, `Input`, and `HardwareError` |
| `input` | `InputEvent`: a touch at a panel pixel, or a press of `VolumeUp`, `VolumeDown` or `Power` |
| `schedule` | `arm_next`: read the clock, pick the earliest alarm, arm or disarm the RTC |
| `battery` | `ChargeLimit`, `BatteryReading`, `stop_at_limit` and the 20 % mark — ADR 0008, D3 |
| `face` | `Frame`, `Face` and `render`: the time in seven-segment digits, the battery mark, the alarm below, amber while ringing; `target` maps a tap to the alarm digit it steps |
| `ringer` | `Ringer`: one daily alarm, armed or not, quiet, ringing or snoozed for 9 minutes; `show` and `listen` drive it from the clock and `Input` |

The panel is the legacy framebuffer, `/dev/fb0` under our `devtmpfs`, with no DRM (ADR
0004).

`input` finds `hisi_gpio_key`, `hisi_on` and `huawei,ts_kit` by name in
`/sys/class/input`, and opens each when it appears rather than once at start-up: the touch
panel registers only after its firmware request is answered, which on Android is
`ueventd`'s job. A thread of `alarmd` mounts `odm` read-only and answers each request in
`/sys/class/firmware` from `etc/firmware/`, refusing one it has no file for. The touch
controller sleeps while the panel is off, so touch works only while `fb0` is held open.

### The loop, as intended

1. Read the clock; draw the time.
2. Arm the RTC for the earliest alarm (`schedule::arm_next`).
3. Turn the panel off and suspend.
4. On wake — RTC or power button — decide which: ring for the alarm, or show the time.
5. Snooze or dismiss from touch or the volume keys, then back to 2.

## Host tooling

```
scripts/        the repository's guards and the boot image round trip
tests/          their tests
```

`scripts/boot_image.py` unpacks a header-v0 image into its sections and a `header.json`, and
repacks it; it refuses an image it could not reproduce byte for byte, and reproduces the
stock image in `kernel`.

## Debugging without a console

There is no serial port. The first image built is a debug `/init` that serves the ROM's
`mksh` on a CDC-ACM USB gadget, `/dev/ttyGS0` on the phone and `/dev/cu.usbmodem*` on the
Mac, and reboots into TWRP when the shell exits. Every `/init` logs to `/dev/kmsg`, which
pstore keeps across a reboot; `dfx` keeps a panic in `/init`. → ADR 0009; `docs/ROADMAP.md`,
P2.
