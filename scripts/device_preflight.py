from __future__ import annotations

import argparse
import ctypes
import hashlib
import re
import subprocess
import sys
from collections.abc import Callable
from dataclasses import dataclass
from pathlib import Path
from typing import Final

FASTBOOT: Final = "fastboot"
LIBUSB: Final = Path("/opt/homebrew/lib/libusb-1.0.dylib")
VENDOR: Final = 0x18D1
PRODUCT: Final = 0xD00D
SUCCESS: Final = 0
RE_ENUMERATED: Final = -5
TIMEOUT: Final = 10.0
BOOTLOADER: Final = "(bootloader)"
MODEL: Final = "ANE-LX1"
UNLOCKED: Final = "UNLOCKED"
LOCKS: Final = ("FB", "USER")
LOCK_STATE: Final = re.compile(r"^(\w+) LockState: (\w+)$")
PARTITION: Final = 25165824


class PreflightError(RuntimeError):
    pass


@dataclass(frozen=True, slots=True)
class Answer:
    code: int
    stderr: str


@dataclass(frozen=True, slots=True)
class Backup:
    path: Path
    sha256: str


STOCK: Final = Backup(
    Path.home() / "p20-backups" / "kernel-mmcblk0p31-stock.img",
    "fe6e372eaf56b4358905443e3c475bde05851d7acd1cadbce395a376b4986a84",
)

Reset = Callable[[], None]
Fastboot = Callable[[tuple[str, ...]], Answer]


def libusb_reset(library: Path = LIBUSB) -> None:
    try:
        usb = ctypes.CDLL(str(library))
    except OSError as error:
        raise PreflightError(f"{library} does not load, so brew install libusb: {error}") from error
    usb.libusb_init.argtypes = [ctypes.POINTER(ctypes.c_void_p)]
    usb.libusb_open_device_with_vid_pid.argtypes = [
        ctypes.c_void_p,
        ctypes.c_uint16,
        ctypes.c_uint16,
    ]
    usb.libusb_open_device_with_vid_pid.restype = ctypes.c_void_p
    usb.libusb_reset_device.argtypes = [ctypes.c_void_p]
    usb.libusb_close.argtypes = [ctypes.c_void_p]
    usb.libusb_exit.argtypes = [ctypes.c_void_p]
    usb.libusb_error_name.argtypes = [ctypes.c_int]
    usb.libusb_error_name.restype = ctypes.c_char_p
    context = ctypes.c_void_p()
    started: int = usb.libusb_init(ctypes.byref(context))
    if started != SUCCESS:
        raise PreflightError(f"libusb_init failed: {usb.libusb_error_name(started).decode()}")
    try:
        handle: int | None = usb.libusb_open_device_with_vid_pid(context, VENDOR, PRODUCT)
        if handle is None:
            raise PreflightError(
                f"no {VENDOR:04x}:{PRODUCT:04x} on USB: the phone is not in Fastboot & Rescue mode"
            )
        try:
            reset: int = usb.libusb_reset_device(handle)
        finally:
            usb.libusb_close(handle)
        if reset not in (SUCCESS, RE_ENUMERATED):
            raise PreflightError(
                f"libusb_reset_device failed: {usb.libusb_error_name(reset).decode()}"
            )
    finally:
        usb.libusb_exit(context)


def fastboot_runner(binary: str = FASTBOOT, timeout: float = TIMEOUT) -> Fastboot:
    def run(command: tuple[str, ...]) -> Answer:
        invocation = " ".join((binary, *command))
        try:
            result = subprocess.run(
                [binary, *command], capture_output=True, text=True, timeout=timeout, check=False
            )
        except FileNotFoundError as error:
            raise PreflightError(f"{binary} is not installed: {error}") from error
        except subprocess.TimeoutExpired as error:
            raise PreflightError(
                f"{invocation} answered nothing in {timeout:g} s: the USB reset did not take"
            ) from error
        return Answer(result.returncode, result.stderr)

    return run


def bootloader_lines(answer: Answer) -> list[str]:
    return [
        line.removeprefix(BOOTLOADER).strip()
        for line in answer.stderr.splitlines()
        if line.startswith(BOOTLOADER)
    ]


def ask(reset: Reset, fastboot: Fastboot, *command: str) -> list[str]:
    reset()
    answer = fastboot(command)
    if answer.code != SUCCESS:
        said = answer.stderr.strip().splitlines()
        raise PreflightError(
            f"fastboot {' '.join(command)} exited {answer.code}: {said[0] if said else 'no output'}"
        )
    return bootloader_lines(answer)


def model(lines: list[str]) -> str:
    if lines != [MODEL]:
        raise PreflightError(f"the device is {' / '.join(lines) or 'unnamed'}, not {MODEL}")
    return MODEL


def locks(lines: list[str]) -> dict[str, str]:
    found = {
        match.group(1): match.group(2)
        for line in lines
        if (match := LOCK_STATE.fullmatch(line)) is not None
    }
    for name in LOCKS:
        state = found.get(name)
        if state != UNLOCKED:
            raise PreflightError(f"{name} LockState is {state or 'not reported'}, not {UNLOCKED}")
    return found


def sha256(path: Path) -> str:
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def backed_up(backup: Backup) -> str:
    if not backup.path.is_file():
        raise PreflightError(f"the backup {backup.path} is missing")
    found = sha256(backup.path)
    if found != backup.sha256:
        raise PreflightError(f"the backup {backup.path} hashes to {found}, not {backup.sha256}")
    return found


def fits(image: Path) -> tuple[int, str]:
    if not image.is_file():
        raise PreflightError(f"the image {image} is missing")
    size = image.stat().st_size
    if size == 0:
        raise PreflightError(f"the image {image} is empty")
    if size > PARTITION:
        raise PreflightError(f"the image {image} is {size} bytes, over kernel's {PARTITION}")
    return size, sha256(image)


def parser() -> argparse.ArgumentParser:
    found = argparse.ArgumentParser(prog="device_preflight.py")
    target = found.add_mutually_exclusive_group(required=True)
    target.add_argument("image", type=Path, nargs="?")
    target.add_argument("--reset", action="store_true")
    return found


def preflight(image: Path, reset: Reset, run: Fastboot, backup: Backup) -> None:
    size, digest = fits(image)
    print(f"image {image}: {size} bytes, sha256 {digest}")
    backed_up(backup)
    print(f"backup {backup.path}: sha256 {backup.sha256}")
    print(f"device: {model(ask(reset, run, 'oem', 'get-product-model'))}")
    states = locks(ask(reset, run, "oem", "lock-state", "info"))
    print("lock state: " + ", ".join(f"{name} {states[name]}" for name in LOCKS))


def main(
    argv: list[str],
    reset: Reset = libusb_reset,
    fastboot: Fastboot | None = None,
    backup: Backup = STOCK,
) -> int:
    options = parser().parse_args(argv[1:])
    image: Path | None = options.image
    try:
        if image is not None:
            preflight(image, reset, fastboot or fastboot_runner(), backup)
        reset()
    except PreflightError as error:
        print(error, file=sys.stderr)
        return 1
    print("ready: the next fastboot invocation is answered")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
