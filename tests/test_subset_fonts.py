from __future__ import annotations

import io
from pathlib import Path

import pytest
from fontTools.fontBuilder import FontBuilder
from fontTools.pens.ttGlyphPen import TTGlyphPen
from fontTools.ttLib import TTFont

from subset_fonts import INTER, OUT, FontError, Subset, code_points, main, mapped, subset

COMMITTED = Path(__file__).resolve().parents[1] / OUT / INTER.name
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


def test_the_committed_subset_maps_exactly_the_figures() -> None:
    assert mapped(TTFont(COMMITTED)) == code_points("0123456789:%-")


def test_the_committed_subset_is_an_instance_with_no_hinting_or_layout() -> None:
    font = TTFont(COMMITTED)
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


def test_main_writes_the_subset(tmp_path: Path, capsys: pytest.CaptureFixture[str]) -> None:
    source = variable(tmp_path / "fake.ttf", INTER.text)
    assert main(["subset_fonts.py", "--out", str(tmp_path), str(source)]) == 0
    written = tmp_path / INTER.name
    assert mapped(TTFont(written)) == code_points(INTER.text)
    assert f"{written.stat().st_size} bytes" in capsys.readouterr().out


def test_main_reports_a_file_that_is_not_a_font(
    tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    source = tmp_path / "junk.ttf"
    source.write_bytes(b"not a font")
    assert main(["subset_fonts.py", "--out", str(tmp_path), str(source)]) == 1
    assert str(source) in capsys.readouterr().err
    assert not (tmp_path / INTER.name).exists()
