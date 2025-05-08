#!/usr/bin/env python3
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
    def __init__(self) -> None:
        pass

    def log(self, message: str) -> None:
        print(message)


def _handle_connection(
    original: Path,
    console: Console,
    conn: socket.socket,
    addr: socket.AddressInfo,
) -> None:
    console.log(f"Accepted connection {addr}")
    with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as orginal_sock:
        orginal_sock.connect(str(original))
        while True:
            data = conn.recv(1024)
            if data:
                orginal_sock.sendall(data)
            else:
                return
            origin_data = orginal_sock.recv(1024)
            if origin_data:
                conn.sendall(origin_data)
            else:
                sys.exit(1)


def main() -> None:
    config = parse_args()
    console = Console()
    original = config.target.with_suffix(".original")
    config.target.rename(original)
    try:
        with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as sock:
            sock.bind(str(config.target))
            sock.listen()
            while True:
                conn, addr = sock.accept()
                _handle_connection(original, console, conn, addr)
    finally:
        original.rename(config.target)


if __name__ == "__main__":
    main()
