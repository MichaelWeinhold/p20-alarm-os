# The low-battery light

**State.** `alarmd` blinks `/sys/class/leds/red` through the `indicator` adapter while the
battery reads at or below 20 % off the cable; nothing on the phone is hand-made for it.
**It is host-tested only**: the red LED has not been read on the phone under our `/init`.
→ ADR 0013, issue 76.

- [ ] Read the red LED on the phone, issue 76's *Done when*, once the battery is at or
      below 20 %:
  1. Put `alarmd` built from `main` on `system` as `/init`, as in [phone.md](phone.md),
     and keep its sha256.
  2. Pull the cable and let the panel go off: the red LED blinks.
  3. Put the cable in: the LED stops.
  4. Record both readings, with the `alarmd` hash, in `docs/device-history/phone.md`;
     then delete this box and say above what is now true.

⚠️ `alarmd` logs `opening the indicator` when the LED is absent at start-up and `driving the
low-battery warning` when a write fails, such as a kernel without the `timer` trigger.

**Undo.** Nothing to undo: a clear writes `brightness` 0, and the power-off turns the LED
off on its own.

The readings: [docs/device-history/phone.md](../device-history/phone.md), issue 76.
