#!/usr/bin/env python3
import select
import socket
import sys
from argparse import ArgumentParser
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


def _handle_connection(
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
            console.log("recv: " + "rrdcached" if sock == conn else "cmc")
            data = sock.recv(1024)
            if sock == conn:
                console.log(f">> {data.decode('ascii')}")
                if data:
                    original_sock.send(data)
                else:
                    return
            else:
                console.log(f"<< {data!r}")
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
    console = Console(True)
    original = config.target.with_suffix(".original")
    config.target.rename(original)
    try:
        with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as orginal_sock:
            orginal_sock.connect(str(original))
            with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as sock:
                sock.bind(str(config.target))
                sock.listen()
                while True:
                    conn, addr = sock.accept()
                    console.log(f"accepted connection {addr}")
                    _handle_connection(console, orginal_sock, conn)
    finally:
        original.rename(config.target)


if __name__ == "__main__":
    main()
