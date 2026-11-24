from __future__ import annotations

import io
from pathlib import Path

import pytest
from fontTools.fontBuilder import FontBuilder
from fontTools.pens.ttGlyphPen import TTGlyphPen
from fontTools.ttLib import TTFont

from subset_fonts import (
    ICONS,
    INTER,
    OUT,
    SYMBOLS,
    FontError,
    Subset,
    code_points,
    main,
    mapped,
    subset,
)

FONTS = Path(__file__).resolve().parents[1] / OUT
HINTING = ("fpgm", "prep", "cvt ", "hdmx", "VDMX", "LTSH")


def square() -> object:
    pen = TTGlyphPen(None)
    pen.moveTo((0, 0))
    pen.lineTo((0, 700))
    pen.lineTo((500, 700))
    pen.lineTo((500, 0))
    pen.closePath()
    return pen.glyph()


def variable(path: Path, text: str, axes: tuple[str, ...] = ("wght", "opsz")) -> Path:
    names = [".notdef", *(f"g{ord(character)}" for character in text)]
    builder = FontBuilder(1000, isTTF=True)
    builder.setupGlyphOrder(names)
    builder.setupCharacterMap({ord(character): f"g{ord(character)}" for character in text})
    builder.setupGlyf({name: square() for name in names})
    builder.setupHorizontalMetrics({name: (600, 0) for name in names})
    builder.setupHorizontalHeader(ascent=800, descent=-200)
    builder.setupNameTable({"familyName": "Fake", "styleName": "Regular"})
    builder.setupOS2()
    builder.setupPost()
    builder.setupFvar([(tag, 0, 400, 1000, tag) for tag in axes], [])
    builder.save(str(path))
    return path


def loaded(data: bytes) -> TTFont:
    return TTFont(io.BytesIO(data))


def sources(tmp_path: Path) -> list[str]:
    inter = variable(tmp_path / "inter.ttf", INTER.text)
    symbols = variable(tmp_path / "symbols.ttf", SYMBOLS.text, tuple(SYMBOLS.axes))
    return [str(inter), str(symbols)]


def test_the_committed_subset_maps_exactly_the_figures() -> None:
    assert mapped(TTFont(FONTS / INTER.name)) == code_points("0123456789:%-")


def test_the_committed_icons_map_exactly_the_fourteen_symbols() -> None:
    assert len(ICONS) == 14
    assert mapped(TTFont(FONTS / SYMBOLS.name)) == set(ICONS.values())


@pytest.mark.parametrize("wanted", [INTER, SYMBOLS])
def test_the_committed_subset_is_an_instance_with_no_hinting_or_layout(wanted: Subset) -> None:
    font = TTFont(FONTS / wanted.name)
    assert "fvar" not in font
    assert "gvar" not in font
    for table in HINTING:
        assert table not in font
    for table in ("GSUB", "GPOS"):
        if table in font:
            assert font[table].table.FeatureList.FeatureCount == 0


def test_subset_keeps_only_the_wanted_code_points(tmp_path: Path) -> None:
    source = variable(tmp_path / "fake.ttf", "0123456789:%-ab")
    made = loaded(subset(source, INTER))
    assert mapped(made) == code_points(INTER.text)
    assert "fvar" not in made


def test_subset_instances_the_icons_at_their_axes(tmp_path: Path) -> None:
    source = variable(tmp_path / "fake.ttf", SYMBOLS.text + "a", tuple(SYMBOLS.axes))
    made = loaded(subset(source, SYMBOLS))
    assert mapped(made) == set(ICONS.values())
    assert "fvar" not in made


def test_subset_names_the_missing_icon(tmp_path: Path) -> None:
    source = variable(tmp_path / "fake.ttf", SYMBOLS.text[1:], tuple(SYMBOLS.axes))
    with pytest.raises(FontError, match="no glyph for U\\+E855"):
        subset(source, SYMBOLS)


def test_subset_refuses_a_font_missing_a_wanted_glyph(tmp_path: Path) -> None:
    source = variable(tmp_path / "fake.ttf", "0123456789")
    with pytest.raises(FontError, match="no glyph for"):
        subset(source, INTER)


def test_subset_refuses_a_font_missing_an_axis(tmp_path: Path) -> None:
    source = variable(tmp_path / "fake.ttf", INTER.text, ("wght",))
    with pytest.raises(FontError, match="no opsz axis"):
        subset(source, INTER)


def test_subset_refuses_a_static_font(tmp_path: Path) -> None:
    source = variable(tmp_path / "fake.ttf", INTER.text)
    font = TTFont(source)
    del font["fvar"]
    font.save(source)
    with pytest.raises(FontError, match="not a variable font"):
        subset(source, Subset("x.ttf", {}, "0"))


def test_main_writes_both_subsets(tmp_path: Path, capsys: pytest.CaptureFixture[str]) -> None:
    assert main(["subset_fonts.py", "--out", str(tmp_path), *sources(tmp_path)]) == 0
    printed = capsys.readouterr().out
    for wanted in (INTER, SYMBOLS):
        written = tmp_path / wanted.name
        assert mapped(TTFont(written)) == code_points(wanted.text)
        assert f"{written}: {written.stat().st_size} bytes" in printed


def test_main_writes_neither_subset_when_one_fails(
    tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    inter, _ = sources(tmp_path)
    symbols = variable(tmp_path / "short.ttf", SYMBOLS.text[1:], tuple(SYMBOLS.axes))
    assert main(["subset_fonts.py", "--out", str(tmp_path), inter, str(symbols)]) == 1
    assert "U+E855" in capsys.readouterr().err
    assert not (tmp_path / INTER.name).exists()
    assert not (tmp_path / SYMBOLS.name).exists()


def test_main_reports_a_file_that_is_not_a_font(
    tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    source = tmp_path / "junk.ttf"
    source.write_bytes(b"not a font")
    _, symbols = sources(tmp_path)
    assert main(["subset_fonts.py", "--out", str(tmp_path), str(source), symbols]) == 1
    assert str(source) in capsys.readouterr().err
    assert not (tmp_path / INTER.name).exists()
