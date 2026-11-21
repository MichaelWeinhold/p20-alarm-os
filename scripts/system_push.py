from __future__ import annotations

import argparse
import hashlib
import shlex
import subprocess
import sys
from collections.abc import Callable, Sequence
from dataclasses import dataclass
from pathlib import Path, PurePosixPath
from typing import Final

ADB: Final = "adb"
TIMEOUT: Final = 120.0
RECOVERY: Final = "recovery"
SYSTEM: Final = "/dev/block/mmcblk0p52"
MOUNT: Final = PurePosixPath("/mnt/sys")
PART: Final = ".part"


class PushError(RuntimeError):
    pass


@dataclass(frozen=True, slots=True)
class Answer:
    code: int
    stdout: bytes
    stderr: str


@dataclass(frozen=True, slots=True)
class Pushed:
    path: PurePosixPath
    size: int
    sha256: str


Adb = Callable[[tuple[str, ...]], Answer]


def adb_runner(binary: str = ADB, timeout: float = TIMEOUT) -> Adb:
    def run(command: tuple[str, ...]) -> Answer:
        try:
            result = subprocess.run(
                [binary, *command], capture_output=True, timeout=timeout, check=False
            )
        except FileNotFoundError as error:
            raise PushError(f"{binary} is not installed: {error}") from error
        except subprocess.TimeoutExpired as error:
            raise PushError(f"{binary} {' '.join(command)} took over {timeout:g} s") from error
        return Answer(result.returncode, result.stdout, result.stderr.decode(errors="replace"))

    return run


def asked(adb: Adb, *command: str) -> bytes:
    answer = adb(command)
    if answer.code != 0:
        said = (
            answer.stderr.strip() or answer.stdout.decode(errors="replace").strip()
        ).splitlines()
        raise PushError(
            f"adb {' '.join(command)} exited {answer.code}: {said[0] if said else 'no output'}"
        )
    return answer.stdout


def shell(adb: Adb, command: str) -> str:
    return asked(adb, "shell", command).decode(errors="replace")


def state(adb: Adb, wanted: str) -> None:
    found = asked(adb, "get-state").decode(errors="replace").strip()
    if found != wanted:
        raise PushError(f"the phone is in {found or 'no state'}, not {wanted}")


def sha256(path: Path) -> str:
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def destination(relative: PurePosixPath) -> PurePosixPath:
    if relative.is_absolute() or not relative.parts or ".." in relative.parts:
        raise PushError(f"{relative} is not a path inside system")
    return MOUNT / relative


def on_phone(adb: Adb, path: PurePosixPath) -> tuple[str, int]:
    quoted = shlex.quote(str(path))
    lines = shell(adb, f"sha256sum {quoted} && stat -c %s {quoted}").split()
    if len(lines) != 3 or lines[1] != str(path) or not lines[2].isdigit():
        raise PushError(f"{path} reads back as {' '.join(lines) or 'nothing'}")
    return lines[0], int(lines[2])


def mounted(adb: Adb) -> None:
    point = shlex.quote(str(MOUNT))
    shell(
        adb,
        f"mkdir -p {point} && "
        f"if grep -q ' {MOUNT} ' /proc/mounts; then mount -o remount,rw {point}; "
        f"else mount -o rw {SYSTEM} {point}; fi",
    )


def placed(adb: Adb, local: Path, relative: PurePosixPath) -> Pushed:
    if not local.is_file():
        raise PushError(f"{local} is missing")
    final = destination(relative)
    part = final.with_name(final.name + PART)
    size, digest = local.stat().st_size, sha256(local)
    shell(adb, f"mkdir -p {shlex.quote(str(final.parent))}")
    asked(adb, "push", str(local), str(part))
    shell(adb, "sync")
    found = on_phone(adb, part)
    if found != (digest, size):
        shell(adb, f"rm -f {shlex.quote(str(part))}")
        raise PushError(
            f"{part} reads back as {found[1]} bytes, sha256 {found[0]}; "
            f"{local} is {size} bytes, sha256 {digest}"
        )
    shell(
        adb,
        f"chmod 644 {shlex.quote(str(part))} && mv {shlex.quote(str(part))} "
        f"{shlex.quote(str(final))} && sync",
    )
    if on_phone(adb, final) != (digest, size):
        raise PushError(f"{final} changed in its rename")
    return Pushed(final, size, digest)


def push(adb: Adb, files: Sequence[tuple[Path, PurePosixPath]]) -> list[Pushed]:
    state(adb, RECOVERY)
    mounted(adb)
    try:
        return [placed(adb, local, relative) for local, relative in files]
    finally:
        shell(adb, f"umount {shlex.quote(str(MOUNT))}")


def parser() -> argparse.ArgumentParser:
    found = argparse.ArgumentParser(prog="system_push.py")
    found.add_argument("--to", type=PurePosixPath, required=True)
    found.add_argument("files", type=Path, nargs="+")
    return found


def main(argv: list[str], adb: Adb | None = None) -> int:
    options = parser().parse_args(argv[1:])
    to: PurePosixPath = options.to
    files: list[Path] = options.files
    try:
        pushed = push(adb or adb_runner(), [(local, to / local.name) for local in files])
    except PushError as error:
        print(error, file=sys.stderr)
        return 1
    for each in pushed:
        print(f"{each.path}: {each.size} bytes, sha256 {each.sha256}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
