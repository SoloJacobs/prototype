#!/usr/bin/env python3
import json
import select
import socket
import sys
import time
from argparse import ArgumentParser
from base64 import b64encode
from dataclasses import dataclass
from pathlib import Path


@dataclass(frozen=True)
class Config:
    target: Path


def parse_args() -> Config:
    parser = ArgumentParser(description="Replace path and log")
    parser.add_argument(
        "--target",
        type=Path,
        required=True,
        help="The target path to replace",
    )
    namespace = parser.parse_args()
    return Config(target=namespace.target)


class Console:
    def __init__(self, verbose: bool) -> None:
        self._verbose = verbose

    def log(self, message: str) -> None:
        if self._verbose:
            print(message)


def _serialize(message: bytes) -> str | list[float]:
    return b64encode(message).decode("ascii")


def _get_complete_message(data: bytes) -> tuple[list[bytes], bytes]:
    # ascii protocol seperated by new lines.
    if data.endswith(b"\n") or not data:
        return data.splitlines(), b""
    *messages, trailing = data.splitlines()
    return messages, trailing


class DataLog:
    def __init__(self, id_: str, path: Path) -> None:
        self._id = id_
        self._file = path.open("a")

    def log_send(self, data: bytes) -> None:
        self._log("send", data)

    def log_recv(self, data: bytes) -> None:
        self._log("recv", data)

    def _log(self, type_: str, data: bytes) -> None:
        self._file.write(
            json.dumps(
                {
                    "id": self._id,
                    "type_": type_,
                    "message": _serialize(data),
                    "time": time.time(),
                }
            )
            + "\n"
        )


def _handle_connection(
    data_log: DataLog,
    console: Console,
    original_sock: socket.socket,
    conn: socket.socket,
) -> None:
    while True:
        console.log("select")
        readable, _, exceptional = select.select(
            [conn, original_sock], [], [conn, original_sock]
        )
        conn.setblocking(False)
        for sock in readable:
            console.log("recv: " + ("rrdcached" if sock == conn else "cmc"))
            data = sock.recv(2 * 32)
            if sock == conn:
                console.log(f">> {data.decode('ascii')}")
                data_log.log_send(data)
                if data:
                    original_sock.send(data)
                else:
                    return
            else:
                console.log(f"<< {data!r}")
                data_log.log_recv(data)
                if data:
                    try:
                        conn.send(data)
                    except BrokenPipeError:
                        conn.close()
                        return
                else:
                    sys.exit(1)


def main() -> None:
    config = parse_args()
    console = Console(False)
    original = config.target.with_suffix(".original")
    config.target.rename(original)
    try:
        with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as orginal_sock:
            orginal_sock.connect(str(original))
            with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as sock:
                sock.bind(str(config.target))
                sock.listen()
                i = 0
                while True:
                    conn, addr = sock.accept()
                    console.log(f"accepted connection {addr}")
                    _handle_connection(
                        DataLog(str(i), Path("datalog.jsonl")),
                        console,
                        orginal_sock,
                        conn,
                    )
                    i += 1
    finally:
        original.rename(config.target)


if __name__ == "__main__":
    main()
