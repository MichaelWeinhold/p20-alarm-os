# 7. Hold the charge below full, and leave charging itself to the kernel

- **Status:** Accepted
- **Date:** 2026-11-12
- **Issue:** #27

## Context

The battery is old: its gauge has learned 2283 of 3000 mAh, 76 %. A bedside clock spends
most of its life on the cable, and lithium cells age fastest when they are held full and
warm. ADR 0002 counts the charger among the drivers the vendor kernel brings, but nobody
had checked what it does with no Android above it.

Issue 27 read it three ways: from the ROM, from TWRP's root shell over the same kernel,
and from a probe of ours as `/init` on `system`. The readings are in
`docs/device-history/phone.md`, *the battery, the charger and the panel*.

- **The kernel charges with our binary as PID 1**, and applies the battery's limits
  itself. They come from the device tree: 4400 mV, no charge below 0 °C or above 50 °C,
  reduced current below 10 °C, and 4100 mV between 45 and 50 °C. It also lowers the
  termination voltage as the battery ages.
- **At 100 % the charger never terminates.** The battery takes 110–130 mA at 4.385 V,
  above the 100 mA termination current, so on the cable it is held full indefinitely.
- **`hw_power/charger/charge_data/enable_charger` 0 stops charging, and the battery then
  rests.** The phone runs from USB, `current_now` reads 0, and the coulomb counter stops.
  1 restarts it within 5 s. Every plug-in clears the 0 again
  (`charge_start_charging` in `huawei_charger.c`).
- **Under our `/init` the phone draws about 350 mA**, against 38 mA for the ROM asleep
  with the panel off, and 166–193 mA for TWRP with its panel timed out. The CPU governor,
  taking cores offline, `lcd_backlight0` and `fb0/blank` each moved it by less than the
  ±5 mA noise. `lcd_backlight0` reads 4000, the bootloader's level.
- **From the ROM, the panel costs about 92 mA lit at backlight 0**, plus about 27 mA per
  1000 backlight units, up to 351 mA in all at the maximum, 8081.

## Decisions

### D1. Charging stays the kernel's: `alarmd` neither starts it nor regulates it

Answers question 1. The vendor driver charges, enforces the temperature table and the
voltage limit, and ages the termination voltage, all with no userspace. `alarmd` adds no
charging logic of its own and never writes current or voltage limits. If the kernel is
ever replaced, this decision reopens (ADR 0004).

### D2. `alarmd` holds the charge between 75 and 80 % with `enable_charger`

Answers question 2. It is the one cheap change that slows the battery's ageing, since the
clock is on the cable most of the time:

- at or above 80 %, `alarmd` writes 0 to `enable_charger`, and the battery rests while USB
  powers the phone;
- at or below 75 %, it writes 1;
- it reads the node back on every pass, and writes again whenever a plug-in has reset it;
- off the cable it writes nothing.

The band's numbers are this decision's defaults. Changing them needs a setting, not a new
ADR.

The cost is the gauge's calibration. With no full charge, the gauge stops relearning its
capacity, and `capacity` drifts. `alarmd` therefore lets the charge reach 100 % once every
30 days and then resumes the hold.

### D3. `alarmd` reads the battery through one adapter, and trusts four of its numbers

Answers question 3. The adapter reads `/sys/class/power_supply/Battery` and
`/sys/class/hw_power/charger/charge_data`:

| Node | Unit | Trusted for |
|---|---|---|
| `capacity` | % | the hold's band |
| `status` | `Charging`, `Full`, `Not charging`, `Discharging` | whether the cable is in |
| `voltage_now` | µV | a check on `capacity` |
| `temp` | 0.1 °C | logged; the kernel acts on it |
| `current_now` | mA, positive flowing in | logged only |
| `health` | `Good`, … | logged only |

`cycle_count` is not trusted: it reads 7. Per architecture rule 3, a node that is absent or
reads anything unexpected is a `HardwareError`, and then the hold leaves `enable_charger`
at 1. A clock that cannot read its battery charges it to full rather than not at all.

