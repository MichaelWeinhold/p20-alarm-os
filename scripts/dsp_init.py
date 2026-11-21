from __future__ import annotations

import argparse
import struct
import sys
import tempfile
from collections.abc import Sequence
from dataclasses import dataclass
from pathlib import Path, PurePosixPath
from typing import Final

from system_push import Adb, PushError, adb_runner, asked, shell, state

ASYNC: Final = 0xC00C4170
SYNC: Final = 0x40184171
WRITE_PARAMS: Final = 0xC0184175
USB: Final = 0x4018417C
SEQUENCE: Final = (
    SYNC,
    WRITE_PARAMS,
    WRITE_PARAMS,
    WRITE_PARAMS,
    ASYNC,
    USB,
    USB,
    ASYNC,
    ASYNC,
    ASYNC,
    ASYNC,
    ASYNC,
)
KEPT: Final = (0, 1, 2, 3, 4, 7, 8, 9, 10, 11)
KINDS: Final = {ASYNC: 0, SYNC: 1, WRITE_PARAMS: 2}
PARAMETERS: Final = 309608
ENTRY: Final = struct.Struct("<III")
FRAME: Final = struct.Struct("<BII")
HAL: Final = "vendor.audio-hal-2-0"
AUDIOSERVER: Final = "audioserver"
PROGRAM: Final = PurePosixPath("/vendor/bin/hw/vendor.huawei.hardware.audio@4.0-service")
WORK: Final = PurePosixPath("/data/local/tmp")
RECORDER: Final = WORK / "dsp-recorder"
RECORDING: Final = WORK / "hifi.rec"
SECONDS: Final = 30
STOPPED_WITHIN: Final = 50
BUILT: Final = Path("target/aarch64-unknown-linux-musl/release/dsp-recorder")
OUT: Final = Path("out/dsp-init.bin")
RECORDINGS: Final = 2


class InitError(RuntimeError):
    pass


@dataclass(frozen=True, slots=True)
class Message:
    request: int
    reply: int
    body: bytes


def messages(recording: bytes) -> list[Message]:
    found: list[Message] = []
    at = 0
    while at < len(recording):
        if at + ENTRY.size > len(recording):
            raise InitError(f"the recording ends inside an entry header at byte {at}")
        request, length, reply = ENTRY.unpack_from(recording, at)
        at += ENTRY.size
        if at + length > len(recording):
            raise InitError(f"the recording ends inside a {length}-byte body at byte {at}")
        found.append(Message(request, reply, recording[at : at + length]))
        at += length
    return found


def kept(found: Sequence[Message]) -> list[Message]:
    requests = tuple(message.request for message in found)
    if requests != SEQUENCE:
        named = " ".join(f"{request:08x}" for request in requests) or "nothing"
        raise InitError(f"the recording is {len(requests)} ioctls, {named}; not the reading's 12")
    for index, message in enumerate(found):
        if message.request == WRITE_PARAMS and len(message.body) != PARAMETERS:
            raise InitError(f"WRITE_PARAMS {index} is {len(message.body)} bytes, not {PARAMETERS}")
    return [found[index] for index in KEPT]


def framed(chosen: Sequence[Message]) -> bytes:
    return b"".join(
        FRAME.pack(KINDS[message.request], len(message.body), message.reply) + message.body
        for message in chosen
    )


def init(recording: bytes) -> bytes:
    return framed(kept(messages(recording)))


def stopped(adb: Adb) -> None:
    shell(
        adb,
        f"stop {AUDIOSERVER}; stop {HAL}; "
        f"for i in $(seq {STOPPED_WITHIN}); do pidof {PROGRAM.name} >/dev/null || exit 0; "
        "sleep 0.1; done; exit 1",
    )


def recorded(adb: Adb, local: Path) -> bytes:
    stopped(adb)
    shell(
        adb,
        f"rm -f {RECORDING}; {RECORDER} {PROGRAM} {RECORDING} {len(SEQUENCE)} {SECONDS} & "
        f"start {HAL}; wait $!",
    )
    asked(adb, "pull", str(RECORDING), str(local))
    return local.read_bytes()


def recordings(adb: Adb, recorder: Path, count: int) -> list[bytes]:
    if not recorder.is_file():
        raise InitError(f"{recorder} is missing: build it with cargo build --release --target")
    state(adb, "device")
    asked(adb, "root")
    asked(adb, "wait-for-device")
    asked(adb, "push", str(recorder), str(RECORDER))
    shell(adb, f"chmod 755 {RECORDER}")
    try:
        with tempfile.TemporaryDirectory() as scratch:
            return [recorded(adb, Path(scratch) / f"hifi-{index}.rec") for index in range(count)]
    finally:
        shell(adb, f"start {HAL}; start {AUDIOSERVER}")


def same(inits: Sequence[bytes]) -> bytes:
    if len(set(inits)) != 1:
        sizes = ", ".join(str(len(each)) for each in inits)
        raise InitError(f"{len(inits)} recordings give different init files, {sizes} bytes")
    return inits[0]


def parser() -> argparse.ArgumentParser:
    found = argparse.ArgumentParser(prog="dsp_init.py")
    found.add_argument("--recorder", type=Path, default=BUILT)
    found.add_argument("--out", type=Path, default=OUT)
    found.add_argument("--from", dest="recordings", type=Path, nargs="+")
    return found


def main(argv: list[str], adb: Adb | None = None) -> int:
    options = parser().parse_args(argv[1:])
    out: Path = options.out
    sources: list[Path] | None = options.recordings
    try:
        if sources is None:
            raw = recordings(adb or adb_runner(), options.recorder, RECORDINGS)
        else:
            raw = [source.read_bytes() for source in sources]
        result = same([init(each) for each in raw])
    except (InitError, PushError, OSError) as error:
        print(error, file=sys.stderr)
        return 1
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_bytes(result)
    print(f"{out}: {len(result)} bytes from {len(raw)} recordings that agree")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
