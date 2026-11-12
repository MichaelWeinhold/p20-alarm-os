# 2. Boot the vendor kernel, and replace everything above it with one program

- **Status:** Accepted; amended by ADR 0006
- **Date:** 2026-11-11
- **Recorded with:** the bootstrap, issue 1. Which kernel build exactly is open, and is
  its own research issue.

## Context

The goal is a phone that does one thing: show the time, ring an alarm, and otherwise sleep.
The Huawei P20 Lite (`anne`) runs a HiSilicon Kirin 659, for which there is essentially no
public hardware documentation. The display panel, touchscreen, power-management IC,
charger, RTC and audio each need a driver.

Huawei publishes the kernel source for the phone under the GPL, and the image of the ROM
already on the phone carries a kernel that drives all of that hardware.

## Decision

**Keep the vendor kernel and replace the userspace.** The image is the existing kernel,
which runs one static Rust binary, `alarmd`, as `/init`. It is not an initramfs: the
bootloader loads no ramdisk from `kernel`, so `/init` sits on `system` (ADR 0006). It:

- draws the clock to the framebuffer or DRM device,
- reads touch and buttons from `/dev/input/event*`,
- arms the wake alarm on `/dev/rtc0` and suspends with `echo mem > /sys/power/state`,
- rings through the vibrator, and later perhaps the speaker.

Only `system` is written — the phone has no `boot`, and `kernel` stays stock — so
recovery and fastboot stay intact, and the `system` backup restores the phone.

## Rejected alternatives

**An operating system from scratch, on bare metal.** Every driver would be reverse
engineered from an undocumented SoC — months to years before a pixel is lit. *Reopens if*
a mainline-Linux port for the Kirin 659 appears that covers display, touch, RTC and
suspend, which would make the vendor kernel the thing to replace instead.

**Stripped Android with a kiosk clock app.** Easy, and it works, but it keeps the Android
userspace whose removal is the point, and it is not a smaller or more understandable
system. *Reopens if* deep sleep or wake from the RTC proves impossible without Android's
userspace pieces, which the vendor kernel may expect.

**postmarketOS as the base.** It would bring a maintained userspace and tooling. It is not
rejected outright: whether a `huawei-anne` port exists, and what its kernel and notes
give, is the first research issue. *Reopens* the choice of kernel if the port has a better
one; the single-program userspace stands either way.

## Consequences

- **Audio is the hardest part.** Kirin audio runs through a DSP that needs firmware and
  mixer routing which Android's audio HAL normally sets up. The vibrator comes first; the
  speaker is a separate, parked research issue.
- **There is no serial console.** A boot that fails is a black screen, so the second
  step is a debug `/init` with SSH over USB gadget networking, or `ramoops`.
- **Waking from the RTC must be proven early** — the alarm depends on it — and the battery
  gain over stock Android in airplane mode may be small.
