from __future__ import annotations

import hashlib
import os
import select
import subprocess
import threading
from collections.abc import Callable, Iterator
from pathlib import Path, PurePosixPath

import pytest

from serial_shell import (
    FAILED,
    MNT,
    TIMED_OUT,
    SerialError,
    exchange,
    main,
    opened,
    parsed,
    port,
    push,
    typed,
)

PROMPT = b":/ # "
WIDTH = 80

Rewrite = Callable[[str], str]


def redrawn(line: bytes) -> bytes:
    pieces = [line[start : start + WIDTH] for start in range(0, max(len(line), 1), WIDTH // 2)]
    return b"\r".join(pieces) + b"\r\n"


class Phone:
    def __init__(self, master: int, mnt: Path, rewrite: Rewrite | None = None) -> None:
        self.master = master
        self.mnt = mnt
        self.rewrite = rewrite
        self.lines: list[str] = []
        self.stopped = threading.Event()
        self.thread = threading.Thread(target=self.serve, daemon=True)

    def answer(self, line: bytes) -> bytes:
        command = line.decode().replace(f"{MNT}/", f"{self.mnt}/")
        self.lines.append(command)
        if self.rewrite is not None:
            command = self.rewrite(command)
        result = subprocess.run(
            ["sh", "-c", command], stdout=subprocess.PIPE, stderr=subprocess.STDOUT, check=False
        )
        return result.stdout.replace(b"\n", b"\r\n")

    def stop(self) -> None:
        self.stopped.set()
        self.thread.join()

    def serve(self) -> None:
        buffer = b""
        while not self.stopped.is_set():
            try:
                readable, _, _ = select.select([self.master], [], [], 0.02)
                if not readable:
                    continue
                buffer += os.read(self.master, 65536)
            except OSError:
                return
            while b"\n" in buffer:
                line, buffer = buffer.split(b"\n", 1)
                os.write(self.master, redrawn(line) + self.answer(line) + PROMPT)


class Silent(Phone):
    def answer(self, line: bytes) -> bytes:
        return b""


@pytest.fixture
def pair() -> Iterator[tuple[int, Path]]:
    master, slave = os.openpty()
    try:
        yield master, Path(os.ttyname(slave))
    finally:
        os.close(slave)
        os.close(master)


def served(phone: Phone) -> Phone:
    phone.thread.start()
    return phone


@pytest.fixture
def phone(pair: tuple[int, Path], tmp_path: Path) -> Iterator[Phone]:
    found = served(Phone(pair[0], tmp_path))
    yield found
    found.stop()


def run(pair: tuple[int, Path], *arguments: str) -> int:
    return main(["serial_shell.py", "--port", str(pair[1]), "--timeout", "3", *arguments])


def test_a_command_prints_its_output_without_the_echo_and_exits_with_its_status(
    pair: tuple[int, Path], phone: Phone, capsys: pytest.CaptureFixture[str]
) -> None:
    assert run(pair, "echo 4.9.319-Abdelhay-KSU-") == 0
    assert capsys.readouterr().out == "4.9.319-Abdelhay-KSU-\n"


def test_a_failing_command_exits_with_its_own_status(
    pair: tuple[int, Path], phone: Phone, capsys: pytest.CaptureFixture[str]
) -> None:
    assert run(pair, "false") == 1
    assert run(pair, "(exit 7)") == 7
    assert capsys.readouterr().out == ""


def test_output_without_a_final_newline_and_stderr_both_come_back(
    pair: tuple[int, Path], phone: Phone, capsys: pytest.CaptureFixture[str]
) -> None:
    assert run(pair, "printf abc; echo oops >&2") == 0
    assert capsys.readouterr().out == "abcoops\n"


def test_quotes_and_a_long_redrawn_line_survive(
    pair: tuple[int, Path], phone: Phone, capsys: pytest.CaptureFixture[str]
) -> None:
    long = "x" * 300
    assert run(pair, f"echo 'it'\"'\"'s' \"{long}\"") == 0
    assert capsys.readouterr().out == f"it's {long}\n"


def test_the_command_reads_no_input_from_the_line(
    pair: tuple[int, Path], phone: Phone, capsys: pytest.CaptureFixture[str]
) -> None:
    assert run(pair, "cat; echo done") == 0
    assert capsys.readouterr().out == "done\n"


def test_a_phone_that_never_answers_times_out_with_its_own_code(
    pair: tuple[int, Path], tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    silent = served(Silent(pair[0], tmp_path))
    try:
        code = main(["serial_shell.py", "--port", str(pair[1]), "--timeout", "0.3", "uname -r"])
    finally:
        silent.stop()
    assert code == TIMED_OUT
    assert "no end marker in 0.3 s" in capsys.readouterr().err


def test_the_typed_line_never_holds_the_markers_it_waits_for() -> None:
    tag = "p20-abc-"
    line = typed("echo hi", tag)
    assert f"{tag}BEGIN".encode() not in line
    assert f"{tag}END".encode() not in line


def test_a_reply_waits_for_the_whole_status() -> None:
    tag = "p20-abc-"
    assert parsed(b"p20-abc-BEGIN\r\nout\r\np20-abc-END:1", tag) is None
    reply = parsed(b"junk p20-abc-BEGIN\r\nout\r\np20-abc-END:12\r\n:/ # ", tag)
    assert reply is not None
    assert (reply.output, reply.status) == (b"out\n", 12)


def test_no_port_and_two_ports_each_fail_with_a_clear_message(tmp_path: Path) -> None:
    with pytest.raises(SerialError, match="not on the cable"):
        port(tmp_path)
    (tmp_path / "cu.usbmodem1").touch()
    assert port(tmp_path) == tmp_path / "cu.usbmodem1"
    (tmp_path / "cu.usbmodem2").touch()
    with pytest.raises(SerialError, match=r"2 ports match .* name one with --port"):
        port(tmp_path)


def test_main_without_a_port_fails_with_its_own_code(
    tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    assert main(["serial_shell.py", "uname -r"], tmp_path) == FAILED
    assert "not on the cable" in capsys.readouterr().err


def test_a_file_that_is_no_serial_port_is_refused(tmp_path: Path) -> None:
    path = tmp_path / "cu.usbmodem1"
    path.touch()
    with pytest.raises(SerialError, match="not a serial port"), opened(path):
        pass


def test_a_pushed_file_arrives_whole_and_its_hash_is_checked_on_the_phone(
    pair: tuple[int, Path], phone: Phone, tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    source = tmp_path / "alarmd"
    data = os.urandom(10_000)
    source.write_bytes(data)
    assert run(pair, "--push", str(source), "--to", "/mnt/copy") == 0
    assert (tmp_path / "copy").read_bytes() == data
    assert not (tmp_path / "copy.b64").exists()
    assert hashlib.sha256(data).hexdigest() in capsys.readouterr().out


def test_an_empty_file_pushes_too(pair: tuple[int, Path], phone: Phone, tmp_path: Path) -> None:
    source = tmp_path / "empty"
    source.touch()
    with opened(pair[1]) as fd:
        push(fd, source, PurePosixPath("/mnt/copy"), 3)
    assert (tmp_path / "copy").read_bytes() == b""


def test_a_hash_that_differs_on_the_phone_fails(
    pair: tuple[int, Path], tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    def corrupted(command: str) -> str:
        return command.replace("sha256sum", "echo 00 #")

    lying = served(Phone(pair[0], tmp_path, corrupted))
    source = tmp_path / "alarmd"
    source.write_bytes(b"alarmd")
    try:
        assert run(pair, "--push", str(source), "--to", "/mnt/copy") == FAILED
    finally:
        lying.stop()
    assert "hashes to 00 on the phone" in capsys.readouterr().err


def test_a_push_outside_the_tmpfs_is_refused(pair: tuple[int, Path], phone: Phone) -> None:
    with opened(pair[1]) as fd, pytest.raises(SerialError, match="not under /mnt"):
        push(fd, Path("unused"), PurePosixPath("/system/init"), 3)
    assert phone.lines == []


def test_a_command_that_fails_during_a_push_names_itself(
    pair: tuple[int, Path], phone: Phone, tmp_path: Path
) -> None:
    source = tmp_path / "alarmd"
    source.write_bytes(b"alarmd")
    with opened(pair[1]) as fd, pytest.raises(SerialError, match=r"'printf .* exited [1-9]"):
        push(fd, source, PurePosixPath("/mnt/missing/alarmd"), 3)


def test_to_without_push_is_a_usage_error(pair: tuple[int, Path]) -> None:
    with pytest.raises(SystemExit):
        run(pair, "--to", "/mnt/x", "uname -r")


def test_exchange_answers_in_order_on_one_open_port(pair: tuple[int, Path], phone: Phone) -> None:
    with opened(pair[1]) as fd:
        first = exchange(fd, "echo one", 3)
        second = exchange(fd, "echo two", 3)
    assert (first.output, second.output) == (b"one\n", b"two\n")
