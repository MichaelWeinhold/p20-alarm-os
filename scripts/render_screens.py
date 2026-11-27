from __future__ import annotations

import hashlib
import struct
import subprocess
import sys
import zlib
from collections.abc import Callable
from dataclasses import dataclass
from pathlib import Path
from typing import Final

CARGO: Final = "cargo"
EXAMPLE: Final = ["run", "--quiet", "--release", "--locked", "-p", "alarm-core"]
IMAGES: Final = Path("docs/images")
SCREENS: Final = IMAGES / "screens.png"
PREVIEW: Final = IMAGES / "social-preview.png"
HASHES: Final = IMAGES / "screens.sha256"
TESTDATA: Final = Path("crates/alarm-core/testdata")
FIXTURES: Final = tuple(
    TESTDATA / name for name in ("face-sheet.txt", "face-timer.txt", "face-settings.txt")
)
SIGNATURE: Final = b"\x89PNG\r\n\x1a\n"
HEADER: Final = struct.Struct("<II")


@dataclass(frozen=True, slots=True)
class Image:
    width: int
    height: int
    pixels: bytes


class RenderError(RuntimeError):
    pass


Rendering = Callable[[Path], bytes]


def from_cargo(root: Path) -> bytes:
    result = subprocess.run(
        [CARGO, *EXAMPLE, "--example", "screens"],
        cwd=root,
        capture_output=True,
        check=False,
    )
    if result.returncode != 0:
        message = result.stderr.decode(errors="replace").strip()
        raise RenderError(message or f"the screens example exited {result.returncode}")
    return result.stdout


def images(data: bytes) -> list[Image]:
    found: list[Image] = []
    offset = 0
    while offset < len(data):
        if len(data) - offset < HEADER.size:
            raise RenderError(f"{len(data) - offset} bytes left where a size belongs")
        width, height = HEADER.unpack_from(data, offset)
        offset += HEADER.size
        length = width * height * 4
        if len(data) - offset < length:
            left = len(data) - offset
            raise RenderError(f"a {width}x{height} image has {left} of {length} bytes")
        found.append(Image(width, height, data[offset : offset + length]))
        offset += length
    return found


def chunk(kind: bytes, body: bytes) -> bytes:
    return struct.pack(">I", len(body)) + kind + body + struct.pack(">I", zlib.crc32(kind + body))


def png(image: Image) -> bytes:
    stride = image.width * 4
    rows = b"".join(
        b"\x00" + image.pixels[row * stride : (row + 1) * stride] for row in range(image.height)
    )
    header = struct.pack(">IIBBBBB", image.width, image.height, 8, 6, 0, 0, 0)
    return (
        SIGNATURE
        + chunk(b"IHDR", header)
        + chunk(b"IDAT", zlib.compress(rows, 9))
        + chunk(b"IEND", b"")
    )


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def manifest(root: Path) -> str:
    return "".join(f"{digest(root / fixture)}  {fixture.as_posix()}\n" for fixture in FIXTURES)


def recorded(text: str) -> dict[str, str]:
    found: dict[str, str] = {}
    for line in text.splitlines():
        hashed, separator, path = line.partition("  ")
        if not separator or not path:
            raise RenderError(f"{HASHES}: {line!r} is not '<sha256>  <path>'")
        found[path] = hashed
    return found


def stale(root: Path) -> list[str]:
    hashes = root / HASHES
    if not hashes.is_file():
        return [f"{HASHES} is missing; run scripts/render_screens.py"]
    held = recorded(hashes.read_text())
    wanted = {fixture.as_posix() for fixture in FIXTURES}
    found = [
        f"{HASHES} names {path}, which the screens do not use" for path in held.keys() - wanted
    ]
    for fixture in FIXTURES:
        path = fixture.as_posix()
        if path not in held:
            found.append(f"{HASHES} holds no hash for {path}")
        elif held[path] != digest(root / fixture):
            found.append(f"{path} changed since the screens were rendered")
    return sorted(found)


def main(argv: list[str], rendering: Rendering = from_cargo) -> int:
    root = Path(argv[1]).resolve() if len(argv) > 1 else Path.cwd()
    try:
        rendered = images(rendering(root))
    except RenderError as error:
        print(error, file=sys.stderr)
        return 1
    if len(rendered) != 2:
        print(f"the screens example wrote {len(rendered)} images, not 2", file=sys.stderr)
        return 1
    (root / IMAGES).mkdir(parents=True, exist_ok=True)
    for path, image in zip((SCREENS, PREVIEW), rendered, strict=True):
        (root / path).write_bytes(png(image))
        print(f"{path}: {image.width}x{image.height}")
    (root / HASHES).write_text(manifest(root))
    print(f"{HASHES}: {len(FIXTURES)} fixtures")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
