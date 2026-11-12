from __future__ import annotations

import argparse
import dataclasses
import gzip
import struct
import sys
from collections.abc import Iterable
from dataclasses import dataclass
from pathlib import Path
from typing import Final

from boot_image import BootImageError, build, sha256, unpack

NEWC: Final = b"070701"
TRAILER: Final = "TRAILER!!!"
ALIGNMENT: Final = 4
FILE_TYPE: Final = 0o170000
DIRECTORY: Final = 0o040000
CHARACTER: Final = 0o020000
REGULAR: Final = 0o100000
ELF: Final = b"\x7fELF"
ELF_64_LITTLE: Final = b"\x02\x01"
EM_AARCH64: Final = 183


class InitramfsError(ValueError):
    pass


@dataclass(frozen=True, slots=True)
class Entry:
    name: str
    mode: int
    data: bytes = b""
    rdev: tuple[int, int] = (0, 0)


def aligned(data: bytes) -> bytes:
    return data + b"\0" * (-len(data) % ALIGNMENT)


def record(inode: int, entry: Entry) -> bytes:
    name = entry.name.encode("ascii") + b"\0"
    links = 2 if entry.mode & FILE_TYPE == DIRECTORY else 1
    values = (inode, entry.mode, 0, 0, links, 0, len(entry.data), 0, 0, *entry.rdev, len(name), 0)
    header = NEWC + b"".join(f"{value:08x}".encode("ascii") for value in values)
    return aligned(header + name) + aligned(entry.data)


def archive(entries: Iterable[Entry]) -> bytes:
    every = (*entries, Entry(TRAILER, 0))
    return b"".join(record(inode, entry) for inode, entry in enumerate(every, start=1))


def checked_init(data: bytes) -> bytes:
    if data[:4] != ELF or data[4:6] != ELF_64_LITTLE:
        raise InitramfsError("/init is not a 64-bit little-endian ELF")
    (machine,) = struct.unpack_from("<H", data, 18)
    if machine != EM_AARCH64:
        raise InitramfsError(f"/init is built for ELF machine {machine}, not aarch64")
    return data


def entries(init: bytes) -> tuple[Entry, ...]:
    return (
        Entry("dev", DIRECTORY | 0o755),
        Entry("dev/console", CHARACTER | 0o600, rdev=(5, 1)),
        Entry("dev/null", CHARACTER | 0o666, rdev=(1, 3)),
        Entry("proc", DIRECTORY | 0o755),
        Entry("init", REGULAR | 0o755, checked_init(init)),
    )


def ramdisk(init: bytes) -> bytes:
    return gzip.compress(archive(entries(init)), compresslevel=9, mtime=0)


def image_with(stock: bytes, init: bytes) -> bytes:
    image = unpack(stock)
    return build(dataclasses.replace(image, ramdisk=ramdisk(init)))


def parser() -> argparse.ArgumentParser:
    found = argparse.ArgumentParser(prog="initramfs.py")
    found.add_argument("init", type=Path)
    found.add_argument("stock", type=Path)
    found.add_argument("image", type=Path)
    return found


def main(argv: list[str]) -> int:
    options = parser().parse_args(argv[1:])
    init: Path = options.init
    stock: Path = options.stock
    target: Path = options.image
    try:
        data = image_with(stock.read_bytes(), init.read_bytes())
        with target.open("xb") as stream:
            stream.write(data)
    except (BootImageError, InitramfsError, OSError) as error:
        print(error, file=sys.stderr)
        return 1
    print(f"image {target}: {len(data)} bytes, sha256 {sha256(data)}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
