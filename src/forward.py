#!/usr/bin/env python3
import socket
import subprocess
import sys
from argparse import ArgumentParser
from dataclasses import dataclass
from pathlib import Path

import psutil


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


def parse_proc_net_unix() -> list[dict[str, str]]:
    """Parse /proc/net/unix into structured data"""
    entries = []

    with open("/proc/net/unix") as f:
        # Skip header line
        next(f)

        for line in f:
            fields = line.strip().split()

            entry = {
                "num": fields[0],  # Kernel address (hex)
                "ref_count": fields[1],
                "protocol": fields[2],
                "flags": fields[3],  # Hex flags
                "type": fields[4],  # Socket type (e.g., STREAM)
                "state": fields[5],  # Socket state (e.g., CONNECTED)
                "inode": fields[6],  # Inode number
                "path": fields[7] if len(fields) > 7 else "",
            }
            entries.append(entry)

    return entries


def parse_lsof_unix(target: Path) -> list[dict[str, str]]:
    call = subprocess.run(
        ["lsof", str(target)], capture_output=True, text=True, check=False
    )

    lines = call.stdout.splitlines()[1:]
    entries = []
    for line in lines:
        command, pid, user, fd, type_, device, size, node, *name = line.split()
        entries.append(
            {
                "command": command,
                "pid": pid,
                "user": user,
                "fd": fd,
                "type": type_,
                "device": device,
                "size": size,
                "node": node,
                "name": " ".join(name),
            }
        )

    return entries


def get_process_info(pid: int) -> dict[str, object]:
    try:
        p = psutil.Process(pid)
        return {
            "pid": pid,
            "name": p.name(),
            "status": p.status(),
            "cmdline": p.cmdline(),
            "exe": p.exe(),
            "username": p.username(),
            "create_time": p.create_time(),
            "parent_pid": p.ppid(),
        }
    except psutil.NoSuchProcess:
        return {"error": f"Process {pid} doesn't exist"}


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
