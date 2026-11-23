# 15. Ring through the speaker after replaying the HAL's DSP init

- **Status:** Accepted; D1 amended with the wait after the replay by issue 92
- **Date:** 2026-11-21
- **Issue:** #14
- **Amends:** ADR 0014, D2

## Context

The plan parked the speaker as its largest risk: Kirin audio runs through a HiFi DSP that
Android's audio HAL sets up. Issue 14 asks whether `alarmd`, as `/init` on `system` with
no Android userspace, can play a sound through the speaker when an alarm rings, and at
what cost. ADR 0014 keeps sounds as files on `system`, read-only, and leaves their format
to this issue.

The readings, from TWRP, the debug `/init` and the ROM on 2026-11-21, are in
`docs/device-history/phone.md`, *the speaker from TWRP, our debug `/init` and the ROM*:

- **The speaker is the codec's own class-D.** `smartpakit`, TAS2560 and TFA9895 are
  disabled in the device tree.
- **The route is the HAL's `media-speaker` path**: 272 mixer controls, read from `odm`'s
  `mixer_paths_anne.xml`.
- **PCM device 0 takes only `S16_LE` stereo**, and plays 48000 Hz in periods of 960.
- **With the route set and nothing sent to the DSP, nothing is heard.** Under our `/init`
  the DSP plays eight times too slowly; under TWRP it is not loaded at all.
- **Twelve `ioctl`s on `/dev/hifi_misc` make the difference.** They were recorded from
  the HAL at its start, and ten of them were replayed under the debug `/init`. After
  that, a 6 s clip played in 6.03 s and was heard through the speaker. Three of the ten
  write a 309608-byte parameter block that the HAL builds from `odm`'s algorithm XML.

## Decisions

### D1. `alarmd` replays the HAL's DSP init, once per boot, before the first sound

It sends the recorded messages 0 to 4 and 7 to 11 of the reading, in order, as they were
recorded:

- `PLAY_PAUSE`;
- the three parameter writes;
- `0xdd9c`, `SET_DEVICE`, `SET_PARA` twice and `SET_SOURCE` twice.

It does not send the two USB audio queries. An `ioctl` that fails is `HardwareError`, and
the alarm rings on the vibrator alone (architecture rule 3).

**The messages are a file on `system`, never in the repository.** The parameter block is
`odm`'s data, read off the phone (AGENTS.md). A host script records it under the ROM,
whose `/init` stays on `system` as `init.rom`:

1. stop `audioserver` and the HAL;
2. start the HAL under an on-device recorder that `ptrace`s it;
3. keep only the messages' bodies and their `ioctl` kinds;
4. put the file on `system` by ADR 0014, D3's steps.

`alarmd` builds each argument struct itself. It sends the file as it is and does not try
to understand it.

**Amended by issue 92**: a stream started within milliseconds of the replay played at an
eighth of its speed, unheard, and the DSP's watchdog fired when it ended; started 2 s or
10 s after it, 6 s of the clip played in 6.03 s and 6.04 s. Issue 14's heard run had about
5 s between the two by chance. `alarmd` replays the messages at start-up, and a ring
within 5 s of the replay waits out the rest. A second replay, into a DSP that had played,
did no harm.

### D2. The route is `media-speaker`'s controls, set at each ring and unset after it

A host script reads `mixer_paths_anne.xml` from the `odm` backup. It writes the 257
top-level defaults and `media-speaker`'s 15 controls as name and value, one per line, to a
second file on `system`, by the same steps. When a ring starts, `alarmd` sets them
through `controlC0`'s element `ioctl`s by name. When the ring ends, it sets the 15 back to
their defaults, so the class-D is not left on between rings. The follow-up's reading of
the draw after a ring checks that this is enough.

`CLASSD_VOLTAGE_CONFIG` refuses its value under every `/init` and is skipped. The speaker
sounded without it. The volume is the route's and the DSP's as recorded; a volume setting
is not decided here.

### D3. A sound is a WAV file, PCM 16-bit, 48000 Hz, stereo, converted on the host

The host converts any sound with `ffmpeg` before ADR 0014, D3's push. That includes the
operator's MP3 and the ROM's Vorbis alarms, and the converted file is the one format
PCM device 0 accepts. `alarmd` reads the RIFF header and plays the data as it is. A file
in any other format, at any other rate or with another channel count is `HardwareError`,
and `alarmd` never resamples or decodes. The phone needs no decoder and no resampler,
and `alarmd` takes no dependency for audio (ADR 0001, D7).

