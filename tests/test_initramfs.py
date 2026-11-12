from __future__ import annotations

import gzip
import struct
from dataclasses import dataclass
from pathlib import Path

import pytest

from boot_image import BootImage, Header, build, parse
from initramfs import main, ramdisk

CMDLINE = "loglevel=4 coherent_pool=512K androidboot.selinux=enforcing buildvariant=user"


@dataclass(frozen=True, slots=True)
class Member:
    name: str
    mode: int
    links: int
    rdev: tuple[int, int]
    data: bytes


def elf(machine: int = 183) -> bytes:
    header = b"\x7fELF\x02\x01\x01" + b"\0" * 9 + struct.pack("<HH", 2, machine)
    return header + b"\x90" * 1001


def members(compressed: bytes) -> list[Member]:
    data = gzip.decompress(compressed)
    found: list[Member] = []
    offset = 0
    while True:
        assert data[offset : offset + 6] == b"070701"
        values = [int(data[at : at + 8], 16) for at in range(offset + 6, offset + 110, 8)]
        _, mode, uid, gid, links, mtime, size, _, _, major, minor, name_size, check = values
        assert (uid, gid, mtime, check) == (0, 0, 0, 0)
        name_at = offset + 110
        name = data[name_at : name_at + name_size - 1].decode("ascii")
        assert data[name_at + name_size - 1] == 0
        data_at = name_at + name_size + (-(110 + name_size) % 4)
        content = data[data_at : data_at + size]
        offset = data_at + size + (-size % 4)
        if name == "TRAILER!!!":
            assert offset == len(data)
            return found
        found.append(Member(name, mode, links, (major, minor), content))


def stock(path: Path) -> Path:
    header = Header(0x480000, 0x10400000, 0x1300000, 0x7E00000, 2048, 0x12000000, "", CMDLINE, "")
    image = build(BootImage(header, b"\x1f\x8b" + b"k" * 5000, b"", b""))
    path.write_bytes(image + b"\x55" * 4096)
    return path


def test_the_ramdisk_holds_the_init_and_the_nodes_std_needs() -> None:
    init = elf()
    assert members(ramdisk(init)) == [
        Member("dev", 0o040755, 2, (0, 0), b""),
        Member("dev/console", 0o020600, 1, (5, 1), b""),
        Member("dev/null", 0o020666, 1, (1, 3), b""),
        Member("proc", 0o040755, 2, (0, 0), b""),
        Member("init", 0o100755, 1, (0, 0), init),
    ]


def test_the_ramdisk_is_the_same_bytes_every_time() -> None:
    assert ramdisk(elf()) == ramdisk(elf())


def test_the_image_is_the_stock_image_with_the_ramdisk_added(
    tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    init = tmp_path / "init"
    init.write_bytes(elf())
    source = stock(tmp_path / "stock.img")
    target = tmp_path / "probe.img"
    assert main(["initramfs.py", str(init), str(source), str(target)]) == 0
    before = parse(source.read_bytes())
    after = parse(target.read_bytes())
    assert after.header == before.header
    assert after.kernel == before.kernel
    assert after.second == b""
    assert after.ramdisk == ramdisk(elf())
    assert len(target.read_bytes()) % 2048 == 0
    printed = capsys.readouterr().out
    assert f"image {target}: {len(target.read_bytes())} bytes, sha256 " in printed


@pytest.mark.parametrize(
    ("init", "reason"),
    [
        (b"#!/bin/sh\nexit 0\n", "is not a 64-bit little-endian ELF"),
        (elf(machine=62), "ELF machine 62, not aarch64"),
    ],
)
def test_an_init_that_cannot_run_on_the_phone_is_refused(
    tmp_path: Path, capsys: pytest.CaptureFixture[str], init: bytes, reason: str
) -> None:
    path = tmp_path / "init"
    path.write_bytes(init)
    target = tmp_path / "probe.img"
    assert main(["initramfs.py", str(path), str(stock(tmp_path / "stock.img")), str(target)]) == 1
    assert reason in capsys.readouterr().err
    assert not target.exists()


def test_an_existing_image_is_never_overwritten(
    tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    init = tmp_path / "init"
    init.write_bytes(elf())
    target = tmp_path / "probe.img"
    target.write_bytes(b"kept")
    assert main(["initramfs.py", str(init), str(stock(tmp_path / "stock.img")), str(target)]) == 1
    assert "exists" in capsys.readouterr().err
    assert target.read_bytes() == b"kept"
