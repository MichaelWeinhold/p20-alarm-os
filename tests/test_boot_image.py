from __future__ import annotations

import hashlib
import json
import struct
from pathlib import Path

import pytest

from boot_image import FIELDS, main

CMDLINE = "loglevel=4 coherent_pool=512K androidboot.selinux=enforcing buildvariant=user"


def page(data: bytes, page_size: int) -> bytes:
    return data + b"\0" * (-len(data) % page_size)


def synthetic(
    kernel: bytes,
    ramdisk: bytes = b"",
    second: bytes = b"",
    page_size: int = 2048,
    version: int = 0,
    magic: bytes = b"ANDROID!",
    stored_id: bytes | None = None,
) -> bytes:
    sha = hashlib.sha1(usedforsecurity=False)
    for section in (kernel, ramdisk, second):
        sha.update(section)
        sha.update(struct.pack("<I", len(section)))
    header = struct.pack(
        "<8s10I16s512s32s1024s",
        magic,
        len(kernel),
        0x480000,
        len(ramdisk),
        0x10400000,
        len(second),
        0x1300000,
        0x7E00000,
        page_size,
        version,
        0x12000000,
        b"",
        CMDLINE.encode(),
        sha.digest() if stored_id is None else stored_id,
        b"",
    )
    return b"".join(page(part, page_size) for part in (header, kernel, ramdisk, second) if part)


def gzip_like(size: int) -> bytes:
    return b"\x1f\x8b" + bytes(index * 7 % 251 for index in range(size - 2))


def round_trip(tmp_path: Path, data: bytes) -> bytes:
    source = tmp_path / "in.img"
    source.write_bytes(data)
    assert main(["boot_image.py", "unpack", str(source), str(tmp_path / "out")]) == 0
    assert main(["boot_image.py", "repack", str(tmp_path / "out"), str(tmp_path / "re.img")]) == 0
    return (tmp_path / "re.img").read_bytes()


@pytest.mark.parametrize("page_size", [2048, 4096])
@pytest.mark.parametrize(
    ("kernel", "ramdisk", "second"),
    [
        (gzip_like(12345), b"", b""),
        (gzip_like(12345), gzip_like(3000), b""),
        (gzip_like(4096), gzip_like(2048), gzip_like(77)),
    ],
    ids=["kernel", "ramdisk", "second"],
)
def test_unpack_then_repack_reproduces_the_image_byte_for_byte(
    tmp_path: Path, page_size: int, kernel: bytes, ramdisk: bytes, second: bytes
) -> None:
    data = synthetic(kernel, ramdisk, second, page_size)
    assert round_trip(tmp_path, data) == data


def test_unpack_writes_each_present_section_and_the_header_fields(tmp_path: Path) -> None:
    kernel = gzip_like(5000)
    source = tmp_path / "in.img"
    source.write_bytes(synthetic(kernel))
    assert main(["boot_image.py", "unpack", str(source), str(tmp_path / "out")]) == 0
    assert sorted(path.name for path in (tmp_path / "out").iterdir()) == [FIELDS, "kernel"]
    assert (tmp_path / "out" / "kernel").read_bytes() == kernel
    assert json.loads((tmp_path / "out" / FIELDS).read_text()) == {
        "kernel_addr": 0x480000,
        "ramdisk_addr": 0x10400000,
        "second_addr": 0x1300000,
        "tags_addr": 0x7E00000,
        "page_size": 2048,
        "os_version": 0x12000000,
        "name": "",
        "cmdline": CMDLINE,
        "extra_cmdline": "",
    }


