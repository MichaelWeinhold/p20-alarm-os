# Architecture

## The image

```
┌──────────────────────────────────────────────┐
│ kernel partition (boot.img)                  │
│                                              │
│  ┌────────────────┐  ┌─────────────────────┐ │
│  │ vendor kernel  │  │ initramfs           │ │
│  │ (stock, as the │  │  /init = alarmd     │ │
│  │  ROM ships it) │  │  /dev, /sys, /proc  │ │
│  └────────────────┘  └─────────────────────┘ │
└──────────────────────────────────────────────┘
recovery_*, erecovery_*, vbmeta, system, vendor, … — not written by the image
```

The kernel is reused as it is (ADR 0002). `boot.img` is unpacked, its ramdisk replaced by
our initramfs, and repacked with the kernel, command line, base addresses and header
version unchanged. The image is written only to `kernel`; the phone has no `boot`.

The initramfs holds one static binary at `/init`, which mounts `devtmpfs`, `sysfs` and
`proc` itself, since nothing else in the image will.

## The program

```
crates/
  alarm-core/   logic only: time, alarms, scheduling, and the hardware traits
  (adapters)    one crate per device family, each implementing alarm-core's traits
  (alarmd)      the binary: opens the adapters, runs the loop, is PID 1
```

Only `alarm-core` exists so far. The rest arrive with the roadmap phases that need them.

### `alarm-core`

| Module | What it holds |
|---|---|
| `time` | `UnixSeconds`, `TimeOfDay`, `Weekday`, `UtcOffset` — civil time without a tz database |
| `alarm` | `Alarm`, `Weekdays`, and when an alarm next fires after a given instant |
| `hardware` | the traits `Clock`, `WakeAlarm`, `Suspend`, `Vibrator`, and `HardwareError` |
| `schedule` | `arm_next`: read the clock, pick the earliest alarm, arm or disarm the RTC |

Display and input traits are not defined yet. The panel is the legacy framebuffer,
`/dev/graphics/fb0`, with no DRM (ADR 0004); what the touch controller reports is read off
the device first.

### The loop, as intended

1. Read the clock; draw the time.
2. Arm the RTC for the earliest alarm (`schedule::arm_next`).
3. Turn the panel off and suspend.
4. On wake — RTC or power button — decide which: ring for the alarm, or show the time.
5. Snooze or dismiss from touch or the volume keys, then back to 2.

## Host tooling

```
scripts/        the repository's guards; later the boot image and initramfs builders
tests/          their tests
```

The boot image builder will be a Python round trip that is tested to reproduce the stock
`boot.img` byte for byte before it changes anything.

## Debugging without a console

There is no serial port. The first image built is a debug initramfs that brings up USB
gadget networking and an SSH server, or keeps logs in `ramoops`, so that a boot that does
not draw can still be read. → `docs/ROADMAP.md`, P2.
