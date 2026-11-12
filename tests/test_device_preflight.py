from __future__ import annotations

import hashlib
from pathlib import Path

import pytest

from device_preflight import (
    PARTITION,
    Answer,
    Backup,
    Fastboot,
    PreflightError,
    Reset,
    fastboot_runner,
    libusb_reset,
    main,
)

FIXTURES = Path(__file__).parent / "fixtures" / "fastboot"
MODEL = ("oem", "get-product-model")
LOCK = ("oem", "lock-state", "info")


def recorded(name: str) -> str:
    return (FIXTURES / f"{name}.stderr").read_text()


ANSWERS = {
    MODEL: Answer(0, recorded("oem-get-product-model")),
    LOCK: Answer(0, recorded("oem-lock-state-info")),
}


class Phone:
    def __init__(self, answers: dict[tuple[str, ...], Answer]) -> None:
        self.answers = answers
        self.events: list[str] = []

    def reset(self) -> None:
        self.events.append("reset")

    def fastboot(self, command: tuple[str, ...]) -> Answer:
        self.events.append(" ".join(command))
        return self.answers[command]


def image(tmp_path: Path, size: int) -> Path:
    path = tmp_path / "image.img"
    with path.open("wb") as stream:
        stream.truncate(size)
    return path


def backup(tmp_path: Path, content: bytes = b"stock") -> Backup:
    path = tmp_path / "stock.img"
    path.write_bytes(content)
    return Backup(path, hashlib.sha256(content).hexdigest())


def preflight(path: Path, phone: Phone, stock: Backup, reset: Reset | None = None) -> int:
    fastboot: Fastboot = phone.fastboot
    return main(["device_preflight.py", str(path)], reset or phone.reset, fastboot, stock)


