from __future__ import annotations

import argparse
import re
import shlex
import struct
import subprocess
import sys
from collections.abc import Callable, Sequence
from dataclasses import dataclass
from pathlib import Path, PurePosixPath
from typing import Final

from system_push import (
    MOUNT,
    Adb,
    Pushed,
    PushError,
    adb_runner,
    on_phone,
    placed,
    sha256,
    shell,
    writable,
)

FFMPEG: Final = "ffmpeg"
TIMEOUT: Final = 120.0
ALARMS: Final = PurePosixPath("system/media/audio/alarms")
OUT: Final = Path("out/sounds")
SUFFIX: Final = ".wav"
LONGEST: Final = 64
NAME: Final = re.compile(r"[A-Za-z0-9][A-Za-z0-9._-]*")
DIGEST: Final = re.compile(r"[0-9a-f]{64}")
HEADER: Final = struct.Struct("<4sI4s4sIHHIIHH4sI")
PCM: Final = 1
CHANNELS: Final = 2
RATE: Final = 48000
BITS: Final = 16
FRAME: Final = CHANNELS * BITS // 8
FORMAT: Final = (b"WAVE", b"fmt ", 16, PCM, CHANNELS, RATE, RATE * FRAME, FRAME, BITS, b"data")


class SoundError(RuntimeError):
    pass


@dataclass(frozen=True, slots=True)
class Installed:
    written: list[Pushed]
    kept: list[Pushed]
    beside: int


Convert = Callable[[Path, Path], None]


def ffmpeg_runner(binary: str = FFMPEG, timeout: float = TIMEOUT) -> Convert:
    def run(source: Path, wav: Path) -> None:
        command = [
            binary,
            "-nostdin",
            "-hide_banner",
            "-loglevel",
            "error",
            "-y",
            "-i",
            str(source),
            "-map",
            "0:a:0",
            "-map_metadata",
            "-1",
            "-fflags",
            "+bitexact",
            "-flags:a",
            "+bitexact",
            "-c:a",
            "pcm_s16le",
            "-ar",
            str(RATE),
            "-ac",
            str(CHANNELS),
            "-f",
            "wav",
            str(wav),
        ]
        try:
            result = subprocess.run(command, capture_output=True, timeout=timeout, check=False)
        except FileNotFoundError as error:
            raise SoundError(f"{binary} is not installed: {error}") from error
        except subprocess.TimeoutExpired as error:
            raise SoundError(f"{binary} took over {timeout:g} s on {source}") from error
        if result.returncode != 0:
            said = result.stderr.decode(errors="replace").strip().splitlines()
            raise SoundError(
                f"{binary} exited {result.returncode} on {source}: "
                f"{said[0] if said else 'no output'}"
            )

    return run


def named(source: Path) -> str:
    name = source.stem + SUFFIX
    if not NAME.fullmatch(source.stem) or len(name.encode()) > LONGEST:
        raise SoundError(
            f"{name} is not a sound name: letters, digits, '.', '_' and '-', "
            f"at most {LONGEST} bytes"
        )
    return name


def checked(wav: Path) -> None:
    data = wav.read_bytes()
    if len(data) < HEADER.size:
        raise SoundError(f"{wav} is {len(data)} bytes, shorter than a WAV header")
    length = len(data) - HEADER.size
    if HEADER.unpack_from(data) != (b"RIFF", len(data) - 8, *FORMAT, length):
        raise SoundError(f"{wav} is not PCM 16-bit, {RATE} Hz, stereo with a bare header")
    if length == 0 or length % FRAME:
        raise SoundError(f"{wav} holds {length} bytes of sound, not whole frames")


def converted(convert: Convert, sources: Sequence[Path], out: Path) -> list[Path]:
    names = [named(source) for source in sources]
    if len(set(names)) != len(names):
        raise SoundError(f"two sounds would share a name: {', '.join(sorted(names))}")
    out.mkdir(parents=True, exist_ok=True)
    wavs: list[Path] = []
    for source, name in zip(sources, names, strict=True):
        if not source.is_file():
            raise SoundError(f"{source} is missing")
        wav = out / name
        convert(source, wav)
        checked(wav)
        wavs.append(wav)
    return wavs


def inventory(adb: Adb) -> dict[str, str]:
    directory = shlex.quote(str(MOUNT / ALARMS))
    found: dict[str, str] = {}
    listing = shell(
        adb,
        f'cd {directory} && for f in *; do if [ -f "$f" ]; then sha256sum "$f" || exit 1; fi; done',
    )
    for line in listing.splitlines():
        digest, _, name = line.partition("  ")
        if not DIGEST.fullmatch(digest) or not name:
            raise PushError(f"{MOUNT / ALARMS} lists {line!r}, not a sha256 and a name")
        found[name] = digest
    if not found:
        raise PushError(f"{MOUNT / ALARMS} holds no files: is system still the ROM's?")
    return found


def install(adb: Adb, wavs: Sequence[Path]) -> Installed:
    with writable(adb):
        before = inventory(adb)
        kept: list[Pushed] = []
        fresh: list[Path] = []
        for wav in wavs:
            final = MOUNT / ALARMS / wav.name
            ours = (sha256(wav), wav.stat().st_size)
            if wav.name not in before:
                fresh.append(wav)
                continue
            there = on_phone(adb, final)
            if there != ours:
                raise PushError(
                    f"{final} is already there, {there[1]} bytes, sha256 {there[0]}; "
                    f"{wav} is {ours[1]} bytes, sha256 {ours[0]}: nothing written"
                )
            kept.append(Pushed(final, ours[1], ours[0]))
        written = [placed(adb, wav, ALARMS / wav.name) for wav in fresh]
        after = inventory(adb)
        moved = sorted(name for name, digest in before.items() if after.get(name) != digest)
        if moved:
            raise PushError(f"{MOUNT / ALARMS} changed beside the push: {', '.join(moved)}")
    return Installed(written, kept, len(before) - len(kept))


def parser() -> argparse.ArgumentParser:
    found = argparse.ArgumentParser(prog="alarm_sound.py")
    found.add_argument("--out", type=Path, default=OUT)
    found.add_argument("sounds", type=Path, nargs="+")
    return found


def main(argv: list[str], adb: Adb | None = None, convert: Convert | None = None) -> int:
    options = parser().parse_args(argv[1:])
    out: Path = options.out
    sounds: list[Path] = options.sounds
    try:
        wavs = converted(convert or ffmpeg_runner(), sounds, out)
        done = install(adb or adb_runner(), wavs)
    except (SoundError, PushError, OSError) as error:
        print(error, file=sys.stderr)
        return 1
    for each in done.written:
        print(f"{each.path}: {each.size} bytes, sha256 {each.sha256}")
    for each in done.kept:
        print(f"{each.path}: already there, {each.size} bytes, sha256 {each.sha256}")
    print(f"{done.beside} files beside them unchanged")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
