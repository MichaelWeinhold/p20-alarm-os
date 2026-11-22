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
  speaker/      the adapter for the speaker: the HAL's DSP init replayed on hifi_misc,
                the speaker's route set on controlC0, the alarm sound looped on pcmC0D0p
  alarmd/       the binary: opens the adapters, draws the time and the alarms, waits on input
                until the next minute or until the panel is due off, saves a changed list
                as the panel goes off, suspends while it is off, powers off after the
                window off the cable, starts dark on a boot by the alarm, boots the
                modem with its radio off, pulses the vibrator and sounds the speaker
                while it rings, is PID 1
  init-probe/   a probe /init that runs the charge stop and logs the power nodes
  debug-init/   the debug /init: the ROM's shell on a USB serial gadget (ADR 0009)
  dsp-recorder/ run under the ROM, not in the image: records the audio HAL's ioctls on
                /dev/hifi_misc by ptrace (ADR 0015)
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
| `hardware` | the traits `Clock`, `WakeAlarm`, `Suspend`, `PowerOff`, `BootReason`, `Vibrator`, `Speaker`, `Battery`, `Display`, `Input`, `Settings`, `Modem`, and `HardwareError` |
| `boot` | `Boot`: why the phone started — the alarm, a press, the charger or anything else; only the alarm starts with the panel dark — ADR 0010, D4 |
| `window` | `Window`: two minutes suspended after the panel goes off, then *power off*, unless an alarm rings, is snoozed or is due within two minutes of the end, which `Ringer::held` extends to a running timer; the RTC instant to suspend until; `power_on_at`, two minutes before the alarm — ADR 0010, D2 and D3; `alarmd` stays suspended instead on the cable or with the power-off toggle off |
| `input` | `InputEvent`: a touch at a panel pixel, the finger moved while held, its lift, or a press of `VolumeUp`, `VolumeDown` or `Power` |
| `schedule` | `arm_next`: read the clock, pick the earliest alarm, arm or disarm the RTC |
| `settings` | `Choices` and the record that holds them: two 4 KiB slots, each framed by magic, version, length, sequence and CRC-32; `load` takes the newest valid slot, a version-1 one with the power-off toggle on, a version-2 one with no saved timers, a version-3 one lit for 10 s, `save` writes the other one as version 4 — ADR 0011 |
| `battery` | `ChargeLimit`, `BatteryReading`, `stop_at_limit`, and the `Gauge`: a `Level` of low at the 20 % mark, full from the limit, middle between, and the capacity when it was read — ADR 0008, D3 |
| `face` | `Frame`, `Face`, `Screen` and `render`: a bar along the bottom with a bell, an hourglass and a gear for the alarm, timer and settings screens, the open one lit; the alarm screen adds the time in seven-segment digits, the battery in the top-right corner as one to three bars and a percentage, red when low, the active alarm or dim dashes, a slider of an hour wheel and a minute wheel with a dim arrow above and below each and a `+` beside the picked time, every stroke a third of the digit's unit, and up to four saved alarms, the active one lit, each with a cross; the settings screen shows a power symbol and a switch, lit with its knob right while the phone powers off after the window, dim with its knob left while it stays suspended, and below it a sun beside how long a press lights the panel, as `m:ss`; the timer screen adds the time left or dim dashes beside a play, lit while the countdown is above zero, or a stop while one runs, a slider of a minute wheel and a second wheel with a `+`, and up to four saved timers, the one on the wheels lit, each with a cross; a ringing alarm or timer fills the panel amber with no bar, a timer showing its length; `target` maps a tap to a tab by thirds of the bar, to a wheel's row, the `+`, a saved alarm or its cross, to the timer's start row, to the switch's row, the lit time's row, or to nothing; `pitch` is the drag that steps a wheel once |
| `ringer` | `Ringer`: up to four daily alarms, sorted, at most one of them active; quiet, ringing or snoozed for 9 minutes, and the instant the active one next rings; the slider's picked time, moved by a drag on a wheel or a tap on one of its arrows, added to the list by the `+`; a tap on a saved alarm makes it active, or disarms it when it already is, and a second tap on its cross, red after the first, removes it; any other touch, the panel going off or a ring clears the red; a tap on the switch's row turns the power-off toggle, on until turned; a tap on the lit time's row steps it through 10 s, 30 s, 1 min, 2 min and 5 min and back to 10 s; up to four saved timers from 00:01 to 99:59, sorted, a tap on one putting it on the wheels; one running timer, started and stopped by the start row, ringing at its end until a press of the side button, held in memory only; `show` draws the open screen, and `listen` sends a tab tap and the lit time to the `Light` and every tap, move, lift or press to the `Ringer`, touch only while no alarm rings |
| `light` | `Light`: the panel lit by a press for the lit time, 10 s until set, kept lit by a touch or a ringing alarm, off at a press or the timeout — ADR 0008, D2; the ringer hears input only while it is lit; the open `Screen`, back to the alarm screen when the panel goes off or an alarm rings |
| `vibration` | `Vibration`: a 1 s pulse every 2 s while the alarm rings, stopped at once by a snooze or a dismissal; a vibrator that fails is left alone until the next ring |
| `sound` | `Sound`: the speaker started once when an alarm or a timer rings and stopped at a snooze or a dismissal; a speaker that fails to start is left alone until the next ring |

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
It reports protocol A with no slots: `BTN_TOUCH` 1 in a finger's first report, a position
in each report while it is held, and `BTN_TOUCH` 0 when it lifts. The decoder turns those
into a touch, moves and a lift, keeps only the last of a read's moves, and drops the hold
at a `SYN_DROPPED`, sending no lift.

