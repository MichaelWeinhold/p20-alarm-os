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
ICONS: Final[Mapping[str, int]] = {
    "alarm": 0xE855,
    "hourglass_empty": 0xE88B,
    "settings": 0xE8B8,
    "battery_android_0": 0xF30D,
    "power_settings_new": 0xF8C7,
    "light_mode": 0xE518,
    "add": 0xE145,
    "close": 0xE5CD,
    "keyboard_arrow_up": 0xE316,
    "keyboard_arrow_down": 0xE313,
    "play_arrow": 0xE037,
    "stop": 0xE047,
    "toggle_on": 0xE9F6,
    "toggle_off": 0xE9F5,
}
SYMBOLS: Final = Subset(
    "material-symbols-light-icons.ttf",
    {"FILL": 0, "GRAD": 0, "opsz": 48, "wght": 300},
    "".join(map(chr, ICONS.values())),
)


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
        listed = ", ".join(f"U+{point:04X}" for point in absent)
        raise FontError(f"{source} has no glyph for {listed}")
    out = io.BytesIO()
    instance.save(out)
    return out.getvalue()


def parser() -> argparse.ArgumentParser:
    found = argparse.ArgumentParser(prog="subset_fonts.py")
    found.add_argument("--out", type=Path, default=OUT)
    found.add_argument("inter", type=Path)
    found.add_argument("symbols", type=Path)
    return found


def main(argv: list[str]) -> int:
    options = parser().parse_args(argv[1:])
    out: Path = options.out
    inter: Path = options.inter
    symbols: Path = options.symbols
    try:
        made = [
            (out / wanted.name, subset(source, wanted))
            for source, wanted in ((inter, INTER), (symbols, SYMBOLS))
        ]
        for target, data in made:
            target.write_bytes(data)
    except (FontError, OSError) as error:
        print(error, file=sys.stderr)
        return 1
    for target, data in made:
        print(f"{target}: {len(data)} bytes")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
