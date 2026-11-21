from __future__ import annotations

import argparse
import subprocess
import sys
import xml.etree.ElementTree as ElementTree
from collections.abc import Callable, Mapping
from dataclasses import dataclass
from pathlib import Path, PurePosixPath
from typing import Final

DEBUGFS: Final = Path("/opt/homebrew/opt/e2fsprogs/sbin/debugfs")
IMAGE: Final = Path.home() / "p20-backups" / "odm-mmcblk0p44-2026-11-21.img"
MIXER_PATHS: Final = PurePosixPath("/etc/audio/hissc/mixer_paths_anne.xml")
ROUTE: Final = "media-speaker"
OUT: Final = Path("out/speaker-route.tsv")
DEFAULT: Final = "default"
SPEAKER: Final = "speaker"
TIMEOUT: Final = 30.0


class RouteError(RuntimeError):
    pass


@dataclass(frozen=True, slots=True)
class Control:
    name: str
    value: str


Reader = Callable[[Path, PurePosixPath], bytes]


def debugfs_reader(binary: Path = DEBUGFS) -> Reader:
    def read(image: Path, path: PurePosixPath) -> bytes:
        if not binary.is_file():
            raise RouteError(f"{binary} is missing: brew install e2fsprogs")
        if not image.is_file():
            raise RouteError(f"{image} is missing")
        try:
            result = subprocess.run(
                [str(binary), "-R", f"cat {path}", str(image)],
                capture_output=True,
                timeout=TIMEOUT,
                check=False,
            )
        except subprocess.TimeoutExpired as error:
            raise RouteError(f"{binary} took over {TIMEOUT:g} s on {image}") from error
        if result.returncode != 0 or not result.stdout:
            said = result.stderr.decode(errors="replace").strip().splitlines()
            raise RouteError(f"{path} does not read out of {image}: {said[-1] if said else ''}")
        return result.stdout

    return read


def control(element: ElementTree.Element) -> Control:
    name, value = element.get("name"), element.get("value")
    if name is None or value is None or "\t" in name + value or "\n" in name + value:
        raise RouteError(f"a ctl without a plain name and value: {element.attrib}")
    return Control(name, value)


def parsed(xml: bytes) -> tuple[list[Control], dict[str, ElementTree.Element]]:
    try:
        root = ElementTree.fromstring(xml)
    except ElementTree.ParseError as error:
        raise RouteError(f"the mixer paths do not parse: {error}") from error
    if root.tag != "mixer":
        raise RouteError(f"the root is <{root.tag}>, not <mixer>")
    defaults = [control(element) for element in root.findall("ctl")]
    paths: dict[str, ElementTree.Element] = {}
    for element in root.findall("path"):
        name = element.get("name")
        if name is None or name in paths:
            raise RouteError(f"a top-level path named {name!r} twice or not at all")
        paths[name] = element
    return defaults, paths


def expanded(
    name: str, paths: Mapping[str, ElementTree.Element], within: tuple[str, ...] = ()
) -> list[Control]:
    if name in within:
        raise RouteError(f"path {name} includes itself through {' > '.join(within)}")
    element = paths.get(name)
    if element is None:
        raise RouteError(f"no path named {name}")
    found: list[Control] = []
    for child in element:
        if child.tag == "ctl":
            found.append(control(child))
        elif child.tag == "path":
            included = child.get("name")
            if included is None:
                raise RouteError(f"path {name} includes a path with no name")
            found.extend(expanded(included, paths, (*within, name)))
        else:
            raise RouteError(f"path {name} holds a <{child.tag}>")
    return found


def route(xml: bytes, name: str = ROUTE) -> str:
    defaults, paths = parsed(xml)
    speaker = expanded(name, paths)
    if not defaults or not speaker:
        raise RouteError(f"{len(defaults)} defaults and {len(speaker)} controls in {name}")
    return "".join(
        [f"{DEFAULT}\t{each.name}\t{each.value}\n" for each in defaults]
        + [f"{SPEAKER}\t{each.name}\t{each.value}\n" for each in speaker]
    )


def parser() -> argparse.ArgumentParser:
    found = argparse.ArgumentParser(prog="speaker_route.py")
    found.add_argument("--image", type=Path, default=IMAGE)
    found.add_argument("--out", type=Path, default=OUT)
    return found


def main(argv: list[str], reader: Reader | None = None) -> int:
    options = parser().parse_args(argv[1:])
    image: Path = options.image
    out: Path = options.out
    try:
        written = route((reader or debugfs_reader())(image, MIXER_PATHS))
    except RouteError as error:
        print(error, file=sys.stderr)
        return 1
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(written)
    lines = written.splitlines()
    speaker = sum(1 for line in lines if line.startswith(f"{SPEAKER}\t"))
    print(f"{out}: {len(lines) - speaker} defaults and {speaker} {ROUTE} controls")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
