from __future__ import annotations

import argparse
import base64
import hashlib
import os
import re
import select
import shlex
import sys
import termios
import time
import tty
import uuid
from collections.abc import Iterator
from contextlib import contextmanager
from dataclasses import dataclass
from pathlib import Path, PurePosixPath
from typing import Final

DEV: Final = Path("/dev")
PORTS: Final = "cu.usbmodem*"
MNT: Final = PurePosixPath("/mnt")
TIMEOUT: Final = 10.0
TIMED_OUT: Final = 124
FAILED: Final = 125
CHUNK: Final = 2048
READ: Final = 65536


class SerialError(RuntimeError):
    pass


class TimedOutError(SerialError):
    pass


@dataclass(frozen=True, slots=True)
class Reply:
    output: bytes
    status: int


def port(directory: Path = DEV) -> Path:
    found = sorted(directory.glob(PORTS))
    pattern = directory / PORTS
    if not found:
        raise SerialError(f"no {pattern}: the phone is not on the cable, or not on the debug /init")
    if len(found) > 1:
        names = ", ".join(str(path) for path in found)
        raise SerialError(f"{len(found)} ports match {pattern}, {names}: name one with --port")
    return found[0]


@contextmanager
def opened(path: Path) -> Iterator[int]:
    try:
        fd = os.open(path, os.O_RDWR | os.O_NOCTTY | os.O_NONBLOCK)
    except OSError as error:
        raise SerialError(f"{path} does not open: {error}") from error
    try:
        try:
            tty.setraw(fd)
            termios.tcflush(fd, termios.TCIFLUSH)
        except termios.error as error:
            raise SerialError(f"{path} is not a serial port: {error}") from error
        yield fd
    finally:
        os.close(fd)


def typed(command: str, tag: str) -> bytes:
    quoted = shlex.quote(command)
    return f'echo "{tag}"BEGIN; eval {quoted} </dev/null; echo "{tag}"END:$?\n'.encode()


def parsed(received: bytes, tag: str) -> Reply | None:
    marker = re.escape(tag.encode())
    match = re.search(rb"%sBEGIN\r?\n(.*?)%sEND:(\d+)\r?\n" % (marker, marker), received, re.DOTALL)
    if match is None:
        return None
    return Reply(match.group(1).replace(b"\r\n", b"\n"), int(match.group(2)))


def exchange(fd: int, command: str, timeout: float) -> Reply:
    tag = f"p20-{uuid.uuid4().hex[:12]}-"
    pending = typed(command, tag)
    received = bytearray()
    deadline = time.monotonic() + timeout
    while (reply := parsed(bytes(received), tag)) is None:
        left = deadline - time.monotonic()
        if left <= 0:
            raise TimedOutError(f"{command!r} answered no end marker in {timeout:g} s")
        readable, writable, _ = select.select([fd], [fd] if pending else [], [], left)
        try:
            if writable:
                pending = pending[os.write(fd, pending) :]
            if readable:
                chunk = os.read(fd, READ)
                if not chunk:
                    raise SerialError("the port closed")
                received += chunk
        except BlockingIOError:
            continue
        except OSError as error:
            raise SerialError(f"the port went away: {error}") from error
    return reply


def checked(fd: int, command: str, timeout: float) -> Reply:
    reply = exchange(fd, command, timeout)
    if reply.status != 0:
        said = reply.output.decode(errors="replace").strip()
        raise SerialError(f"{command[:80]!r} exited {reply.status}: {said or 'no output'}")
    return reply


def push(fd: int, source: Path, target: PurePosixPath, timeout: float) -> str:
    if not target.is_absolute() or MNT not in target.parents:
        raise SerialError(f"{target} is not under {MNT}, the phone's tmpfs")
    try:
        data = source.read_bytes()
    except OSError as error:
        raise SerialError(f"{source} does not read: {error}") from error
    digest = hashlib.sha256(data).hexdigest()
    encoded = base64.b64encode(data).decode()
    staged = shlex.quote(f"{target}.b64")
    destination = shlex.quote(str(target))
    checked(fd, f'printf %s "" > {staged}', timeout)
    for start in range(0, len(encoded), CHUNK):
        checked(fd, f"echo {encoded[start : start + CHUNK]} >> {staged}", timeout)
    checked(fd, f"base64 -d < {staged} > {destination} && rm {staged}", timeout)
    words = checked(fd, f"sha256sum {destination}", timeout).output.split()
    found = words[0].decode(errors="replace") if words else "nothing"
    if found != digest:
        raise SerialError(f"{target} hashes to {found} on the phone, not {digest}")
    return digest


def parser() -> argparse.ArgumentParser:
    found = argparse.ArgumentParser(prog="serial_shell.py")
    found.add_argument("--port", type=Path)
    found.add_argument("--timeout", type=float, default=TIMEOUT)
    found.add_argument("--to", type=PurePosixPath)
    action = found.add_mutually_exclusive_group(required=True)
    action.add_argument("command", nargs="?")
    action.add_argument("--push", type=Path, metavar="FILE")
    return found


def main(argv: list[str], directory: Path = DEV) -> int:
    arguments = parser()
    options = arguments.parse_args(argv[1:])
    source: Path | None = options.push
    target: PurePosixPath | None = options.to
    timeout: float = options.timeout
    if target is not None and source is None:
        arguments.error("--to goes with --push")
    try:
        with opened(options.port or port(directory)) as fd:
            if source is not None:
                destination = target or MNT / source.name
                digest = push(fd, source, destination, timeout)
                print(f"{source} is {destination} on the phone, sha256 {digest}")
                return 0
            reply = exchange(fd, options.command, timeout)
    except TimedOutError as error:
        print(error, file=sys.stderr)
        return TIMED_OUT
    except SerialError as error:
        print(error, file=sys.stderr)
        return FAILED
    sys.stdout.flush()
    sys.stdout.buffer.write(reply.output)
    sys.stdout.buffer.flush()
    return reply.status


if __name__ == "__main__":
    sys.exit(main(sys.argv))
