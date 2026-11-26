# 11. Keep the settings in a raw record on `reserved4`

- **Status:** Accepted; D2's format raised to version 2 by issue 83, to version 3 by
  issue 81, to version 4 by issue 97, to version 5 by issue 110, to version 6 by
  issue 115, to version 7 by issue 128, to version 8 by issue 86, and to version 9 by
  issue 135
- **Date:** 2026-11-13
- **Issue:** #30

## Context

P7 sets alarms, the clock's `UtcOffset` and the timer on the phone, and ADR 0010, D2
powers the phone off two minutes after every use. The settings must therefore survive a
power-off many times a day, a reboot, and an empty battery. Until they do, ADR 0010, D4
keeps the phone suspended.

Our `/init` is a file on `system`, which the kernel mounts read-only (ADR 0006, D4). The
readings, from TWRP on 2026-11-13, are in `docs/device-history/phone.md`, *where the
settings can live*:

- **Eight partitions hold nothing but zeros**: `reserved2` (46 MiB), `reserved3`,
  `reserved4`, `reserved5`, `reserved9`, `hisitest1`, `hisitest2` and `persist`, after
  the ROM's use. `hisitest0` beside them holds a log of screen versions.
- **`userdata` is not encrypted.** TWRP mounts it as ext4 with no password, and the
  package names under `/data/data` read in clear. The assumption of issue 30 is disproved.
- **`cache`** is ext4, and TWRP writes `/cache/recovery` on every boot into recovery.
- **A record written to `reserved4` came through** `adb reboot recovery`, the bootloader
  and TWRP byte for byte, and `reserved5` beside it did not change.

Writes are rare: a change made on the settings screen, and a one-shot alarm
(`Alarm::once`) removed after it rings. A ringing or snoozed alarm keeps the phone
suspended (ADR 0010, D2), so neither needs a write.

## Decisions

### D1. The settings live on `reserved4`, raw, with no filesystem

`reserved4` is `mmcblk0p14`, 2097152 bytes. Nothing reads it but us: it held zeros under
the ROM, and neither the bootloader nor TWRP touched a record on it. A raw record needs
no mount, no journal replay and no `fsck`, and its power cut is D3's to handle.

The adapter finds the device by `PARTNAME=reserved4` in `/sys/class/block/*/uevent`, the
parse `crates/input` already does for `odm`, which moves to a common module (architecture
rule 6). A partition of any other size is `HardwareError`, so a phone with another
partition table is never written.

`reserved5` (`mmcblk0p15`, the same size, also zero) is the fallback, and is backed up
beside `reserved4` under `~/p20-backups/`.

### D2. The record is two 4 KiB slots, each with a sequence number and a CRC

The record is the partition's first 8192 bytes: slot 0 at offset 0, slot 1 at 4096. A
slot is little-endian:

| Offset | Bytes | Field |
|---|---|---|
| 0 | 8 | magic, `P20ALRM\0` |
| 8 | 4 | format version, `9` |
| 12 | 4 | payload length, at most 4068 |
| 16 | 8 | sequence number |
| 24 | n | payload |
| 24 + n | 4 | CRC-32 (IEEE) of bytes 0 to 24 + n |

The rest of the slot is zero. The payload is the settings, encoded in fixed-width
little-endian fields: the alarms, the `UtcOffset` and ADR 0010, D2's window. Its layout is
part of the format version, and a change to it raises the version.

| Payload offset | Bytes | Field |
|---|---|---|
| 0 | 4 | `UtcOffset` in seconds, signed |
| 4 | 4 | the window in seconds |
| 8 | 1 | ADR 0010, D2's power-off toggle, `1` on and `0` off; absent in version 1 |
| 9 | 1 | the number of alarms, n |
| 10 | 4 n | each alarm: hour, minute, weekdays, and `1` armed or `0` not |
| 10 + 4 n | 1 | the number of saved timers, m; absent before version 3 |
| 11 + 4 n | 3 m | each timer: hours `0` to `23`, minutes `0` to `59` and seconds `0` to `59`, never all `0`; before version 7, 2 m bytes of minutes `0` to `99` and seconds |
| 11 + 4 n + 3 m | 2 | how long a press lights the panel, in seconds: `10`, `30`, `60`, `120` or `300`; absent before version 4 |
| 13 + 4 n + 3 m | 1 | the ring's volume, `1` to `15`; absent before version 5 |
| 14 + 4 n + 3 m | 1 | the ring's sound, `1` on and `0` off; absent before version 6 |
| 15 + 4 n + 3 m | 1 | the ring's vibration, `1` on and `0` off; absent before version 6 |
| 16 + 4 n + 3 m | 1 + k | the chosen sound's name: its length k, then its bytes, ADR 0014's rule; absent before version 8 |
| 17 + 4 n + 3 m + k | 2 | how long a ring ramps up, in seconds: `0` off, or `15`, `30`, `60`, `120` or `300`; absent before version 9 |

