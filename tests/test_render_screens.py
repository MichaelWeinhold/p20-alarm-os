from __future__ import annotations

import struct
import zlib
from pathlib import Path

import pytest

from render_screens import (
    FIXTURES,
    HASHES,
    HEADER,
    PREVIEW,
    SCREENS,
    SIGNATURE,
    Image,
    RenderError,
    images,
    main,
    manifest,
    png,
    stale,
)

ROOT = Path(__file__).resolve().parents[1]


def raw(image: Image) -> bytes:
    return HEADER.pack(image.width, image.height) + image.pixels


def checkerboard(width: int, height: int) -> Image:
    pixels = b"".join(
        bytes([255, 0, 0, 255]) if (x + y) % 2 else bytes([0, 0, 0, 0])
        for y in range(height)
        for x in range(width)
    )
    return Image(width, height, pixels)


def chunks(data: bytes) -> list[tuple[bytes, bytes]]:
    found: list[tuple[bytes, bytes]] = []
    offset = len(SIGNATURE)
    while offset < len(data):
        (length,) = struct.unpack_from(">I", data, offset)
        kind = data[offset + 4 : offset + 8]
        body = data[offset + 8 : offset + 8 + length]
        (crc,) = struct.unpack_from(">I", data, offset + 8 + length)
        assert crc == zlib.crc32(kind + body)
        found.append((kind, body))
        offset += 12 + length
    return found


def fixtures(root: Path) -> None:
    for index, fixture in enumerate(FIXTURES):
        (root / fixture).parent.mkdir(parents=True, exist_ok=True)
        (root / fixture).write_text(f"fixture {index}\n")


def test_the_committed_screens_hash_the_fixtures_they_were_rendered_from() -> None:
    assert stale(ROOT) == []


def test_a_png_holds_each_row_unfiltered_under_a_valid_header() -> None:
    image = checkerboard(3, 2)
    encoded = png(image)
    assert encoded.startswith(SIGNATURE)
    found = chunks(encoded)
    assert [kind for kind, _ in found] == [b"IHDR", b"IDAT", b"IEND"]
    assert found[0][1] == struct.pack(">IIBBBBB", 3, 2, 8, 6, 0, 0, 0)
    rows = zlib.decompress(found[1][1])
    assert rows == b"\x00" + image.pixels[:12] + b"\x00" + image.pixels[12:]


def test_the_example_output_splits_into_its_images() -> None:
    one = checkerboard(2, 3)
    other = checkerboard(4, 1)
    assert images(raw(one) + raw(other)) == [one, other]


@pytest.mark.parametrize("cut", [1, HEADER.size + 1])
def test_a_truncated_output_is_refused(cut: int) -> None:
    with pytest.raises(RenderError):
        images(raw(checkerboard(2, 2))[:-cut])


def test_matching_hashes_are_not_stale(tmp_path: Path) -> None:
    fixtures(tmp_path)
    (tmp_path / HASHES).parent.mkdir(parents=True)
    (tmp_path / HASHES).write_text(manifest(tmp_path))
    assert stale(tmp_path) == []


def test_a_rerecorded_fixture_makes_the_screens_stale(tmp_path: Path) -> None:
    fixtures(tmp_path)
    (tmp_path / HASHES).parent.mkdir(parents=True)
    (tmp_path / HASHES).write_text(manifest(tmp_path))
    (tmp_path / FIXTURES[1]).write_text("re-recorded\n")
    assert stale(tmp_path) == [f"{FIXTURES[1].as_posix()} changed since the screens were rendered"]


def test_a_missing_hash_file_makes_the_screens_stale(tmp_path: Path) -> None:
    fixtures(tmp_path)
    assert stale(tmp_path) == [f"{HASHES} is missing; run scripts/render_screens.py"]


def test_a_hash_file_without_a_fixture_or_with_another_is_stale(tmp_path: Path) -> None:
    fixtures(tmp_path)
    (tmp_path / HASHES).parent.mkdir(parents=True)
    lines = manifest(tmp_path).splitlines()
    (tmp_path / HASHES).write_text("\n".join([*lines[1:], "00  other.txt"]) + "\n")
    assert stale(tmp_path) == [
        f"{HASHES} holds no hash for {FIXTURES[0].as_posix()}",
        f"{HASHES} names other.txt, which the screens do not use",
    ]


def test_a_malformed_hash_line_is_refused(tmp_path: Path) -> None:
    fixtures(tmp_path)
    (tmp_path / HASHES).parent.mkdir(parents=True)
    (tmp_path / HASHES).write_text("not a line\n")
    with pytest.raises(RenderError):
        stale(tmp_path)


def test_main_writes_both_pictures_and_the_hashes(tmp_path: Path) -> None:
    fixtures(tmp_path)
    screens = checkerboard(3, 2)
    preview = checkerboard(2, 1)
    assert main(["render_screens.py", str(tmp_path)], lambda _: raw(screens) + raw(preview)) == 0
    assert (tmp_path / SCREENS).read_bytes() == png(screens)
    assert (tmp_path / PREVIEW).read_bytes() == png(preview)
    assert stale(tmp_path) == []


def test_main_refuses_an_output_without_two_images(tmp_path: Path) -> None:
    fixtures(tmp_path)
    assert main(["render_screens.py", str(tmp_path)], lambda _: raw(checkerboard(1, 1))) == 1
    assert not (tmp_path / HASHES).exists()
