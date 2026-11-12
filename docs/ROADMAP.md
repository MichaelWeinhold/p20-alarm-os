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
  writes the stock image back. It leaves a `flashing-the-phone` skill and a device
  preflight to build.

## P1 — Boot image round trip

Unpack the current `boot.img` and repack it unchanged, byte for byte.

**Done when:** the repacked image equals the original, by test, and boots the ROM as
before.

## P2 — Debug lifeline

A minimal initramfs that brings up USB gadget networking and SSH, or `ramoops`.

**Done when:** the laptop reaches a shell on the phone over USB, and a failed boot leaves a
log that can be read.

## P3 — Draw

`alarmd` as `/init` draws the time on the panel.

## P4 — Touch and buttons

Read `/dev/input/event*`; set an alarm and dismiss it.

## P5 — Wake from suspend

Arm `/dev/rtc0`, suspend, and wake on time. The whole project depends on this, so it is
proven as early as P2 allows, even before the clock looks right.

**Done when:** an alarm armed for five minutes ahead wakes a suspended phone, and the
reading is in `docs/device-history/`.

## P6 — Vibrate

Ring the alarm through the vibrator.

## Parked

- **Speaker audio without Android** — the Kirin DSP, its firmware and mixer routing.
  Labelled `future`; picked up once P6 is in daily use.