A version-1 slot still loads, with the toggle on, and the next save writes the current
version. Version 1 is the same payload without the toggle byte. Amended by issue 83.
A version-2 slot loads with no saved timers; version 2 is the payload without the timer
count and entries. A running timer is not kept. Amended by issue 81.
A version-3 slot loads lit for 10 s; version 3 is the payload without the lit time.
Amended by issue 97.
A version-4 slot loads at the loudest volume, `15`; version 4 is the payload without the
volume. Amended by issue 110.
A version-5 slot loads with the sound and the vibration on; version 5 is the payload
without either. Amended by issue 115.
A version-3 to version-6 slot loads each timer's minutes and seconds as the same length
in hours, minutes and seconds, `75:00` as `1:15:00`; those versions store a timer in two
bytes, minutes `0` to `99` and seconds. Amended by issue 128.
A version-8 slot loads with the ramp off, so an upgrade rings at once at its volume as
it did; version 8 is the payload without the ramp length. A ramped ring starts at volume
`1` and steps up to the saved volume over that length. Amended by issue 135.

Encoding, decoding and choosing a slot live in `alarm-core`, tested on the host with
fakes (architecture rule 1). The adapter only reads and writes the 8192 bytes. The CRC is
written in `alarm-core`, with no crate.

### D3. A save writes the older slot, flushes, and reads it back

A save writes the slot that does not hold the newest valid record, with the sequence
number one higher, then calls `fsync` on the block device, which flushes the eMMC's
cache, and reads the slot back. A mismatch is `HardwareError`. A power cut in the middle
of a save therefore leaves the other slot whole.

A load reads both slots and takes the valid one with the higher sequence number. A slot
is valid when its magic, version, length and CRC all hold.

- **Both slots all zero** means never written: no alarms, and the defaults of ADR 0010.
  This is a known state of the partition, not a guessed default (architecture rule 3).
- **No valid slot, and either not all zero** is `HardwareError`. No alarm is armed, and the
  screen says so. The next save writes slot 0 over it.

### D4. The rollback to stock writes the backup back

`~/p20-backups/reserved4-mmcblk0p14-stock.img`, all zeros, goes back with `dd` from TWRP.
The ROM never read the partition, so even a record left on it costs the ROM nothing.
`fastboot` and everything before it are not touched, and `reserved4` is not on ADR 0005,
D4's hard-brick list. ADR 0006, D4's read-only `system` stands.

## Rejected alternatives

**A file on `system`, remounted read-write for each save.** It needs no partition of its
own, but a power cut during the remount can corrupt the root, and the phone then boots
nothing of ours. It amends ADR 0006, D4 for a few hundred bytes. *Reopens if* `system`
becomes a filesystem of ours alone (ADR 0006, D4) with a writable area built in.

**A file on `userdata`.** It is not encrypted, but it is a 25 GiB ext4 shared with the
ROM's data: every boot mounts it and may replay its journal, a power cut leaves our
`/init` an `fsck` to run that it does not have, and a factory reset from TWRP wipes it.
*Reopens if* the settings outgrow a slot, or need files such as alarm sounds.

**A file on `cache`.** TWRP writes it on every boot into recovery, and its wipe empties
it; it is a filesystem to mount, like `userdata`. *Reopens with* `userdata`, if a
filesystem is needed and `userdata` is not wanted.

**Another all-zero partition.** `reserved2`, `reserved3` and `reserved9` are larger than
needed; `persist` carries a name another vendor's software might use; `hisitest1` and
`hisitest2` sit beside `hisitest0`, which a test tool writes. *Reopens if* something
other than us is found to change `reserved4`; `reserved5` is first.

**A filesystem of ours on a spare partition.** It needs a mount, a journal and an `fsck`
for less than 4 KiB. *Reopens if* the settings outgrow a slot.

**A new partition for the settings.** It rewrites `ptable`, on ADR 0005, D4's hard-brick
list. *Reopens* never.

**One slot, rewritten in place.** A power cut in the middle of a save loses the only
copy. *Reopens* never.

## Consequences

- **ADR 0010, D4's condition is met** once the storage adapter lands: the power-off of
  issue 52 can follow it.
- **The storage adapter is a follow-up under this issue**: the `Settings` trait in
  `alarm_core::hardware`, the codec and the slot choice in `alarm-core`, and a `storage`
  crate that owns `reserved4`. It moves the `PARTNAME` parse into a common module, and
  adds the crate to `docs/ARCHITECTURE.md`.
- **A write to `reserved4` is a write by our image**, beside ADR 0006, D3's `system`. It
  happens only from `alarmd`, never from a host script.
- **The eMMC's own wear levelling is assumed** to spread the rewrites of two 4 KiB slots.
  At a few saves a day that is not read.
