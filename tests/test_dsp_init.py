from __future__ import annotations

import struct
from pathlib import Path

import pytest

from dsp_init import (
    ASYNC,
    PARAMETERS,
    RECORDER,
    RECORDING,
    SEQUENCE,
    SET_PARA,
    SET_SOURCE,
    SYNC,
    USB,
    WRITE_PARAMS,
    InitError,
    init,
    kept,
    main,
    messages,
)
from system_push import Answer


def entry(request: int, body: bytes, reply: int = 0) -> bytes:
    return struct.pack("<III", request, len(body), reply) + body


IDS = {0: 0xDD53, 4: 0xDD9C, 7: 0xDD96, 8: SET_PARA, 9: SET_PARA, 10: SET_SOURCE, 11: SET_SOURCE}


def body(request: int, index: int) -> bytes:
    if request == WRITE_PARAMS:
        return bytes([index]) * PARAMETERS
    if request == USB:
        return b"usb!" * 4
    return struct.pack("<HHI", IDS.get(index, 0xDD00), 0, index)


def recording(requests: tuple[int, ...] = SEQUENCE, order: tuple[int, ...] = ()) -> bytes:
    return b"".join(
        entry(request, body(request, order[index] if order else index), 8 if request == SYNC else 0)
        for index, request in enumerate(requests)
    )


class Phone:
    def __init__(self, recordings: list[bytes], state: str = "device") -> None:
        self.recordings = recordings
        self.state = state
        self.calls: list[tuple[str, ...]] = []

    def __call__(self, command: tuple[str, ...]) -> Answer:
        self.calls.append(command)
        if command == ("get-state",):
            return Answer(0, f"{self.state}\n".encode(), "")
        if command[0] == "pull":
            Path(command[2]).write_bytes(self.recordings.pop(0))
        return Answer(0, b"", "")

    def shells(self) -> list[str]:
        return [call[1] for call in self.calls if call[0] == "shell"]


def test_a_recording_reads_back_as_its_entries() -> None:
    found = messages(entry(SYNC, b"abcd", 8) + entry(ASYNC, b""))
    assert [(each.request, each.reply, each.body) for each in found] == [
        (SYNC, 8, b"abcd"),
        (ASYNC, 0, b""),
    ]


@pytest.mark.parametrize("cut", [5, 14])
def test_a_truncated_recording_is_refused(cut: int) -> None:
    with pytest.raises(InitError, match="ends inside"):
        messages(entry(SYNC, b"abcd", 8)[:cut])


def test_the_init_keeps_messages_0_to_4_and_7_to_11_in_order() -> None:
    chosen = kept(messages(recording()))
    assert [each.body for each in chosen] == [
        body(request, index) for index, request in enumerate(SEQUENCE) if index not in (5, 6)
    ]


def test_the_init_is_the_kind_the_lengths_and_the_body_of_each() -> None:
    found = init(recording())
    kind, length, reply = struct.unpack_from("<BII", found, 0)
    assert (kind, length, reply) == (1, 8, 8)
    assert found[9:17] == body(SYNC, 0)
    kind, length, reply = struct.unpack_from("<BII", found, 17)
    assert (kind, length, reply) == (2, PARAMETERS, 0)
    assert len(found) == 10 * 9 + 8 + 3 * PARAMETERS + 8 + 5 * 8


@pytest.mark.parametrize(
    "requests",
    [
        SEQUENCE[1:],
        (*SEQUENCE, ASYNC),
        (ASYNC, *SEQUENCE[1:]),
        (*SEQUENCE[:5], ASYNC, ASYNC, *SEQUENCE[7:]),
    ],
)
def test_a_sequence_that_differs_in_number_or_kind_is_refused(
    requests: tuple[int, ...],
) -> None:
    with pytest.raises(InitError, match="not the reading's 12"):
        init(recording(requests))


def test_the_racing_set_para_and_set_source_are_written_set_para_first() -> None:
    interleaved = recording(order=(0, 1, 2, 3, 4, 5, 6, 7, 8, 10, 9, 11))
    assert init(interleaved) == init(recording())


def test_a_racing_tail_of_other_messages_is_refused() -> None:
    with pytest.raises(InitError, match="not two SET_PARA and two SET_SOURCE"):
        init(recording(order=(0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 7)))


