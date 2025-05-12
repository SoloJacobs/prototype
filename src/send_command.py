#!/usr/bin/env python3
import socket
from pathlib import Path

COMMANDS = [
    b"STATS\n",
    b"FETCH /opt/omd/sites/v240/var/check_mk/rrd/v240/Site_v240_statistics.rrd MAX 1746707229 1746797229 7\n",
    b"FETCHBIN /opt/omd/sites/v240/var/check_mk/rrd/v240/Site_v240_statistics.rrd MAX 1746707229 1746797229 7\n",
    b"UPDATE /tmp/Check_MK.rrd value N:1\n",
    b"FETCH /tmp/Check_MK.rrd AVERAGE\n",
    b"FETCHBIN /tmp/Check_MK.rrd AVERAGE\n",
    b"FETCH /opt/omd/sites/v240/var/check_mk/rrd/server-linux-ipmi-4/Filesystem__home.rrd MAX 1746889885 1746904285 4\n",
]


def main() -> None:
    target = Path("rrdcached.sock")
    data = b""
    with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as s:
        s.connect(str(target))
        s.sendall(COMMANDS[3])
        while True:
            data += s.recv(2**32)
            print(f"{data!r}")



if __name__ == "__main__":
    main()
