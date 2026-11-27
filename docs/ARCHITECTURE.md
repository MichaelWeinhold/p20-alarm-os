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
                timer then reboot(2), for rebooting into recovery, a sync then reboot(2),
                and for the boot's reason, read from /proc/cmdline
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
                as the panel goes off, suspends while it is off and Power is quiet, stays
                awake instead for a wake due within 5 s (issue 117), powers off after the
                window off the cable, starts dark on a boot by the alarm, boots the
                modem with its radio off, pulses the vibrator and sounds the speaker
                while it rings, each only while its toggle is on, blinks the red LED while the battery is low off the
                cable, saves a changed list and reboots into recovery when asked, is
                PID 1
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
| `hardware` | the traits `Clock`, `WakeAlarm`, `Suspend`, `PowerOff`, `Recovery`, `BootReason`, `Vibrator`, `Speaker`, `Sounds`, `Indicator`, `Battery`, `Display`, `Input`, `Settings`, `Modem`, and `HardwareError` |
| `boot` | `Boot`: why the phone started — the alarm, a press, the charger or anything else; only the alarm starts with the panel dark — ADR 0010, D4 |
| `window` | `Window`: two minutes suspended after the panel goes off, then *power off*, unless an alarm rings or is due within two minutes of the end, or `Ringer::held` holds it for a running timer, or for a paused one, answered as *hold*: suspended with no wake set; the RTC instant to suspend until; `power_on_at`, two minutes before the alarm — ADR 0010, D2 and D3; `alarmd` stays suspended instead on the cable or with the power-off toggle off |
| `input` | `InputEvent`: a touch at a panel pixel, the finger moved while held, its lift, or a press of `VolumeUp`, `VolumeDown` or `Power` |
| `schedule` | `arm_next`: read the clock, pick the earliest alarm, arm or disarm the RTC |
| `settings` | `Choices` and the record that holds them: two 4 KiB slots, each framed by magic, version, length, sequence and CRC-32; `load` takes the newest valid slot, a version-1 one with the power-off toggle on, a version-2 one with no saved timers, a version-3 one lit for 10 s, a version-4 one at the loudest volume, a version-5 one with the sound and the vibration on, a version-6 one with its timers in whole minutes, a version-7 one with `ringtone.wav` chosen, a version-8 one with the ramp off, `save` writes the other one as version 9, the chosen sound's file name behind a length byte, 0 for none, then the ramp length in two bytes — ADR 0011, ADR 0014 D4 |
| `battery` | `ChargeLimit`, `BatteryReading`, `stop_at_limit`, and the `Gauge`: a `Level` of low at the 20 % mark, full from the limit, middle between, and the capacity when it was read — ADR 0008, D3 |
| `face` | `Frame`, `Face`, `Screen` and `render`: the theme is black, figures in white, grey for what is off, red for a warning, and `ACCENT` for every icon that is lit — a gradient from blue `#57a2f3` to purple `#6e41d2`, laid across the icon's own square from its top-left to its bottom-right; every icon a Material Symbols glyph through `glyph::icon`, its em square filling the box it occupies; a bar along the bottom with an alarm clock, an hourglass and a gear for the alarm, timer and settings screens, the open one lit; the alarm screen adds the battery in the top-right corner at an eighth of the digit's unit as `battery_android_0` with one to three bars inside it and a percentage, red when low, and has two stages: the board, with the time in Inter Light, rasterised and blended over the frame, under the weekday and the day of the month, inside a dim ring that the `ACCENT` arc fills clockwise from the top by the share of a day left until the next armed alarm, an `add_circle` at the ring's lower right, dim once five alarms are saved, and up to five saved alarms below it, each its time, its days as `Daily`, nothing for a one-off, up to three short names or else initials, how long until it rings while it is armed, and a switch lit while armed; and the editor, a slider of an hour wheel and a minute wheel with a dim arrow above and below each, a `check` and a `delete` beside the picked time, the `delete` red while marked, and below them the seven days' initials, the chosen ones lit; the settings screen shows a power symbol and a switch, `toggle_on` lit while the phone powers off after the window, `toggle_off` dim while it stays suspended, below it a sun beside how long a press lights the panel, as `m:ss`, below that a `volume_up` speaker and a `vibration` phone, each beside a switch lit while the ring sounds or vibrates, between them the chosen sound's name without `.wav`, cut to its row, or a dim `None`, beside a `keyboard_arrow_down`, below those a `trending_up` arrow beside the ramp length as `m:ss`, or a dim switch while it is off, and below that a `build` wrench and a `restart_alt` arrow, both red while the row is marked, the rows moved up by the scroll and cut where they pass the top of the first row or the bar; a tap on the name's row opens the sound list in its place, a `close` above up to six names, the chosen one with a `check` in `ACCENT`, and a `keyboard_arrow_down` beside the `close` while there is a second page; the timer screen adds the battery as the alarm screen does, and has two stages: the list, with no clock, a dim ring in the board's place holding an hour, a minute and a second wheel at three quarters of the board's unit, its leading zero pairs grey, with a circled play at the ring's lower right, dim while the wheels read zero, and at its lower left a circled `+`, dim while the wheels read zero or four timers are saved, or a circled `delete` while the picked length is saved, red while marked, and up to four saved timers below the ring at the board's pitch, each its length as `m:ss`, or `h:mm:ss` from an hour, right-aligned so the colons line up, with a play; and the running stage, with the time left as `m:ss`, or smaller as `h:mm:ss` from an hour, over the length it counts from in grey, inside a dim ring that the `ACCENT` arc empties from, clockwise from the top, a `pause` below it, or a play and an X beside it while paused; a ringing alarm or timer draws the halo, with no bar: a closed `ACCENT` ring holding an `alarm` or an `hourglass` in `ACCENT`, the time in white and below it, grey, the alarm's time or the timer's length as `m:ss` or `h:mm:ss`, over a grey power symbol; while the volume is shown, a black panel at the left edge, over any screen or a ring, holds a `volume_up` speaker above a grey track filled in `ACCENT` from the bottom to its level; `target` maps a tap to a tab by thirds of the bar, to the board's `add_circle`, a saved alarm or its switch, to the editor's wheel rows, `check`, `delete` or a day, to a timer wheel's row, the ring's play or its `+` or `delete`, a saved timer or its play, to the running stage's pause or X, to the switch's row, the lit time's row, the sound's row, the sound name's row, the vibration's row, the ramp's row, the recovery row, each where the scroll has moved it, the list's `close`, its page turn, one of its names, or to nothing; `pitch` is the drag that steps a wheel once, and `reach` how far the settings rows scroll, nothing while they all fit above the bar |
| `glyph` | `rasterise`: one character of the Inter Light subset under `fonts/`, embedded with `include_bytes!`, scaled to a pixel height and centred on a point; `icon`: one `Icon` of the Material Symbols subset beside it, its em square scaled to a `Square` and centred on its point; `ring`: a band around a `Square` and the arc of it a share of a full turn sweeps clockwise from the top, both in one pass, each pixel's coverage taken from its distance to the centre and to the arc's two edges rather than from an outline; each hands its coverage to `face` to blend, and a glyph missing from its subset draws nothing |
| `ringer` | `Ringer`: up to five saved alarms, sorted by time and days with duplicates merged, each a one-off or repeating on its days and each with its own switch; quiet or ringing until Power, and the instant the soonest armed one next rings, a one-off disarmed once it has rung; the `add_circle` opens the editor on the current time, a tap on a saved alarm opens it on that alarm, and a tap on its switch arms or disarms it alone; in the editor a drag on a wheel or a tap on one of its arrows moves the picked time, a tap on a day turns it, the `check` saves — a new alarm armed, a saved one keeping its switch — and a second tap on the `delete`, red after the first, removes the saved alarm or discards a new one; any other touch, the panel going off or a ring clears the red; a drag on the settings screen scrolls its rows by the distance moved, as far as `reach` and back to the top, and starts at the top each time a tab or the panel going off leaves it; a tap on a settings row acts at its lift, and not at all once the finger has moved half a `pitch`; a tap on the switch's row turns the power-off toggle, on until turned; a tap on the lit time's row steps it through 10 s, 30 s, 1 min, 2 min and 5 min and back to 10 s; a tap on the sound's or the vibration's row turns that one, each on until turned; a tap on the ramp's row steps its length through off, 15 s, 30 s, 1 min, 2 min and 5 min and back to off, off until set; a ring with a ramp starts at volume 1 and climbs to the volume over that length, `heard` the level it plays at and `ramp_left` the seconds to its next step, and a volume key while it climbs ends the ramp and steps from the level heard; one sound for every alarm and the timer, by file name, none until chosen, from the `.wav` names listed at start-up, sorted; a tap on the sound name's row lists them a page of six at a time, the page turn steps to the next page and round, and a tap on a name chooses it and closes the list, as do its `close`, a tab and a ring; a first tap on the recovery row marks it red and a second asks for recovery, taken once by `take_recovery`, the red cleared as the `delete`'s is; up to four saved timers from 00:00:01 to 23:59:59, set on an hour, a minute and a second wheel, sorted; the ring's play starts the picked length when none runs, saved or not and whatever the list holds, the `+` saves it while fewer than four are saved, a tap on a saved timer loads its length into the wheels, and the play on each starts it when none runs; a second tap on the `delete`, red after the first, removes the picked length, the red cleared as the editor's is; one running timer, paused and resumed by the pause, ended while paused by the X, ringing at its end until a press of the side button, which returns to the list, held in memory only and across a tab switch; the volume, 1 to 15 and 15 until set, a step up or down at each press of Volume Up or Volume Down, ringing or not; `show` draws the open screen, and `listen` sends a tab tap and the lit time to the `Light` and every tap, move, lift or press to the `Ringer`, touch only while no alarm rings |
| `ramp` | `Ramp`: one step from 1 up to the volume every length ÷ steps seconds, whole steps from the seconds since the ring began, held at 1 while the clock reads before it; `next_ramp_seconds` the tap cycle |
| `light` | `Light`: the panel lit by a press for the lit time, 10 s until set, kept lit by a touch or a ringing alarm, off at a press or the timeout — ADR 0008, D2; the ringer hears input only while it is lit; the open `Screen`, back to the alarm screen when the panel goes off or an alarm rings; the volume shown for 3 s after a volume key on a lit panel, gone with the panel; `settling`, the 2 s after each press of Power, in which `alarmd` does not suspend, since a press inside a suspend's entry leaves no press able to wake it (issue 117) |
| `vibration` | `Vibration`: a 1 s pulse every 2 s while the alarm rings, stopped at once by a dismissal; a vibrator that fails is left alone until the next ring |
| `sound` | `Volume`, its gain in Q16, unity at 15 and 3 dB less a step; `SoundName`: a file name of `A`–`Z`, `a`–`z`, `0`–`9`, `.`, `_` and `-`, at most 64 bytes, starting with a letter or a figure and ending in `.wav`; `Sound`: the chosen sound loaded once into the speaker, and again when the choice changes, a sound that does not load or none chosen leaving the speaker unstarted and the ring on the vibrator alone; the speaker started once at the level the ringer is heard at when an alarm or a timer rings, sent a new volume while it rings, and stopped at a dismissal; a speaker that fails to start is left alone until the next ring |
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
alarm or disarms and suspends instead (ADR 0010, D2). On *hold* it disarms and suspends,
whatever the cable. A press that wakes it arrives as
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
(ADR 0015). At start-up it reads `dsp-init.bin` and `speaker-route.tsv`
off `system`, opens `hifi_misc`, `controlC0` and `pcmC0D0p`, and resolves every route
control to its element number and value, an enumerated one by its item's name; a missing
file, node or control is a `HardwareError` before anything rings. A load reads the
chosen WAV from `/system/media/audio/alarms`, and a start with none loaded is refused.
`AlarmSounds` lists that directory's files whose names `SoundName` takes (ADR 0014, D4). It then replays the
DSP's messages, once. A start within 5 s of the replay waits out the rest: a stream the
DSP is sent sooner plays at an eighth of its speed, unheard, and its watchdog fires when
the stream ends (`docs/device-history/phone.md`, issue 92). Each start writes the
defaults and then the speaker's controls, less `CLASSD_VOLTAGE_CONFIG`, which refuses its
value, asks for 16-bit 48 kHz stereo in two periods of 960 frames, checks that the kernel
kept them, and loops the sound on a thread of its own, preparing again after an underrun.
Every write is a whole period, the end of the sound running on into its start: a short
last period leaves the kernel copying into its buffer at an offset it faults on, and the
phone panics (issue 137). Each period is scaled by the volume's gain as it is written, so a new volume is heard within about 60 ms, the 20 ms period being scaled and the two queued ahead of it. A stop drops the
stream, which ends the thread's blocked write, frees its parameters, and puts the
speaker's controls back to their defaults.

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
   alarm and power off, unless the cable is in or the power-off toggle is off (ADR 0010).
4. On wake — RTC or power button — decide which: ring for the alarm, or show the time.
5. Dismiss with Power, then back to 2.

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
