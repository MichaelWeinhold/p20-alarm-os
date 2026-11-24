from __future__ import annotations

import argparse
import io
import sys
from collections.abc import Mapping
from dataclasses import dataclass
from pathlib import Path
from typing import Final

from fontTools.subset import Options, Subsetter
from fontTools.ttLib import TTFont, TTLibError
from fontTools.varLib.instancer import instantiateVariableFont

OUT: Final = Path("crates/alarm-core/fonts")


@dataclass(frozen=True, slots=True)
class Subset:
    name: str
    axes: Mapping[str, float]
    text: str


INTER: Final = Subset("inter-light-figures.ttf", {"wght": 300, "opsz": 32}, "0123456789:%-")


class FontError(RuntimeError):
    pass


def code_points(text: str) -> set[int]:
    return {ord(character) for character in text}


def mapped(font: object) -> set[int]:
    if not isinstance(font, TTFont):
        raise FontError(f"{type(font).__name__} is not a font")
    table = font["cmap"].getBestCmap()
    if not isinstance(table, dict):
        raise FontError("the font has no Unicode cmap")
    found: set[int] = set()
    for point in table:
        if not isinstance(point, int):
            raise FontError(f"the cmap maps {point!r}, not a code point")
        found.add(point)
    return found


def subset(source: Path, wanted: Subset) -> bytes:
    try:
        font = TTFont(source, recalcTimestamp=False)
    except (TTLibError, OSError) as error:
        raise FontError(f"{source}: {error}") from error
    if "fvar" not in font:
        raise FontError(f"{source} is not a variable font")
    tags = {axis.axisTag for axis in font["fvar"].axes}
    missing = sorted(set(wanted.axes) - tags)
    if missing:
        raise FontError(f"{source} has no {', '.join(missing)} axis")
    instance = instantiateVariableFont(font, dict(wanted.axes))
    options = Options()
    options.hinting = False
    options.layout_features = []
    subsetter = Subsetter(options)
    subsetter.populate(text=wanted.text)
    subsetter.subset(instance)
    absent = sorted(code_points(wanted.text) - mapped(instance))
    if absent:
        raise FontError(f"{source} has no glyph for {''.join(map(chr, absent))!r}")
    out = io.BytesIO()
    instance.save(out)
    return out.getvalue()


def parser() -> argparse.ArgumentParser:
    found = argparse.ArgumentParser(prog="subset_fonts.py")
    found.add_argument("--out", type=Path, default=OUT)
    found.add_argument("inter", type=Path)
    return found


def main(argv: list[str]) -> int:
    options = parser().parse_args(argv[1:])
    out: Path = options.out
    inter: Path = options.inter
    try:
        made = subset(inter, INTER)
        target = out / INTER.name
        target.write_bytes(made)
    except (FontError, OSError) as error:
        print(error, file=sys.stderr)
        return 1
    print(f"{target}: {len(made)} bytes")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
