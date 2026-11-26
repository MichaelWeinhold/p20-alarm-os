# 14. Keep alarm sounds on `system` and the settings on `reserved4`

- **Status:** Accepted; D2 amended by ADR 0015
- **Date:** 2026-11-21
- **Issue:** #82

## Context

ADR 0011 keeps the settings in a raw record of two 4 KiB slots on `reserved4`, with no
filesystem. It rejected a file on `userdata` with *"Reopens if the settings outgrow a
slot, or need files such as alarm sounds"*, and a filesystem of ours on a spare partition
with *"Reopens if the settings outgrow a slot."* Issue 31 asks for alarm sounds kept on
the phone, and its children #80, #81 and #83 add saved alarms, saved timers and a
power-off toggle to the record.

Playing a sound is #14's, parked. This ADR decides only where sounds and settings are
kept. The readings, from TWRP on 2026-11-21, are in `docs/device-history/phone.md`,
*alarm sounds on `system`*:

- **`system` already holds the ROM's alarm sounds**: eleven Ogg Vorbis files in
  `/system/media/audio/alarms`, 44.1 kHz stereo, 1.5 s to 23.7 s, 3016 KiB in all, and 15
  more under its `media/audio/alarms`. Ringtones and notifications beside them are 12420
  KiB and 2236 KiB.
- **`system` has 1871996 KiB free** of 3753456, by `df` on a read-only mount.
- **The kernel exposes one ALSA card**, `HI6250_hi6555c_CARD`, with seven PCM devices;
  device 0, `HI6210_hi6555c`, plays and captures. The formats it accepts are not read:
  TWRP has no `tinyalsa`, and the stream's parameters show only while one is open.
- **`userdata` has 44862988 KiB free** of 51895076.

## Decisions

### D1. The settings stay on `reserved4`, in ADR 0011's record

A slot holds 4068 bytes of payload. Format version 1 is 9 bytes and 4 per alarm, with the
count in one byte, so at most 1029 bytes. The children of issue 31 add to it:

| Field | Bytes, at most | Issue |
|---|---|---|
| version 1: offset, window, alarms (255 × 4) | 1029 | #30, #80 |
| saved timers: a count and 255 × 2, seconds up to 5999 | 511 | #81 |
| the power-off toggle | 1 | #83 |
| the chosen sound, by D4 | 65 | D4 |
| **total** | **1606** | |

**The worst case leaves 2462 bytes free, 60 % of the slot**; ten alarms and ten timers
take 136. Neither of ADR 0011's reopening conditions on size is met, and the settings
are the only thing `alarmd` writes while it runs, so they stay where a power cut costs
nothing (ADR 0011, D3).

### D2. Alarm sounds are files on `system`, read-only

**Amended by ADR 0015, D3**: `alarmd` plays only WAV files converted on the host, so the
ROM's Vorbis alarms are not used as they are. Converted copies sit beside them. The
decision as accepted:

A sound is a file on `system`, our root, which the kernel mounts read-only (ADR 0006, D4).
`alarmd` opens it by path and never writes it. The ROM's sounds under
`/system/media/audio/alarms` are already there and are used as they are, as ADR 0009, D2
uses the ROM's `mksh`; nothing is copied into the repository or into an image of ours.

**A sound of the operator's own goes beside them**, under `/system/media/audio/alarms`.
Its size is no constraint: every ROM alarm decoded to 16-bit PCM is 16.1 MiB, and `system`
has 1.8 GiB free.

**A power cut under `alarmd` costs nothing**, since nothing on `system` is written while
it runs. A cut during a write from TWRP is D3's.

### D3. A sound reaches the phone over `adb` from TWRP, and replaces nothing in place

The way `/init` reaches `system` (ADR 0006, D3): mount `system` read-write in TWRP, push
the file under a temporary name, `sync`, check its sha256 and size against the host's,
then rename it into place and unmount. A cut before the rename leaves a stray temporary
file and every sound that was there; ext4's journal replays at the next mount, read-only
included.

### D4. The record names the chosen sound by its file name

One sound serves every alarm and the timer: a setting, not a field per alarm. It is stored
as the file name, at most 64 bytes, behind a length byte, so a sound added or removed
does not shift another's reference. A name that does not open is `HardwareError`, and the
alarm rings on the vibrator alone (architecture rule 3); it is never replaced by another
sound.

The format is #14's: whether `alarmd` decodes Vorbis on the phone or the host converts a
sound to PCM before D3's push. Neither changes D2, since both fit `system` many times
over.

**Amended by issue 86**: the record is version 8, the name after the outputs. A name is
`A`–`Z`, `a`–`z`, `0`–`9`, `.`, `_` and `-`, starts with a letter or a figure and ends in
`.wav`; a length of 0 is no sound chosen, and so is a blank record. A version-7 or older
record loads with `ringtone.wav` chosen, the file those versions played, so an upgrade
rings as it did. The settings screen lists the names under `/system/media/audio/alarms`
that the rule takes, read once at start-up, and `alarmd` loads the chosen file at
start-up and again whenever the choice changes.

## Rejected alternatives

**Sounds in a file on `userdata`.** It has the room, but it is an ext4 shared with
the ROM's data: `alarmd` would mount it read-write, replay its journal, and need an `fsck`
it does not have, and a factory reset from TWRP wipes it (ADR 0011). `system` is already
mounted and holds the ROM's sounds. *Reopens if* `system` stops holding a filesystem a
file can be added to.

**A filesystem of ours on a spare partition**, ext4 on `reserved2` (46 MiB). It is a
second mount, a partition to back up and format, and a `mkfs` from TWRP, for files that
`system` holds with nothing new. *Reopens if* `system` stops holding a filesystem a file
can be added to and `userdata` is not wanted.

**Sounds raw on a spare partition**, packed by the host and written with `dd`. It needs no
mount, but an index and a framing of ours for every sound, and a rewrite of the whole
pack to add one. *Reopens if* reading a file on `system` costs `alarmd` more than an
adapter that reads a raw partition.

**Sounds built into the binary.** It needs no file at all, but a sound changed means a
new `/init`, and the ROM's 3 MiB of Vorbis would make issue 79's binary of 511984 bytes
seven times as large. *Reopens if* a single fallback sound is wanted for when the chosen
one does not open.

**The settings moved beside the sounds**, as a file on `system`. `system` is read-only
while `alarmd` runs, and remounting it for each save is ADR 0011's first rejected
alternative. *Reopens with* that alternative.

**A sound pushed over the serial shell** (ADR 0009). It carries `base64` through a
terminal, and `system` is read-only under our `/init`, so it would need a remount there.
*Reopens if* TWRP is no longer reachable.

**A sound written by `alarmd` itself.** The phone has no source of sounds: no network,
no storage the operator writes to. *Reopens if* the phone gains a way to receive a file.

**A sound per alarm.** At 69 bytes an alarm, 255 alarms are 17595 bytes, more than four
slots. *Reopens if* a sound per alarm is wanted, with the alarms capped at 50.

## Consequences

- **#80, #81 and #83 store their settings in the record**, as issue 31 assumed.
- **The chosen sound in the record is a follow-up under this issue**, waiting for #14: a
  format version raised by one, the name at the end of the payload, and the settings
  screen's choice among the files present.
- **Putting a sound of the operator's own on `system` is a follow-up under this issue**,
  waiting for #14: D3's steps, as a host script with the sha256 and size checked.
- **`system` stays the ROM's** while its sounds are used, as for its `mksh` (ADR 0009,
  D2). A filesystem of ours alone on `system` (ADR 0006, D4) carries the sounds it needs.
