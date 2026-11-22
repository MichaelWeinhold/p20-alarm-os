# Audio

**State.** `system` holds, under `/system/etc/alarmd/`, the HAL's DSP init and the
speaker's route, read off this phone and its `odm` backup. `alarmd`'s speaker adapter
reads both at start-up and logs `opening the speaker` if either is missing or malformed.
→ ADR 0015, D1 and D2.

| File | Bytes | sha256 |
|---|---|---|
| `dsp-init.bin` | 928990 | `bd239d63c82bdf09c88df740407550e142d8822ee0b43324bfcfea9a53b41efa` |
| `speaker-route.tsv` | 10067 | `f522fb9512b7bfe82de84a81f6dd0b9d7a22e3b475b69dcf910b57ede6c709b7` |

`dsp-init.bin` is the HAL's messages 0–4 and 7–11, each a `u8` kind (0 async, 1 sync,
2 `WRITE_PARAMS`), a `u32` LE body length, a `u32` LE reply length and the body.
`speaker-route.tsv` is `default`, then `speaker`, lines of `<set>\t<control>\t<value>`,
in the order they are set.

**Reproduce.** Boot the ROM: from TWRP, swap `system`'s `/init` for `init.rom` by `mv`, as
in [phone.md](phone.md), and keep ours as `/init.ours`. Then, on the Mac:

```sh
cargo build --release --target aarch64-unknown-linux-musl -p dsp-recorder
uv run python scripts/dsp_init.py                      # out/dsp-init.bin, two recordings that agree
uv run python scripts/speaker_route.py \
  --image ~/p20-backups/odm-mmcblk0p44-2026-11-21.img  # out/speaker-route.tsv
```

⚠️ `audioserver` starts after the HAL: the HAL opens `/dev/hifi_misc` only once
`audioserver` connects, and `dsp_init.py` stops both and starts them in that order.

⚠️ The recorder waits on the HAL blocking. A polling wait slows the HAL until
`audioserver`'s 5 s `TimeCheck` aborts, and the HAL is restarted mid-recording.

⚠️ Messages 8–11, two `SET_PARA` and two `SET_SOURCE`, race and may interleave;
`dsp_init.py` writes both `SET_PARA` first and refuses any other set of four.

Then reboot into TWRP and push:

```sh
uv run python scripts/system_push.py --to system/etc/alarmd out/dsp-init.bin out/speaker-route.tsv
```

⚠️ The path is `system/etc/alarmd`: on the partition, `/etc` is a link to `/system/etc`
and dangles under TWRP.

Put our `/init` back by `mv`, `init` to `init.rom` and `init.ours` to `init`, and clear
`misc` before the reboot, as in [phone.md](phone.md).

**Verify.** `system_push.py` prints each file's size and sha256 read back on the phone;
they match the table above.

**Undo.** From TWRP, `rm -r /mnt/sys/system/etc/alarmd` with `system` mounted read-write.

## Alarm sounds

**State.** `/system/media/audio/alarms` holds the ROM's eleven Vorbis alarms and the
operator's test sound beside them; `alarmd` plays only the `.wav` files, and until a
chosen sound is kept in the settings (issue 86) it plays `ringtone.wav`. → ADR 0014, D2
and D3; ADR 0015, D3.

| File | Bytes | sha256 |
|---|---|---|
| `ringtone.wav` | 5760044 | `cb3a132041de70b4b09cf058c99f7b1169ca5aaa17239fefc9b90564c58d8836` |

**Reproduce.** From TWRP, on the Mac:

```sh
uv run python scripts/alarm_sound.py sounds/ringtone.mp3   # out/sounds/ringtone.wav, pushed
```

⚠️ A name already on the phone with other contents is refused and nothing is written; push
a changed sound under a new name, or `rm` the old one first.

⚠️ A sound's name is its stem plus `.wav`, letters, digits, `.`, `_` and `-`, at most 64
bytes, which is what the settings record holds (ADR 0014, D4).

**Verify.** The script prints each sound's size and sha256 read back on the phone, and
the count of files beside them whose sha256 did not change; a second run prints
`already there` and pushes nothing.

**Undo.** From TWRP, `rm /mnt/sys/system/media/audio/alarms/<name>.wav` with `system`
mounted read-write.

The readings: [docs/device-history/phone.md](../device-history/phone.md), issues 14, 87
and 91.
