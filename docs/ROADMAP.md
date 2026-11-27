# Roadmap

Ordered so that every phase ends with something that can be judged on the phone, and so
that the riskiest unknowns come before the work that depends on them. Each phase is one
or more issues, worked per `.claude/skills/feature-lifecycle/SKILL.md`. What is open is on
the issue tracker; this file is the order.

## P0 — Repository

**Done** — issue 1. The workflow, the guards, the pipeline, and `alarm-core` with its
scheduling logic tested on the host.

## Research before P1

- **Which kernel the image boots.** **Decided** — the kernel already on the phone, in
  ADR 0004. The candidates were the current ROM's kernel, a postmarketOS `huawei-anne`
  port, and Huawei's published source.
- **Reaching and flashing the phone safely.** **Decided** — in ADR 0005: there is no
  tethered `fastboot boot`, so every try writes `kernel` from fastboot, and the rollback
  writes the stock image back. It leaves a `flashing-the-phone` skill to build; the device
  preflight is `scripts/device_preflight.py`, issue 19.
- **Where the image's `/init` lives.** **Decided** — on `system`, in ADR 0006, with
  `kernel` left stock; issue 17's probe ran there as PID 1.

## P1 — Boot image round trip

**Done** — issue 8. Unpack the stock image in `kernel` and repack it unchanged, byte for
byte. Our image no longer needs it — `kernel` stays stock (ADR 0006).

**Done when:** the repacked image equals the original, by test, and boots the ROM as
before.

## P2 — Debug lifeline

A minimal `/init` on `system` that serves a shell on a USB serial gadget, with the log in
pstore. → ADR 0009, issue 9. Its host end is `scripts/serial_shell.py`, issue 42.

**Done when:** the laptop reaches a shell on the phone over USB, and a failed boot leaves a
log that can be read.

## Research before P3

- **How the phone charges and spares its old battery.** **Decided** — in ADR 0007 and
  0008: the kernel charges under our `/init`; the phone runs on its battery, sleeps
  between uses, lights the panel only when used, and `alarmd` stops a charge at 80 %
  through `enable_charger`. The display adapter takes the panel over from the bootloader.
  Both are follow-ups of issue 27, landing with P3; the suspend draw is P5's.

## P3 — Draw

**Done** — issue 10. `alarmd` as `/init` draws the time on the panel.

## P4 — Touch and buttons

Read `/dev/input/event*`; set an alarm and dismiss it.

**Done** — issue 11. The alarm is set by touch, rings on the panel and Power dismisses
it; since issue 113, Power is the only key that ends a ring.

## P5 — Wake from suspend

Arm `/dev/rtc0`, suspend, and wake on time. The whole project depends on this, so it is
proven as early as P2 allows, even before the clock looks right.

**Done when:** an alarm armed for five minutes ahead wakes a suspended phone, the draw
while suspended is read with USB input cut, and both readings are in
`docs/device-history/`. → ADR 0010, which supersedes ADR 0008, D1.

**Proven** — issue 12. The RTC woke a suspend to the second, a suspend draws about 53 mA,
and a power-on alarm switched a powered-off phone on. The adapters and the two-minute
window are built; the power-off with its power-on alarm, off the cable only, is issue
52.

## P6 — Vibrate

Ring the alarm through the vibrator.

**Built** — issue 13. The alarm pulses `hisi_vibrator` through its LED class device and
the transient trigger, and woke a suspended phone vibrating; a dismissal stops it at
once.

## Research before P7

- **Where settings persist across a reboot.** **Decided** — in ADR 0011: a raw record of
  two checksummed slots on `reserved4`, a partition the ROM never used, with `system`
  left read-only. The storage adapter is its follow-up, and unblocks ADR 0010's power-off.
- **Where alarm sounds are kept.** **Decided** — in ADR 0014: as files on `system`,
  read-only, where the ROM's alarm sounds already are. The settings stay on `reserved4`,
  and their two follow-ups wait for the speaker.

## P7 — Settings and a timer

Issue 31, split into children. A navigation bar with an alarm, a timer and a settings
tab (79), an alarm screen with a slider and a list of saved alarms (80), a countdown timer
armed through the RTC like an alarm (81), storage once alarm sounds are kept (82), and a
settings toggle for powering off (83).

**Done when:** its five children are closed, an alarm set on the phone survives a reboot,
and a timer wakes a suspended phone when it ends.

## Research before the speaker

- **Speaker audio without Android.** **Decided** — in ADR 0015: the codec's class-D,
  routed by the HAL's `media-speaker` controls, after `alarmd` replays the HAL's DSP init
  recorded under the ROM. Sounds are WAV files converted on the host. Its two follow-ups
  are a recorder with its host scripts, and the `speaker` adapter.

## Research before smooth type

- **Smooth type and thinner icons.** **Decided** — in ADR 0016: Inter at weight 300 and
  Material Symbols, rasterised by `face` at the layout's unit and blended into the frame,
  12.9 ms per frame for a clock's worth of glyphs on the Kirin 659. Its two follow-ups are
  the figures (104) and the icons (105).

## Research before the timer and settings redesign

- **The timer list, the settings screen and longer lists.** **Decided** — in ADR 0017:
  the timer's picker in a ring with a start and a save on it, lengths in the short form,
  labelled and grouped settings with the battery, a scrolled sound list, and alarm and
  timer lists that scroll under their ring up to 12 rows. Its five follow-ups are filed
  under issue 143.
