from __future__ import annotations

from pathlib import Path, PurePosixPath

import pytest

from speaker_route import MIXER_PATHS, RouteError, debugfs_reader, main, route

MIXER: bytes = b"""<mixer>
  <!-- These are commonly used control sequences -->
  <ctl name="HISSC_DACL_HBF1I_EN" value="0" />
  <ctl name="STEREO_DLINK_TEST_SEL MUX" value="ZERO"  />
  <ctl name="HISSC_CLASSD_VOLTAGE_CONFIG" value="0" />
  <path name="init" >
      <ctl name="HISSC_FS_DACLR" value="4" />
  </path>
  <path name="audio-play-nossc" >
      <ctl name="STEREO_DLINK_TEST_SEL MUX" value="APB" />
  </path>
  <path name="speaker" >
      <ctl name="CLASSD VIR_SWITCH CLASSD_EN" value="1" />
  </path>
  <path name="earpiece" >
      <ctl name="SMT_MIXOUT_EAR_PU MIXER DACL" value="1" />
  </path>
  <path name="media-speaker" >
      <path name="init" />
      <path name="audio-play-nossc" />
      <path name="speaker" />
  </path>
</mixer>
"""


def test_the_defaults_come_first_then_the_route_in_its_order() -> None:
    assert route(MIXER).splitlines() == [
        "default\tHISSC_DACL_HBF1I_EN\t0",
        "default\tSTEREO_DLINK_TEST_SEL MUX\tZERO",
        "default\tHISSC_CLASSD_VOLTAGE_CONFIG\t0",
        "speaker\tHISSC_FS_DACLR\t4",
        "speaker\tSTEREO_DLINK_TEST_SEL MUX\tAPB",
        "speaker\tCLASSD VIR_SWITCH CLASSD_EN\t1",
    ]


def test_a_path_the_route_does_not_include_is_left_out() -> None:
    assert "SMT_MIXOUT_EAR_PU" not in route(MIXER)


def test_a_route_nested_two_deep_is_expanded() -> None:
    nested = MIXER.replace(
        b"</mixer>", b'<path name="mmi"><path name="media-speaker" /></path></mixer>'
    )
    assert route(nested, "mmi") == route(MIXER)


@pytest.mark.parametrize(
    ("xml", "reason"),
    [
        (MIXER.replace(b'<path name="speaker" />', b'<path name="spaeker" />'), "no path named"),
        (MIXER.replace(b'<path name="init" />', b'<path name="media-speaker" />'), "itself"),
        (MIXER.replace(b'value="APB" ', b""), "without a plain name and value"),
        (MIXER.replace(b"<mixer>", b"<mixers>").replace(b"</mixer>", b"</mixers>"), "<mixers>"),
        (MIXER[:-20], "do not parse"),
    ],
)
def test_mixer_paths_that_do_not_make_a_route_are_refused(xml: bytes, reason: str) -> None:
    with pytest.raises(RouteError, match=reason):
        route(xml)


def test_main_writes_the_route_read_from_the_image(
    tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    asked: list[tuple[Path, PurePosixPath]] = []

    def read(image: Path, path: PurePosixPath) -> bytes:
        asked.append((image, path))
        return MIXER

    image = tmp_path / "odm.img"
    out = tmp_path / "out" / "speaker-route.tsv"
    code = main(["speaker_route.py", "--image", str(image), "--out", str(out)], read)
    assert code == 0
    assert asked == [(image, MIXER_PATHS)]
    assert out.read_text() == route(MIXER)
    assert "3 defaults and 3 media-speaker controls" in capsys.readouterr().out


def test_a_missing_debugfs_or_image_is_named(tmp_path: Path) -> None:
    with pytest.raises(RouteError, match="brew install e2fsprogs"):
        debugfs_reader(tmp_path / "debugfs")(tmp_path / "odm.img", MIXER_PATHS)
    debugfs = tmp_path / "debugfs"
    debugfs.write_bytes(b"")
    with pytest.raises(RouteError, match=r"odm\.img is missing"):
        debugfs_reader(debugfs)(tmp_path / "odm.img", MIXER_PATHS)