`vibrator` drives `hisi_vibrator`, the PMU's motor, which the driver only switches on and
off. At start-up it checks that `trigger` offers `transient` and writes `transient`, which
resets `duration`; a pulse writes `duration`, then `state` 1, then `activate` 1, and the
kernel's timer switches the motor off. A stop writes `activate` 0.

`speaker` drives the codec's own class-D through the HiFi DSP, as the ROM's HAL does
(ADR 0015). At start-up it reads `dsp-init.bin`, `speaker-route.tsv` and `ringtone.wav`
off `system`, opens `hifi_misc`, `controlC0` and `pcmC0D0p`, and resolves every route
control to its element number and value, an enumerated one by its item's name; a missing
file, node or control is a `HardwareError` before anything rings. The first start
replays the DSP's messages once. Each start writes the defaults and then the speaker's
controls, less `CLASSD_VOLTAGE_CONFIG`, which refuses its value, asks for 16-bit 48 kHz
stereo in two periods of 960 frames, checks that the kernel kept them, and loops the
sound on a thread of its own, preparing again after an underrun. A stop drops the
stream, which ends the thread's blocked write, and puts the speaker's controls back to
their defaults.

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
   alarm and power off, unless the cable is in or the power-off toggle is off (ADR 0010).
4. On wake — RTC or power button — decide which: ring for the alarm, or show the time.
5. Snooze or dismiss from touch or the volume keys, then back to 2.

## Host tooling

```
scripts/        the repository's guards, the boot image round trip, the serial shell and
                the audio files' recording and push
tests/          their tests
```

`scripts/dsp_init.py` runs `dsp-recorder` under the ROM and writes the DSP init from two
recordings that agree; `scripts/speaker_route.py` reads the speaker's route out of the
`odm` backup; `scripts/system_push.py` pushes files onto `system` from TWRP, each checked
by sha256 and size before and after its rename. `scripts/alarm_sound.py` converts a sound
to 16-bit 48 kHz stereo WAV with `ffmpeg` and pushes it beside the ROM's alarms; it
replaces no file and refuses any other header. → ADR 0014, D3; ADR 0015;
`docs/manual-steps/audio.md`.

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
