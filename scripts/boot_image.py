from __future__ import annotations

import argparse
import hashlib
import json
import struct
import sys
from dataclasses import asdict, dataclass, fields
from pathlib import Path
from typing import Final

MAGIC: Final = b"ANDROID!"
VERSION: Final = 0
HEADER: Final = struct.Struct("<8s10I16s512s32s1024s")
NAME: Final = 16
CMDLINE: Final = 512
EXTRA_CMDLINE: Final = 1024
ID: Final = 32
ID_AT: Final = HEADER.size - EXTRA_CMDLINE - ID
PAGE_SIZES: Final = (2048, 4096, 8192, 16384)
FIELDS: Final = "header.json"


class BootImageError(ValueError):
    pass


@dataclass(frozen=True, slots=True)
class Header:
    kernel_addr: int
    ramdisk_addr: int
    second_addr: int
    tags_addr: int
    page_size: int
    os_version: int
    name: str
    cmdline: str
    extra_cmdline: str


@dataclass(frozen=True, slots=True)
class BootImage:
    header: Header
    kernel: bytes
    ramdisk: bytes
    second: bytes

    def sections(self) -> dict[str, bytes]:
        return {"kernel": self.kernel, "ramdisk": self.ramdisk, "second": self.second}


def pages(size: int, page_size: int) -> int:
    return -(-size // page_size) * page_size


def padded(data: bytes, page_size: int) -> bytes:
    return data.ljust(pages(len(data), page_size), b"\0")


def identity(image: BootImage) -> bytes:
    digest = hashlib.sha1(usedforsecurity=False)
    for section in image.sections().values():
        digest.update(section)
        digest.update(struct.pack("<I", len(section)))
    return digest.digest().ljust(ID, b"\0")


def encoded(text: str, field: str, size: int) -> bytes:
    try:
        data = text.encode("ascii")
    except UnicodeEncodeError as error:
        raise BootImageError(f"{field} is not ASCII: {error}") from error
    if b"\0" in data or len(data) > size:
        raise BootImageError(f"{field} does not fit its {size}-byte field without a NUL")
    return data


def decoded(data: bytes, field: str) -> str:
    try:
        return data.split(b"\0", 1)[0].decode("ascii")
    except UnicodeDecodeError as error:
        raise BootImageError(f"{field} is not ASCII: {error}") from error


def build(image: BootImage) -> bytes:
    header = image.header
    if header.page_size not in PAGE_SIZES:
        raise BootImageError(f"page size {header.page_size} is not one of {PAGE_SIZES}")
    if not image.kernel:
        raise BootImageError("the kernel is empty")
    try:
        packed = HEADER.pack(
            MAGIC,
            len(image.kernel),
            header.kernel_addr,
            len(image.ramdisk),
            header.ramdisk_addr,
            len(image.second),
            header.second_addr,
            header.tags_addr,
            header.page_size,
            VERSION,
            header.os_version,
            encoded(header.name, "name", NAME),
            encoded(header.cmdline, "cmdline", CMDLINE),
            identity(image),
            encoded(header.extra_cmdline, "extra_cmdline", EXTRA_CMDLINE),
        )
    except struct.error as error:
        raise BootImageError(f"a header field does not fit 32 bits: {error}") from error
    return b"".join(
        padded(part, header.page_size) for part in (packed, *image.sections().values()) if part
    )


def parse(data: bytes) -> BootImage:
    if len(data) < HEADER.size:
        raise BootImageError(f"{len(data)} bytes is shorter than a {HEADER.size}-byte header")
    (
        magic,
        kernel_size,
        kernel_addr,
        ramdisk_size,
        ramdisk_addr,
        second_size,
        second_addr,
        tags_addr,
        page_size,
        version,
        os_version,
        name,
        cmdline,
        _,
        extra_cmdline,
    ) = HEADER.unpack_from(data)
    if magic != MAGIC:
        raise BootImageError(f"the magic is {magic!r}, not {MAGIC!r}")
    if version != VERSION:
        raise BootImageError(f"header version {version} is not {VERSION}")
    if page_size not in PAGE_SIZES:
        raise BootImageError(f"page size {page_size} is not one of {PAGE_SIZES}")
    offset = page_size
    sections: list[bytes] = []
    for field, size in (
        ("kernel", kernel_size),
        ("ramdisk", ramdisk_size),
        ("second", second_size),
    ):
        if offset + size > len(data):
            raise BootImageError(
                f"the {field} ends at byte {offset + size}, past the {len(data)}-byte input"
            )
        sections.append(data[offset : offset + size])
        offset += pages(size, page_size)
    kernel, ramdisk, second = sections
    return BootImage(
        Header(
            kernel_addr,
            ramdisk_addr,
            second_addr,
            tags_addr,
            page_size,
            os_version,
            decoded(name, "name"),
            decoded(cmdline, "cmdline"),
            decoded(extra_cmdline, "extra_cmdline"),
        ),
        kernel,
        ramdisk,
        second,
    )


def unpack(data: bytes) -> BootImage:
    image = parse(data)
    built = build(image)
    stored = data[ID_AT : ID_AT + ID]
    if stored != identity(image):
        raise BootImageError(f"the stored id {stored.hex()} is not its sections' sha1")
    if data[: len(built)] != built:
        raise BootImageError("the image has non-zero bytes no header field or section holds")
    return image


def number(record: dict[str, object], name: str) -> int:
    value = record[name]
    if not isinstance(value, int) or isinstance(value, bool):
        raise BootImageError(f"{name} in {FIELDS} is not an integer")
    return value


def text(record: dict[str, object], name: str) -> str:
    value = record[name]
    if not isinstance(value, str):
        raise BootImageError(f"{name} in {FIELDS} is not a string")
    return value


def header_from(record: object) -> Header:
    names = sorted(field.name for field in fields(Header))
    if not isinstance(record, dict) or sorted(record) != names:
        raise BootImageError(f"{FIELDS} is not an object with exactly {', '.join(names)}")
    return Header(
        number(record, "kernel_addr"),
        number(record, "ramdisk_addr"),
        number(record, "second_addr"),
        number(record, "tags_addr"),
        number(record, "page_size"),
        number(record, "os_version"),
        text(record, "name"),
        text(record, "cmdline"),
        text(record, "extra_cmdline"),
    )


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def unpack_to(source: Path, directory: Path) -> None:
    data = source.read_bytes()
    image = unpack(data)
    size = len(build(image))
    directory.mkdir(parents=True)
    for field, section in image.sections().items():
        if section:
            (directory / field).write_bytes(section)
            print(f"{field}: {len(section)} bytes")
    (directory / FIELDS).write_text(json.dumps(asdict(image.header), indent=2) + "\n")
    trailing = data[size:]
    print(
        f"image {source}: header v{VERSION}, page size {image.header.page_size}, "
        f"{size} bytes, sha256 {sha256(data[:size])}"
    )
    print(f"id: {identity(image)[:20].hex()}, the sha1 of its sections")
    print(f"trailing: {len(trailing)} bytes, {len(trailing) - trailing.count(0)} of them non-zero")


def repack_from(directory: Path, target: Path) -> None:
    try:
        record: object = json.loads((directory / FIELDS).read_text())
    except json.JSONDecodeError as error:
        raise BootImageError(f"{FIELDS} is not JSON: {error}") from error
    sections = {
        field: (directory / field).read_bytes() if (directory / field).is_file() else b""
        for field in ("kernel", "ramdisk", "second")
    }
    data = build(BootImage(header_from(record), **sections))
    with target.open("xb") as stream:
        stream.write(data)
    print(f"image {target}: {len(data)} bytes, sha256 {sha256(data)}")


def parser() -> argparse.ArgumentParser:
    found = argparse.ArgumentParser(prog="boot_image.py")
    commands = found.add_subparsers(dest="command", required=True)
    unpacking = commands.add_parser("unpack")
    unpacking.add_argument("image", type=Path)
    unpacking.add_argument("directory", type=Path)
    repacking = commands.add_parser("repack")
    repacking.add_argument("directory", type=Path)
    repacking.add_argument("image", type=Path)
    return found


def main(argv: list[str]) -> int:
    options = parser().parse_args(argv[1:])
    image: Path = options.image
    directory: Path = options.directory
    try:
        if options.command == "unpack":
            unpack_to(image, directory)
        else:
            repack_from(directory, image)
    except (BootImageError, OSError) as error:
        print(error, file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
