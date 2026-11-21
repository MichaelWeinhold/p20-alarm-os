from __future__ import annotations

import hashlib
import shlex
import struct
from collections.abc import Callable
from pathlib import Path

import pytest

from alarm_sound import ALARMS, SoundError, checked, converted, ffmpeg_runner, main
from system_push import Answer
from test_system_push import Phone

DIRECTORY = f"/mnt/sys/{ALARMS}"
ROM = (
    "Argon.ogg",
    "Carbon.ogg",
    "CyanAlarm.ogg",
    "Helium.ogg",
    "Krypton.ogg",
    "Neon.ogg",
    "NuclearLaunch.ogg",
    "Osmium.ogg",
    "Oxygen.ogg",
    "Platinum.ogg",
    "Timer.ogg",
)


def wav(frames: int, rate: int = 48000, channels: int = 2, bits: int = 16) -> bytes:
    align = channels * bits // 8
    data = b"\x01\x00" * (frames * align // 2)
    header = struct.pack(
        "<4sI4s4sIHHIIHH4sI",
        b"RIFF",
        36 + len(data),
        b"WAVE",
        b"fmt ",
        16,
        1,
        channels,
        rate,
        rate * align,
        align,
        bits,
        b"data",
        len(data),
    )
    return header + data


class Alarms(Phone):
    def __init__(self, names: tuple[str, ...] = ROM) -> None:
        super().__init__()
        for name in names:
            self.files[f"{DIRECTORY}/{name}"] = f"rom {name}".encode()

    def shell(self, line: str) -> Answer:
        if line.startswith("cd "):
            directory = shlex.split(line)[1]
            listed = [
                f"{hashlib.sha256(data).hexdigest()}  {path.rsplit('/', 1)[1]}"
                for path, data in self.files.items()
                if path.rsplit("/", 1)[0] == directory
            ]
            return Answer(0, "".join(f"{each}\n" for each in listed).encode(), "")
        return super().shell(line)

    def rom(self) -> dict[str, bytes]:
        return {path: data for path, data in self.files.items() if path.endswith(".ogg")}


def converter(data: bytes) -> Callable[[Path, Path], None]:
    def convert(source: Path, out: Path) -> None:
        out.write_bytes(data)

    return convert


def source(tmp_path: Path, name: str = "ringtone.mp3") -> Path:
    path = tmp_path / name
    path.write_bytes(b"ID3 an mp3")
    return path


def run(tmp_path: Path, phone: Phone, data: bytes, *names: str) -> int:
    sounds = [str(source(tmp_path, name)) for name in names or ("ringtone.mp3",)]
    argv = ["alarm_sound.py", "--out", str(tmp_path / "out"), *sounds]
    return main(argv, phone, converter(data))


def pushes(phone: Phone) -> list[tuple[str, ...]]:
    return [call for call in phone.calls if call[0] == "push"]


def test_a_sound_lands_beside_the_roms_alarms_and_leaves_them_unchanged(
    tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    phone = Alarms()
    rom = phone.rom()
    data = wav(48000)
    assert run(tmp_path, phone, data) == 0
    assert phone.files[f"{DIRECTORY}/ringtone.wav"] == data
    assert phone.rom() == rom
    out = capsys.readouterr().out
    assert f"{DIRECTORY}/ringtone.wav: {len(data)} bytes" in out
    assert "11 files beside them unchanged" in out
    assert phone.calls[-1] == ("shell", "umount /mnt/sys")


def test_a_second_run_finds_the_same_sound_there_and_writes_nothing(
    tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    phone = Alarms()
    data = wav(4800)
    assert run(tmp_path, phone, data) == 0
    phone.calls.clear()
    assert run(tmp_path, phone, data) == 0
    assert pushes(phone) == []
    assert f"{DIRECTORY}/ringtone.wav: already there" in capsys.readouterr().out


def test_a_different_sound_under_a_name_already_there_is_refused_before_any_push(
    tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    phone = Alarms((*ROM, "ringtone.wav"))
    assert run(tmp_path, phone, wav(4800), "Argon.mp3", "ringtone.mp3") == 1
    assert pushes(phone) == []
    assert phone.files[f"{DIRECTORY}/ringtone.wav"] == b"rom ringtone.wav"
    assert "is already there" in capsys.readouterr().err
    assert phone.calls[-1] == ("shell", "umount /mnt/sys")


def test_a_rom_alarm_that_changes_beside_the_push_fails_it(tmp_path: Path) -> None:
    class Changing(Alarms):
        def __call__(self, command: tuple[str, ...]) -> Answer:
            if command[0] == "push":
                self.files[f"{DIRECTORY}/Neon.ogg"] = b"changed"
            return super().__call__(command)

    phone = Changing()
    assert run(tmp_path, phone, wav(4800)) == 1


def test_a_directory_with_no_files_is_refused_as_not_the_roms_system(
    tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    phone = Alarms(())
    assert run(tmp_path, phone, wav(4800)) == 1
    assert pushes(phone) == []
    assert "holds no files" in capsys.readouterr().err


def test_a_phone_outside_twrp_is_refused_after_the_conversion(tmp_path: Path) -> None:
    phone = Alarms()
    phone.state = "device"
    assert run(tmp_path, phone, wav(4800)) == 1
    assert phone.calls == [("get-state",)]
    assert (tmp_path / "out" / "ringtone.wav").is_file()


@pytest.mark.parametrize(
    "data",
    [
        wav(4800, rate=44100),
        wav(4800, channels=1),
        wav(4800, bits=24),
        wav(4800)[:-2],
        wav(4800)[:44] + b"LIST" + wav(4800)[44:],
        wav(0),
        b"RIFF",
    ],
    ids=["44100", "mono", "24-bit", "short", "list", "empty", "truncated"],
)
def test_anything_but_bare_pcm_16_bit_48000_stereo_is_refused_before_the_phone(
    tmp_path: Path, data: bytes
) -> None:
    phone = Alarms()
    assert run(tmp_path, phone, data) == 1
    assert phone.calls == []


def test_the_converted_test_sounds_header_passes(tmp_path: Path) -> None:
    path = tmp_path / "ringtone.wav"
    path.write_bytes(wav(30 * 48000))
    checked(path)
    assert path.stat().st_size == 5760044


@pytest.mark.parametrize("name", ["has space.mp3", ".hidden.mp3", f"{'a' * 61}.mp3"])
def test_a_name_the_settings_record_cannot_hold_is_refused(tmp_path: Path, name: str) -> None:
    with pytest.raises(SoundError, match="is not a sound name"):
        converted(converter(wav(4800)), [source(tmp_path, name)], tmp_path / "out")


def test_two_sounds_that_would_share_a_name_are_refused(tmp_path: Path) -> None:
    with pytest.raises(SoundError, match="share a name"):
        converted(
            converter(wav(4800)),
            [source(tmp_path, "ringtone.mp3"), source(tmp_path, "ringtone.ogg")],
            tmp_path / "out",
        )


def test_a_missing_sound_is_named(tmp_path: Path) -> None:
    with pytest.raises(SoundError, match="is missing"):
        converted(converter(wav(4800)), [tmp_path / "gone.mp3"], tmp_path / "out")


def test_a_missing_ffmpeg_is_named(tmp_path: Path) -> None:
    convert = ffmpeg_runner(str(tmp_path / "no-ffmpeg"))
    with pytest.raises(SoundError, match="is not installed"):
        convert(source(tmp_path), tmp_path / "ringtone.wav")
