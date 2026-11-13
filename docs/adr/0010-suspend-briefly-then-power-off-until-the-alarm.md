# 10. Suspend for two minutes after a use, then power off until the alarm

- **Status:** Accepted
- **Date:** 2026-11-13
- **Issue:** #12
- **Supersedes:** ADR 0008, D1

## Context

The whole project depends on the alarm waking the phone. ADR 0008, D1 suspends the phone
whenever it is not in use, and assumes the suspend draw is well under the 38 mA of the
sleeping ROM. Its rejected alternative, powering off between uses, reopens if a suspend
empties a charge in under a week and the RTC can switch the phone on. Issue 12 asks the
five questions below. The readings are in `docs/device-history/phone.md`, *suspend, wake
and power-on from the RTC*, all taken under the debug `/init` with the cable out.

| Question | Answer |
|---|---|
| 1. Does the RTC wake a suspended phone on time? | **Yes**, to the second, as `RTC0`, with no Android userspace |
| 2. What does it draw suspended? | **About 53 mA** with the panel off, the gauge's own average; about 336 mA with it lit |
| 3. Do the side button and a plug-in wake it? | **Both do**: the power key on `GPIO23`, a plug-in on `GPIO29` |
| 4. Can the RTC switch on a powered-off phone? | **Yes**, through a `CLOCK_POWEROFF_ALARM` timer; not through `wakealarm` |
| 5. Which Android pieces are missing? | **None that a suspend needs.** The SoC never reaches deep sleep: the AP and the modem hold votes, owners not found |

At 53 mA, a full charge of 2283 mAh lasts about 43 hours, and ADR 0008, D3's band from
20 to 80 % about 26: a charge every day, where ADR 0008 wants one now and then. Both
conditions of the rejected alternative hold.

## Decisions

### D1. The RTC wakes a suspend, through `wakealarm` and `/sys/power/state`

Arming is `echo 0`, then `echo <epoch>` to `/sys/class/rtc/rtc0/wakealarm`; suspending is
`echo mem > /sys/power/state`, which returns after the wake. These are the `WakeAlarm`
and `Suspend` adapters of `docs/ARCHITECTURE.md`. `charge_wakelock` does not stop a
direct `echo mem`, so the cable needs no special case to suspend.

### D2. After a use, the phone suspends for two minutes, then powers off

Supersedes ADR 0008, D1. When the panel goes off (ADR 0008, D2), the phone suspends, and
a two-minute window starts. A press of the side button in the window lights the time at
once and ends the window; the next one starts when the panel goes off again. The window
does not run while the panel is lit. When it ends without a press, the phone powers off.
The two minutes are a default, changed by a setting.

The phone stays suspended instead of powering off:

- **while the cable is in**, since ADR 0008, D3's charge stop needs `alarmd` running, and
  what a phone powered off on the cable does is not read;
- **while an alarm rings or is snoozed, or is due within the window.** The RTC wakes the
  suspend for it, and the window starts again after it.

### D3. A power-on alarm is armed before every power-off

`alarmd` creates a `timerfd` of `CLOCK_POWEROFF_ALARM` (12), sets it to the next alarm in
UTC with `TFD_TIMER_ABSTIME`, then powers off with `reboot(2)`, as PID 1. The kernel arms
the PMU 60 s before the target, which covers the boot; for a target less than 60 s away
it arms 15 s after the call. With no alarm armed, it powers off with none.

`wakealarm` is never what powers the phone on: `hisi_rtc_shutdown` clears the PMU's alarm
unless the power-off alarm is enabled.

### D4. The boot reads why it started

`/proc/cmdline` carries `normal_reset_type=BR_POWEROFF_ALARM` on a boot by the alarm. Then
`alarmd` waits for the alarm's minute and rings. On any other boot it shows the time, as
for a press. Either way it needs the alarms from before the power-off, so D2's power-off
comes only once the settings persist across a reboot (issue 30). Until then the phone
stays suspended, as ADR 0008, D1 had it.

### D5. Deep sleep is not needed, and ADR 0002's kiosk stays rejected

A suspend and a wake from the RTC both work without Android's userspace, so ADR 0002's
reopening condition for the Android kiosk is not met. The suspend is shallow, which is
why it draws 53 mA. A follow-up looks for the AP's and the modem's votes; it is not
needed for D2.

## Rejected alternatives

**Suspending whenever the phone is not in use**, ADR 0008, D1. A press always shows the
time at once, at a charge a day. *Reopens if* the deep-sleep follow-up brings the
suspend draw under about 8 mA, a week on the band from 20 to 80 %.

**Powering off as soon as the panel goes off.** It saves the window's few mAh, and turns
every glance into a cold boot: the debug `/init` started 12.47 s in on the kernel's clock,
after bootloader stages the command line puts at 13.3 s, with the unlocked bootloader's
warning on screen. *Reopens if* a boot to the time drawn takes under 3 s.

**Powering off on the cable.** It would save nothing the cable does not supply, and leave
the charge stop without `alarmd`. *Reopens if* a power-off on the cable is read to stay
off without charging past the limit.

**Arming `wakealarm` before a power-off.** Read not to switch the phone on, and the
kernel's source says why. *Reopens if* the kernel changes.

**Stripped Android with a kiosk clock app**, ADR 0002. *Reopens if* the deep-sleep
follow-up finds deep sleep needs Android's userspace, and power-off has failed in use.

## Consequences

- **ADR 0008, D1 is superseded**; its D2 and D3 stand. A phone off the cable charges far
  less often, with the off draw still to be read.
- **Opening `/dev/fb0` after a suspend panicked the kernel**, so `alarmd` holds it across
  every suspend, as it already holds it for life (issue 10); whether a held `fb0` lights
  again after a resume is issue 37's to read.
- **The debug `/init` reboots into TWRP when its shell reads an end of file**, which an
  unplugged cable can cause; a follow-up.
- Follow-up issues under issue 12: the `WakeAlarm` and `Suspend` adapters (D1); the
  window in `alarm-core` (D2); the power-off with its power-on alarm, and the boot reason
  (D3, D4), after issue 30; the off draw, a power-off on the cable, and how long the side
  button must be held; the deep-sleep votes and `GPIO28` (D5); the debug `/init`'s shell
  on an end of file.