def test_a_parameter_block_of_another_size_is_refused() -> None:
    entries = messages(recording())
    short = entry(WRITE_PARAMS, b"\x00" * (PARAMETERS - 4))
    raw = b"".join(
        short if index == 2 else entry(each.request, each.body, each.reply)
        for index, each in enumerate(entries)
    )
    with pytest.raises(InitError, match="WRITE_PARAMS 2 is 309604 bytes"):
        init(raw)


def test_two_recordings_that_agree_write_the_init(tmp_path: Path) -> None:
    recorder = tmp_path / "dsp-recorder"
    recorder.write_bytes(b"\x7fELF")
    out = tmp_path / "out" / "dsp-init.bin"
    phone = Phone([recording(), recording()])
    code = main(["dsp_init.py", "--recorder", str(recorder), "--out", str(out)], phone)
    assert code == 0
    assert out.read_bytes() == init(recording())
    assert ("push", str(recorder), str(RECORDER)) in phone.calls
    starts = [line for line in phone.shells() if str(RECORDING) in line]
    assert len(starts) == 2
    assert all(
        line.index(str(RECORDER))
        < line.index("start vendor.audio-hal-2-0")
        < line.index("start audioserver")
        for line in starts
    )
    assert phone.shells()[-1] == "start vendor.audio-hal-2-0; start audioserver"


def test_the_hal_is_stopped_before_each_recording(tmp_path: Path) -> None:
    recorder = tmp_path / "dsp-recorder"
    recorder.write_bytes(b"\x7fELF")
    phone = Phone([recording(), recording()])
    main(["dsp_init.py", "--recorder", str(recorder), "--out", str(tmp_path / "x")], phone)
    lines = phone.shells()
    for start in (i for i, line in enumerate(lines) if str(RECORDING) in line):
        assert lines[start - 1].startswith("stop audioserver; stop vendor.audio-hal-2-0;")


def test_recordings_that_disagree_write_nothing(
    tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    first = tmp_path / "first.rec"
    second = tmp_path / "second.rec"
    first.write_bytes(recording())
    other = messages(recording())
    second.write_bytes(
        b"".join(
            entry(each.request, each.body[:-1] + b"\xff" if index == 9 else each.body, each.reply)
            for index, each in enumerate(other)
        )
    )
    out = tmp_path / "dsp-init.bin"
    code = main(["dsp_init.py", "--out", str(out), "--from", str(first), str(second)])
    assert code == 1
    assert not out.exists()
    said = capsys.readouterr().err
    assert "recordings 0 and 1 differ in message 9, c00c4170" in said
    assert "8 and 8 bytes, first apart at byte 7" in said


def test_the_audio_services_come_back_when_a_recording_fails(tmp_path: Path) -> None:
    recorder = tmp_path / "dsp-recorder"
    recorder.write_bytes(b"\x7fELF")
    phone = Phone([recording(SEQUENCE[1:]), recording()])
    code = main(["dsp_init.py", "--recorder", str(recorder), "--out", str(tmp_path / "x")], phone)
    assert code == 1
    assert phone.shells()[-1] == "start vendor.audio-hal-2-0; start audioserver"


def test_a_phone_in_twrp_is_refused_before_anything_is_pushed(
    tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    recorder = tmp_path / "dsp-recorder"
    recorder.write_bytes(b"\x7fELF")
    phone = Phone([], state="recovery")
    code = main(["dsp_init.py", "--recorder", str(recorder), "--out", str(tmp_path / "x")], phone)
    assert code == 1
    assert phone.calls == [("get-state",)]
    assert "in recovery, not device" in capsys.readouterr().err


def test_the_raw_recordings_are_kept_where_asked(tmp_path: Path) -> None:
    recorder = tmp_path / "dsp-recorder"
    recorder.write_bytes(b"\x7fELF")
    keep = tmp_path / "kept"
    phone = Phone([recording(), recording()])
    argv = ["dsp_init.py", "--recorder", str(recorder), "--out", str(tmp_path / "x")]
    assert main([*argv, "--keep", str(keep)], phone) == 0
    assert sorted(path.name for path in keep.iterdir()) == ["hifi-0.rec", "hifi-1.rec"]
    assert (keep / "hifi-1.rec").read_bytes() == recording()