**This amends ADR 0014, D2**, which used the ROM's alarm sounds as they are. The ROM's
Vorbis files stay where they are. Converted copies go beside them as `.wav` and are the
only files `alarmd` offers. All eleven decoded are 16.1 MiB; `system` has 1.8 GiB free.

### D4. The cost is one adapter crate, `speaker`, behind one trait

`alarm_core::hardware` gains a `Speaker` trait that starts and stops a sound. The new
crate `speaker` is its adapter, and the only code that opens `/dev/hifi_misc`,
`controlC0` and `pcmC0D0p` (architecture rules 2 and 6). Neither the display nor the
vibrator changes.

The `ioctl`s it needs:

| Device | `ioctl`s |
|---|---|
| `controlC0` | `ELEM_LIST`, `ELEM_INFO`, `ELEM_WRITE` |
| `pcmC0D0p` | `HW_PARAMS`, `PREPARE`, `WRITEI_FRAMES`, `DROP` |
| `/dev/hifi_misc` | its async send, its sync send and `WRITE_PARAMS` |

**The sound plays on a thread of the adapter's own.** The largest buffer is 3840 frames,
80 ms, which is shorter than any wait in `alarmd`'s event loop. The thread loops the file
until `stop`, while the vibrator pulses as issue 13 built it.

**The draw while ringing was not read.** It is read by the adapter's follow-up. A ring
lasts minutes a day, so it does not change ADR 0008's and ADR 0010's power plan unless
the draw goes on after `stop`.

## Rejected alternatives

**Android's HAL run under our `/init`.** It needs bionic's linker, `hwservicemanager` and
`audioserver`: the Android userspace ADR 0002 left behind. *Reopens if* the recorded init
stops working and a new recording cannot be made.

**The parameter block built by `alarmd` from `odm`'s XML.** That means reimplementing the
HAL's `ap_param_init` for four algorithm files, for a block that does not change while
`alarmd` runs. *Reopens if* the ROM leaves `system`, so the HAL can no longer be
recorded, or if the block must change at runtime, for a volume for example.

**A smaller, hand-written init.** `SET_DEVICE` alone left the DSP eight times too slow,
and its watchdog fired, but its stream started straight after it, which issue 92 found
slows the full init too. Which subset of the twelve is enough was not searched. *Reopens
if* sending the replay costs a ring a noticeable delay or draw.

**The PCM sent past the DSP.** Device 0 is fed through the DSP's mailbox: with the DSP
not loaded, in TWRP, it logs `remote mb not ready`. Whether one of the other six PCM
devices reaches the codec without the DSP was not read. *Reopens if* one does, or a
kernel of ours (ADR 0004) exposes such a path.

**Vorbis or MP3 decoded by `alarmd`**, so the ROM's sounds play as they are. It adds a
decoder crate and a resampler for the ROM's 44.1 kHz, both on the phone, for files the
host converts once. *Reopens if* a sound must reach the phone without the host, or the
converted sounds outgrow `system`.

**Raw PCM with no header.** It is 44 bytes smaller, but a file in the wrong format would
play as noise rather than fail as `HardwareError`. *Reopens if* a sound is ever streamed
from something other than a file.

**`tinyalsa` or `alsa-lib`, linked statically.** They need a C toolchain for musl and
`unsafe` FFI around a C library, to make seven `ioctl`s of fixed layout. *Reopens if* the
PCM needs more than interleaved writes: `mmap`, mixing or a second stream.

**A smart amplifier through `smartpakit`.** None is enabled on this board. *Reopens if*
the device tree enables one.

## Consequences

- **The recorder and the two host scripts are a follow-up under this issue.** They make
  D1's messages and D2's route into files on `system`, checked by sha256. Two recordings
  from two starts of the HAL must give the same file.
- **The `speaker` adapter and the ring through it are a follow-up under this issue**,
  waiting for the first. It rings with a sound on `system` and with the vibrator. It is
  heard after a cold boot, after a suspend and after a power-on alarm, and the draw is
  read while it rings and after it stops.
- **Issue 87's script converts a sound to D3's WAV before it pushes it**, and issue 86's
  choice lists only `.wav` files.
- **`system` keeps the ROM's `init.rom`**, which a new recording needs.