def test_the_recorded_phone_passes_with_a_reset_before_every_invocation_and_after(
    tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    phone = Phone(ANSWERS)
    path = image(tmp_path, 4096)
    assert preflight(path, phone, backup(tmp_path)) == 0
    assert phone.events == ["reset", " ".join(MODEL), "reset", " ".join(LOCK), "reset"]
    out = capsys.readouterr().out
    assert hashlib.sha256(path.read_bytes()).hexdigest() in out
    assert "device: ANE-LX1" in out
    assert "lock state: FB UNLOCKED, USER UNLOCKED" in out


def test_reset_alone_resets_once_and_asks_nothing(tmp_path: Path) -> None:
    phone = Phone(ANSWERS)
    stock = Backup(tmp_path / "absent.img", "0" * 64)
    assert main(["device_preflight.py", "--reset"], phone.reset, phone.fastboot, stock) == 0
    assert phone.events == ["reset"]


@pytest.mark.parametrize("arguments", [[], ["image.img", "--reset"]])
def test_it_takes_an_image_or_reset_and_not_both(arguments: list[str]) -> None:
    phone = Phone(ANSWERS)
    with pytest.raises(SystemExit) as exited:
        main(["device_preflight.py", *arguments], phone.reset, phone.fastboot)
    assert exited.value.code == 2
    assert phone.events == []


def test_an_image_the_size_of_the_partition_passes(tmp_path: Path) -> None:
    assert preflight(image(tmp_path, PARTITION), Phone(ANSWERS), backup(tmp_path)) == 0


@pytest.mark.parametrize(
    ("size", "said"),
    [(PARTITION + 1, f"{PARTITION + 1} bytes, over kernel's {PARTITION}"), (0, "is empty")],
)
def test_an_image_that_does_not_fit_fails_before_the_phone_is_touched(
    tmp_path: Path, capsys: pytest.CaptureFixture[str], size: int, said: str
) -> None:
    phone = Phone(ANSWERS)
    assert preflight(image(tmp_path, size), phone, backup(tmp_path)) == 1
    assert said in capsys.readouterr().err
    assert phone.events == []


def test_a_missing_image_fails(tmp_path: Path, capsys: pytest.CaptureFixture[str]) -> None:
    assert preflight(tmp_path / "absent.img", Phone(ANSWERS), backup(tmp_path)) == 1
    assert "is missing" in capsys.readouterr().err


def test_a_missing_backup_fails(tmp_path: Path, capsys: pytest.CaptureFixture[str]) -> None:
    stock = Backup(tmp_path / "absent.img", "0" * 64)
    assert preflight(image(tmp_path, 4096), Phone(ANSWERS), stock) == 1
    assert "absent.img is missing" in capsys.readouterr().err


def test_a_backup_with_another_hash_fails(
    tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    stock = Backup(backup(tmp_path).path, "0" * 64)
    assert preflight(image(tmp_path, 4096), Phone(ANSWERS), stock) == 1
    assert f"not {'0' * 64}" in capsys.readouterr().err


def test_another_model_fails(tmp_path: Path, capsys: pytest.CaptureFixture[str]) -> None:
    other = Answer(0, recorded("oem-get-product-model").replace("ANE-LX1", "ANE-LX2"))
    phone = Phone({**ANSWERS, MODEL: other})
    assert preflight(image(tmp_path, 4096), phone, backup(tmp_path)) == 1
    assert "the device is ANE-LX2, not ANE-LX1" in capsys.readouterr().err
    assert " ".join(LOCK) not in phone.events


@pytest.mark.parametrize(
    ("stderr", "said"),
    [
        (
            recorded("oem-lock-state-info").replace(
                "FB LockState: UNLOCKED", "FB LockState: LOCKED"
            ),
            "FB LockState is LOCKED, not UNLOCKED",
        ),
        (
            "".join(
                line
                for line in recorded("oem-lock-state-info").splitlines(keepends=True)
                if "USER" not in line
            ),
            "USER LockState is not reported, not UNLOCKED",
        ),
    ],
)
def test_a_lock_state_other_than_unlocked_fails(
    tmp_path: Path, capsys: pytest.CaptureFixture[str], stderr: str, said: str
) -> None:
    phone = Phone({**ANSWERS, LOCK: Answer(0, stderr)})
    assert preflight(image(tmp_path, 4096), phone, backup(tmp_path)) == 1
    assert said in capsys.readouterr().err
    assert phone.events[-1] == " ".join(LOCK)


def test_a_refused_command_fails_with_what_the_bootloader_said(
    tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    phone = Phone({**ANSWERS, MODEL: Answer(1, recorded("oem-invalid-command"))})
    assert preflight(image(tmp_path, 4096), phone, backup(tmp_path)) == 1
    assert (
        "fastboot oem get-product-model exited 1: FAILED (remote: 'invalid command')"
        in capsys.readouterr().err
    )


def test_a_failed_reset_sends_nothing(tmp_path: Path, capsys: pytest.CaptureFixture[str]) -> None:
    def unplugged() -> None:
        raise PreflightError("no 18d1:d00d on USB")

    phone = Phone(ANSWERS)
    assert preflight(image(tmp_path, 4096), phone, backup(tmp_path), unplugged) == 1
    assert "no 18d1:d00d on USB" in capsys.readouterr().err
    assert phone.events == []


def executable(tmp_path: Path, body: str) -> str:
    path = tmp_path / "fastboot"
    path.write_text(f"#!/bin/sh\n{body}\n")
    path.chmod(0o755)
    return str(path)


def test_the_runner_returns_the_exit_code_and_stderr(tmp_path: Path) -> None:
    run = fastboot_runner(executable(tmp_path, 'echo "(bootloader) $*" >&2\nexit 3'))
    assert run(MODEL) == Answer(3, "(bootloader) oem get-product-model\n")


def test_the_runner_fails_on_a_hang(tmp_path: Path) -> None:
    run = fastboot_runner(executable(tmp_path, "exec sleep 5"), timeout=0.2)
    with pytest.raises(PreflightError, match=r"answered nothing in 0\.2 s"):
        run(MODEL)


def test_the_runner_fails_without_fastboot(tmp_path: Path) -> None:
    with pytest.raises(PreflightError, match="is not installed"):
        fastboot_runner(str(tmp_path / "absent"))(MODEL)


def test_the_reset_fails_without_libusb(tmp_path: Path) -> None:
    with pytest.raises(PreflightError, match="brew install libusb"):
        libusb_reset(tmp_path / "libusb-1.0.dylib")
