from __future__ import annotations

import hashlib
import shlex
from pathlib import Path, PurePosixPath

import pytest

from system_push import MOUNT, Answer, Pushed, PushError, main, push


class Phone:
    def __init__(self, state: str = "recovery", corrupt: bool = False) -> None:
        self.state = state
        self.corrupt = corrupt
        self.files: dict[str, bytes] = {}
        self.calls: list[tuple[str, ...]] = []

    def __call__(self, command: tuple[str, ...]) -> Answer:
        self.calls.append(command)
        if command == ("get-state",):
            return Answer(0, f"{self.state}\n".encode(), "")
        if command[0] == "push":
            data = Path(command[1]).read_bytes()
            self.files[command[2]] = data + b"!" if self.corrupt else data
            return Answer(0, b"", "")
        return self.shell(command[1])

    def shell(self, line: str) -> Answer:
        words = shlex.split(line)
        if words[0] == "sha256sum":
            data = self.files.get(words[1])
            if data is None:
                return Answer(1, b"", f"sha256sum: {words[1]}: No such file\n")
            digest = hashlib.sha256(data).hexdigest()
            return Answer(0, f"{digest}  {words[1]}\n{len(data)}\n".encode(), "")
        if words[0] == "rm":
            self.files.pop(words[2], None)
        if words[0] == "chmod":
            self.files[words[6]] = self.files.pop(words[5])
        return Answer(0, b"", "")


def written(tmp_path: Path, name: str, data: bytes) -> Path:
    path = tmp_path / name
    path.write_bytes(data)
    return path


def test_a_file_lands_under_its_final_name_and_reads_back_whole(tmp_path: Path) -> None:
    phone = Phone()
    local = written(tmp_path, "dsp-init.bin", b"\x71\x41\x18\x40" * 100)
    pushed = push(phone, [(local, PurePosixPath("etc/alarmd/dsp-init.bin"))])
    final = str(MOUNT / "etc/alarmd/dsp-init.bin")
    assert pushed == [
        Pushed(PurePosixPath(final), 400, hashlib.sha256(local.read_bytes()).hexdigest())
    ]
    assert list(phone.files) == [final]


def test_the_push_goes_under_a_temporary_name_and_is_renamed_after_its_check(
    tmp_path: Path,
) -> None:
    phone = Phone()
    local = written(tmp_path, "speaker-route.tsv", b"name\tvalue\n")
    push(phone, [(local, PurePosixPath("etc/alarmd/speaker-route.tsv"))])
    shells = [call[1] for call in phone.calls if call[0] == "shell"]
    part = f"{MOUNT}/etc/alarmd/speaker-route.tsv.part"
    assert ("push", str(local), part) in phone.calls
    assert shells.index("sync") < next(i for i, line in enumerate(shells) if "mv " in line)
    assert shells[-1] == f"umount {MOUNT}"


def test_a_file_that_reads_back_different_is_removed_and_refused(tmp_path: Path) -> None:
    phone = Phone(corrupt=True)
    local = written(tmp_path, "dsp-init.bin", b"init")
    with pytest.raises(PushError, match="reads back as 5 bytes"):
        push(phone, [(local, PurePosixPath("etc/alarmd/dsp-init.bin"))])
    assert phone.files == {}
    assert phone.calls[-1] == ("shell", f"umount {MOUNT}")


def test_a_phone_outside_twrp_is_refused_before_anything_is_mounted(tmp_path: Path) -> None:
    phone = Phone(state="device")
    local = written(tmp_path, "dsp-init.bin", b"init")
    with pytest.raises(PushError, match="in device, not recovery"):
        push(phone, [(local, PurePosixPath("etc/alarmd/dsp-init.bin"))])
    assert phone.calls == [("get-state",)]


@pytest.mark.parametrize("relative", ["/etc/alarmd/x", "../x", "etc/../../x"])
def test_a_destination_outside_system_is_refused(tmp_path: Path, relative: str) -> None:
    local = written(tmp_path, "x", b"x")
    with pytest.raises(PushError, match="not a path inside system"):
        push(Phone(), [(local, PurePosixPath(relative))])


def test_main_pushes_each_file_into_the_directory_and_prints_it(
    tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    phone = Phone()
    first = written(tmp_path, "dsp-init.bin", b"one")
    second = written(tmp_path, "speaker-route.tsv", b"two")
    code = main(["system_push.py", "--to", "etc/alarmd", str(first), str(second)], phone)
    assert code == 0
    assert set(phone.files) == {
        f"{MOUNT}/etc/alarmd/dsp-init.bin",
        f"{MOUNT}/etc/alarmd/speaker-route.tsv",
    }
    assert "speaker-route.tsv: 3 bytes" in capsys.readouterr().out


def test_main_names_a_missing_file_and_fails(
    tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    code = main(["system_push.py", "--to", "etc/alarmd", str(tmp_path / "gone")], Phone())
    assert code == 1
    assert "gone is missing" in capsys.readouterr().err