def test_a_partition_dump_round_trips_its_image_and_counts_what_trails_it(
    tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    data = synthetic(gzip_like(9000))
    trailing = b"\xaa" * 100 + b"\0" * 8000
    assert round_trip(tmp_path, data + trailing) == data
    out = capsys.readouterr().out
    assert out.count(hashlib.sha256(data).hexdigest()) == 2
    assert f"{len(data)} bytes" in out
    assert "trailing: 8100 bytes, 100 of them non-zero" in out


def test_an_added_ramdisk_repacks_as_mkbootimg_would_build_it(tmp_path: Path) -> None:
    kernel = gzip_like(6000)
    source = tmp_path / "in.img"
    source.write_bytes(synthetic(kernel))
    assert main(["boot_image.py", "unpack", str(source), str(tmp_path / "out")]) == 0
    (tmp_path / "out" / "ramdisk").write_bytes(gzip_like(1000))
    assert main(["boot_image.py", "repack", str(tmp_path / "out"), str(tmp_path / "re.img")]) == 0
    assert (tmp_path / "re.img").read_bytes() == synthetic(kernel, gzip_like(1000))


@pytest.mark.parametrize(
    ("data", "reason"),
    [
        (b"ANDROID!" + b"\0" * 100, "shorter than a 1632-byte header"),
        (synthetic(gzip_like(3000), magic=b"ANDROIX!"), "the magic is b'ANDROIX!'"),
        (synthetic(gzip_like(3000), version=1), "header version 1 is not 0"),
        (synthetic(gzip_like(3000), page_size=1024), "page size 1024 is not one of"),
        (synthetic(gzip_like(3000))[:4000], "the kernel ends at byte 5048, past the 4000-byte"),
        (synthetic(gzip_like(3000), stored_id=b"\1" * 32), "the stored id 0101"),
        (synthetic(b""), "the kernel is empty"),
    ],
    ids=["short", "magic", "version", "page", "truncated", "id", "empty"],
)
def test_unpack_refuses_what_is_not_a_well_formed_v0_image(
    tmp_path: Path, capsys: pytest.CaptureFixture[str], data: bytes, reason: str
) -> None:
    source = tmp_path / "in.img"
    source.write_bytes(data)
    assert main(["boot_image.py", "unpack", str(source), str(tmp_path / "out")]) == 1
    assert reason in capsys.readouterr().err
    assert not (tmp_path / "out").exists()


@pytest.mark.parametrize(
    "offset",
    [64 + len(CMDLINE) + 1, 1700, 2048 + 3000],
    ids=["cmdline", "header", "kernel"],
)
def test_unpack_refuses_non_zero_bytes_a_repack_would_drop(
    tmp_path: Path, capsys: pytest.CaptureFixture[str], offset: int
) -> None:
    data = bytearray(synthetic(gzip_like(3000)))
    data[offset] = 0x55
    source = tmp_path / "in.img"
    source.write_bytes(bytes(data))
    assert main(["boot_image.py", "unpack", str(source), str(tmp_path / "out")]) == 1
    assert "non-zero bytes no header field or section holds" in capsys.readouterr().err


def test_unpack_refuses_a_directory_that_exists(
    tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    source = tmp_path / "in.img"
    source.write_bytes(synthetic(gzip_like(3000)))
    (tmp_path / "out").mkdir()
    assert main(["boot_image.py", "unpack", str(source), str(tmp_path / "out")]) == 1
    assert "File exists" in capsys.readouterr().err


def unpacked(tmp_path: Path) -> Path:
    source = tmp_path / "in.img"
    source.write_bytes(synthetic(gzip_like(3000)))
    assert main(["boot_image.py", "unpack", str(source), str(tmp_path / "out")]) == 0
    return tmp_path / "out"


def test_repack_never_overwrites_a_file(tmp_path: Path, capsys: pytest.CaptureFixture[str]) -> None:
    directory = unpacked(tmp_path)
    target = tmp_path / "backup.img"
    target.write_bytes(b"stock")
    assert main(["boot_image.py", "repack", str(directory), str(target)]) == 1
    assert target.read_bytes() == b"stock"
    assert "File exists" in capsys.readouterr().err


@pytest.mark.parametrize(
    ("change", "reason"),
    [
        ({"page_size": "2048"}, "page_size in header.json is not an integer"),
        ({"os_version": True}, "os_version in header.json is not an integer"),
        ({"cmdline": 7}, "cmdline in header.json is not a string"),
        ({"name": "x" * 17}, "name does not fit its 16-byte field"),
        ({"cmdline": "é"}, "cmdline is not ASCII"),
        ({"tags_addr": 1 << 32}, "does not fit 32 bits"),
        ({"page_size": 3000}, "page size 3000 is not one of"),
        ({"board": ""}, "is not an object with exactly"),
    ],
    ids=["page", "bool", "cmdline", "long", "ascii", "wide", "size", "extra"],
)
def test_repack_refuses_header_fields_it_cannot_write(
    tmp_path: Path, capsys: pytest.CaptureFixture[str], change: dict[str, object], reason: str
) -> None:
    directory = unpacked(tmp_path)
    record = json.loads((directory / FIELDS).read_text())
    record.update(change)
    (directory / FIELDS).write_text(json.dumps(record))
    assert main(["boot_image.py", "repack", str(directory), str(tmp_path / "re.img")]) == 1
    assert reason in capsys.readouterr().err
    assert not (tmp_path / "re.img").exists()


def test_repack_refuses_a_header_file_that_is_not_json(
    tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    directory = unpacked(tmp_path)
    (directory / FIELDS).write_text("{")
    assert main(["boot_image.py", "repack", str(directory), str(tmp_path / "re.img")]) == 1
    assert "header.json is not JSON" in capsys.readouterr().err
