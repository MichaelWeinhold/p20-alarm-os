# 13. Blink the LED red while suspended on a low battery

- **Status:** Accepted
- **Date:** 2026-11-20
- **Issue:** #64

## Context

The panel marks a low battery only while it is lit (ADR 0008, D3). Issue 64 asks whether
the notification LED beside the earpiece can warn of one while the panel is off. Under
ADR 0010, D2 the phone is then suspended for two minutes and powered off after them. The
readings are in `docs/device-history/phone.md`, *the notification LED*; the source is
`drivers/leds/hisi_leds.c` in `Abdelhay-Ali/android_kernel_huawei_hi6250_KernelSU`.

| Question | Answer |
|---|---|
| 1. Which node, which colours? | **`/sys/class/leds/red`, `green` and `blue`**, on the PMU, each with three current steps: off, 1 and the colour's maximum |
| 2. What does it cost asleep? | **Red at its maximum, 4.9 mA**, against an unlit suspend of 4.3 mA. At current 1, 1.3 mA steady; blinking, under the 1.5 mA the readings resolve |
| 3. Does it stay lit through a suspend, and hold a vote? | **It stays lit and holds none**: `modem:0` and one `system sleeped` per suspend |
| 4. Does anything keep it lit while off? | **No**: the driver's `shutdown` turns all three off, and the operator saw it go dark |
| 5. Where does the threshold come from? | **`alarm_core::battery::LOW_MARK`, 20 %**, the mark ADR 0008, D3 draws |
| 6. Is the warning worth changing the power model for? | **No.** It shows while the phone is suspended, whatever keeps it suspended |

## Decisions

### D1. The warning is red at current 1, blinking from the PMU

The PMU blinks the LED itself, with no wake of the AP, from one timing register the three
colours share. A write to `brightness` sets that register back to steady, and through
sysfs only the last of `delay_on` and `delay_off` written survives. The adapter writes
`trigger` `timer`, then `brightness` 1, then `delay_off` 2000: on code 0, a flash of
about half a second, and off code 3, about 2 s. Turning it off is `brightness` 0.

Current 1 is the lowest that lights it, and was clearly visible in room light. Steady it
costs about 1.3 mA; blinking with off code 3 it is estimated at 0.3 mA from step 8's duty,
not read. Red reads as a warning; green's maximum is current 1, no brighter than red's
lowest.

### D2. It blinks while the battery is at or below `LOW_MARK`, off the cable

`alarmd` sets it when the battery reads at or below 20 % off the cable, and clears it
otherwise; on the cable the phone is charging and needs no warning. One
threshold, the one ADR 0008, D3 already draws, not a second.

### D3. The power model does not change for it

Under ADR 0010, D2 the LED shows during the two-minute window, and the power-off turns it
off. Staying suspended under 20 % instead would cost about 4.9 mA against the 1.2 mA off
draw: about 4 days from 20 % to empty instead of about 16, and an alarm missed sooner if
the warning goes unseen. The light rides on whatever keeps the phone suspended. Once ADR
0010's first rejected alternative is taken in its own ADR, since the suspend under
`alarmd` now reads 4.6 mA, it blinks the whole time the battery is low, at about 7 % over
the suspend.

### D4. The LED is a device behind a trait of its own

Architecture rule 2: an `Indicator` trait in `alarm_core::hardware` and one adapter that
alone opens `/sys/class/leds`. A write that fails returns `HardwareError` (rule 3); the
rule that decides it lives in `alarm-core` and is tested with a fake.

## Rejected alternatives

**Staying suspended instead of powering off below 20 %**, so the light keeps warning
under ADR 0010, D2. The warning would be seen; the remaining charge would last about a
quarter as long. *Reopens if* a low battery is found to go unnoticed until an alarm is
missed, or the suspend draw falls near the 1.2 mA off draw.

**Red at its maximum, steady.** Seen from further away, at 4.9 mA, more than the suspend
itself. *Reopens if* current 1 is found too dim to notice where the phone stands.

**Blinking at the longest off code.** About 0.1 mA, but a flash more than 20 s apart is
easy to miss. *Reopens if* the 2 s blink is read to cost more than about 0.5 mA.

**No LED, the panel's mark alone.** Every glance already shows it, and it costs nothing.
*Reopens if* the LED is needed for something else, such as a snoozed alarm.

**A software blink**, a timer in `alarmd` toggling `brightness`. It would wake the SoC at
every flash and lose the PMU's blink. *Reopens if* the PMU's timing register is found
unusable.

## Consequences

- **Issue 76, under issue 64**: the `Indicator` trait, its adapter, and the rule
  in `alarm-core` (D1, D2, D4).
- **Under ADR 0010, D2 the warning is short**: two minutes after each use. It is worth
  most once suspending whenever the phone is not in use is taken.
- `docs/device-history/phone.md` records the sysfs behaviour the adapter depends on: the
  shared timing register, and the last delay written surviving alone.
