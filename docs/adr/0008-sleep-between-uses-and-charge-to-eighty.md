# 8. Sleep between uses, light the panel only when used, and stop a charge at 80 %

- **Status:** Accepted; D1 superseded by ADR 0010
- **Date:** 2026-11-12
- **Issue:** #39
- **Supersedes:** ADR 0007, D2 and D4; amends ADR 0007, D6

## Context

ADR 0007 assumed a bedside clock that spends most of its life on the cable, so the
battery's wear came from being held full, and the panel cost mains power. The operator
runs it on its battery instead, and charges it now and then. The question becomes how
long a charge lasts, and how a charge spares the old cell. Issue 39 asks it.

The readings are ADR 0007's, in `docs/device-history/phone.md`, *the battery, the charger
and the panel*. Of 2283 mAh:

| State | Draw | A charge lasts |
|---|---|---|
| our `/init` today, panel lit | about 350 mA, measured | about 6.5 hours |
| the ROM asleep, panel off | about 38 mA, measured | about 2.5 days |
| suspended | not measured | — |

`docs/ARCHITECTURE.md`'s loop already ends each pass with the panel off and the phone
suspended, woken by the RTC or the power button. This ADR makes that loop the power plan.

## Decisions

### D1. The phone is suspended whenever it is not in use

Amends ADR 0007, D6. Between uses the panel is off and the phone suspended; it wakes for
the RTC alarm or the side button, and nothing else. Every other saving is small beside
this one, so the suspend draw decides how often the phone is charged. Issue 12 measures
it, with USB input cut, and checks whether the side button wakes a suspended phone.
**That the suspend draw is well under the 38 mA of the sleeping ROM is assumed**; it is
unmeasured.

### D2. The panel is lit only while the clock is used, on the cable or off it

Supersedes ADR 0007, D4. A press of the side button lights the panel with the time for
10 s, at a backlight level that is a setting; a touch in that time keeps it lit, and
the alarm lights it while it rings. Then the panel goes off and the phone suspends. The
cable changes nothing. The 10 s and the level are defaults, changed by a setting rather
than by an ADR.

Under our `/init` the backlight and `fb0` blank did not reach the panel, so the display
adapter takes the panel over first (issue 37, ADR 0007's consequence, unchanged).

### D3. A charge stops at 80 %, and `alarmd` warns at 20 %

Supersedes ADR 0007, D2. While the cable is in, `alarmd` lets the kernel charge until
`capacity` reaches the limit, then writes 0 to `enable_charger`; the phone then runs from
USB and the battery rests until the cable comes out. A plug-in clears the 0, so
`alarmd` reads the node back on every wake and writes it again.

- The limit is 80 % by default, a setting from 50 to 100 %. 100 % is for a trip, and it
  also lets the gauge relearn its capacity, which a charge that never ends full stops.
- At or below 20 %, the time drawn carries a battery mark, and the panel shows it when
  lit.
- On a `HardwareError` from the battery or the charger, `alarmd` leaves `enable_charger`
  at 1 (ADR 0007, D3, unchanged).

Charging between 20 and 80 % keeps the cell away from both ends, where it ages fastest,
at the cost of 40 % of each charge.

### D4. What ADR 0007 leaves standing

ADR 0007, D1, D3 and D5 stand: the kernel charges and applies the battery's limits, the
battery is read through one adapter that trusts four of its numbers, and the radios
need no code. So does D6's finding that the CPU governor and cores offline save nothing
measurable.

## Rejected alternatives

**The time always on screen, dimmed at night**, ADR 0007, D4's plan for the cable. It
costs about 100 mA dimmed, which off the cable empties a charge in under a day. *Reopens
if* the operator keeps the phone on the cable again, or a panel mode that shows the time
at a few mA turns up.

**Holding the charge between 75 and 80 % while plugged in**, ADR 0007, D2. It answers a
phone that lives on the cable; one charged now and then needs only the stop. *Reopens
with* the alternative above.

**Powering the phone off between uses, and switching it on from the RTC.** Off, it would
draw almost nothing, but a press of the side button would boot the phone before showing
the time. Whether the PMU can power the phone on from its RTC is unknown. *Reopens if*
issue 12 finds that the suspend draw empties a charge in less than a week, and that the
RTC can switch the phone on.

**Charging to 100 % every time.** It gives 40 % more time between charges, and the cell
ages faster at both ends. *Reopens* never as the default; the setting reaches 100 %.

## Consequences

- **Issue 36 changes**: a charge stops at the limit when the cable is in, with a setting
  for the limit and the 20 % mark, instead of holding a band.
- **Issue 37 changes**: the display adapter lights the panel on a press and turns it off
  after 10 s, on the cable or off it.
- **Issue 12 grows**: it measures the suspend draw with USB input cut, checks the side
  button as a wake source, and checks whether the RTC can power the phone on.
- **ADR 0007's consequence for issue 12 stands**: its reading is taken off the cable,
  since the charger's wake lock keeps a plugged-in phone awake.