### D4. On the cable the time stays on screen, dimmed at night; off the cable the panel sleeps

Answers question 4. On the cable, D2 keeps the battery resting, so the panel costs mains
power, not battery wear. The clock shows the time all the time, with a low backlight at
night: about 200 of 8081, about 11 mA over the panel. Off the cable, after a power cut,
the panel's 92 mA and more is most of the phone's draw. The panel goes off, and comes on
for a touch or a button.

Under our `/init`, writing the backlight and blanking `fb0` did not reach the panel the
bootloader lit. Taking the panel over is therefore part of the display adapter, issue 10,
before either works. **That the panel stayed lit under the probe is inferred** from the
draw and the backlight reading, not seen.

### D5. The radios need no code: nothing loads their firmware

Answers question 5. Under our `/init` the modem reads `MODEM_STATE_OFF`, and
`/sys/class/net` holds no `wlan0`, only `lo`, tunnels and the modem's `rmnet*`. The modem
starts only when userspace writes `modem_sysboot_start`; Wi-Fi, Bluetooth and GNSS share
one chip that a vendor daemon starts. `alarmd` starts none of them. **That the Wi-Fi chip
draws nothing while unloaded is inferred**, from the missing interface.

### D6. The CPU is left as the kernel sets it; the saving that matters is the panel and suspend

Answers question 6. Measured under our `/init`, `powersave` and cores offline saved
nothing measurable. In TWRP, the same changes saved 17–27 mA of 193, because the idle
states already park idle cores. Of the 350 mA, the larger share fits the panel, which is
D4's. What remains is suspend between alarms, issue 12; `charge_wakelock` keeps the phone
awake on the cable anyway, unless the charge completes from a wall charger. Nothing else
cheap turned up.

## Rejected alternatives

**Capping the voltage with `regulation_voltage`** (3200–4400 mV). The charger would stop
at, say, 4.1 V, but on charge done the gauge sets `capacity` to 100 and relearns its full
capacity there, so its numbers would no longer mean anything. *Reopens if* a kernel
change lets the gauge keep its scale under a voltage cap.

**`enable_hiz` as the hold.** It cuts the USB input, so the battery powers the phone
while plugged in, and a clock on the cable would cycle its battery. *Reopens if* the USB
input ever has to be cut for another reason, such as a charger that misbehaves.

**`shutdown_q4`, the battery FET switched off.** It disconnects the battery, and the
clock dies with the power. *Reopens* never.

**A mains timer plug**, which needs no code. It cycles the battery once a day, and a
power cut at the wrong time leaves the battery low before the alarm. *Reopens if* D2's
node stops working, or if the kernel is replaced by one with no such node.

**A lower charge current through the thermal limits** (`iin_thermal`, `ichg_thermal`).
The charge still ends full and is held there. *Reopens if* the battery runs warm while
charging, which the 24–27 °C read here does not show.

**Switching the CPU governor or taking cores offline in `alarmd`.** It measured nothing
under our `/init`. *Reopens if* `alarmd` ever keeps the CPU busy, which a clock that
suspends should not.

**The time always on screen, on battery too.** The panel alone would drain 2283 mAh in
under a day. *Reopens if* issue 12 shows that suspend does not work, so the phone is
awake anyway.

## Consequences

- **A battery and charger adapter, with the hold in `alarm-core`**, is a follow-up under
  this issue. The rule is tested on the host with a fake, and the adapter writes only
  `enable_charger`.
- **The display adapter takes the panel over from the bootloader** before it sets the
  backlight or blanks it. It is a follow-up under this issue, beside issue 10.
- **Issue 12's suspend reading is taken off the cable**, since the charger's wake lock
  keeps a phone on the Mac's port awake.
- **`crates/init-probe` logs the power nodes** and strips control characters, since the
  kernel log ends a record at a NUL.
- **A swollen battery is replaced**, not managed. Nobody has looked at this one yet.
