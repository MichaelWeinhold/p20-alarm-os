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
pstore. → ADR 0009, issue 9.

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

**Done** — issue 11. The alarm is set by touch, rings on the panel, a volume key snoozes it
and Power dismisses it.

## P5 — Wake from suspend

Arm `/dev/rtc0`, suspend, and wake on time. The whole project depends on this, so it is
proven as early as P2 allows, even before the clock looks right.

**Done when:** an alarm armed for five minutes ahead wakes a suspended phone, the draw
while suspended is read with USB input cut, and both readings are in
`docs/device-history/`. → ADR 0010, which supersedes ADR 0008, D1.

**Proven** — issue 12. The RTC woke a suspend to the second, a suspend draws about 53 mA,
and a power-on alarm switched a powered-off phone on. The adapters, the two-minute window
and the power-off are issue 12's follow-ups.

## P6 — Vibrate

Ring the alarm through the vibrator.

## Research before P7

- **Where settings persist across a reboot.** Issue 30. The initramfs lives in RAM, so an
  alarm set on the phone needs a spare partition on the eMMC that the ROM can do without.

## P7 — Settings and a timer

Issue 31. A settings screen for the alarms, the clock and the UTC offset, and a countdown
timer armed through the RTC like an alarm.

**Done when:** an alarm and the clock set on the phone survive a reboot, and a timer wakes
a suspended phone when it ends.

## Parked

- **Speaker audio without Android** — the Kirin DSP, its firmware and mixer routing.
  Labelled `future`; picked up once P6 is in daily use.
