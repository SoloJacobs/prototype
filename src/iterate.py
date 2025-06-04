import json
import os
import time
from dataclasses import dataclass
from pathlib import Path

import rrdtool


@dataclass(frozen=True)
class RRDInfo:
    host: str
    service: str
    metrics: list[str]


def _parse_cmc_rrd_info(info_file_path: str) -> RRDInfo:
    with open(info_file_path) as file:
        content = file.read()
    host_line, service_line, metrics_line = content.splitlines()
    return RRDInfo(
        host=host_line[5:],
        service=service_line[8:],
        metrics=metrics_line[8:].split(";"),
    )


def pnp_cleanup(s: str) -> str:
    """Quote a string (host name or service name) in PNP4Nagios format

    Because it is used as path element, this needs to be handled as "str" in Python 2 and 3
    """
    return s.replace(" ", "_").replace(":", "_").replace("/", "_").replace("\\", "_")


def _rrd_cmc_host_dir(hostname: str) -> str:
    # We need /opt here because of bug in rrdcached
    return str(Path("/tmp/rrd") / pnp_cleanup(hostname))


# RRDSpec(
#     format = "cmc_single"
#     host: HostName
#     service: _RRDServiceName
#     metrics: Sequence[tuple[str, str | None]]


def _create_rrd(info: RRDInfo) -> str:
    heartbeat = 8460
    step = 60
    host_dir = _rrd_cmc_host_dir(info.host)
    base_file_name = host_dir + "/" + pnp_cleanup(info.service)

    rrd_file_name = base_file_name + ".rrd"

    os.makedirs(host_dir, exist_ok=True)

    args = [rrd_file_name, "--step", str(step)]
    for nr, _varname in enumerate(info.metrics, 1):
        args.append(f"DS:{nr}:GAUGE:{heartbeat}:U:U")
    args += [
        "RRA:AVERAGE:0.50:1:2880",
        "RRA:AVERAGE:0.50:30:4320",
        "RRA:AVERAGE:0.50:360:5840",
        "RRA:AVERAGE:0.50:5:2880",
        "RRA:MAX:0.50:1:2880",
        "RRA:MAX:0.50:30:4320",
        "RRA:MAX:0.50:360:5840",
        "RRA:MAX:0.50:5:2880",
        "RRA:MIN:0.50:1:2880",
        "RRA:MIN:0.50:30:4320",
        "RRA:MIN:0.50:360:5840",
        "RRA:MIN:0.50:5:2880",
    ]

    rrdtool.create(*args)

    return rrd_file_name


@dataclass(frozen=True, slots=True)
class TS:
    path: Path
    metric_count: int


def _create_rrd_from_ts(ts: TS) -> str:
    heartbeat = 8460
    step = 60

    rrd_file = Path(
        str(ts.path).replace("/opt/omd/sites/prod/var/check_mk/rrd/", "/tmp/rrd/")
    )
    os.makedirs(rrd_file.parent, exist_ok=True)
    args = [str(rrd_file), "--step", str(step)]
    for nr in range(1, ts.metric_count + 1):
        args.append(f"DS:{nr}:GAUGE:{heartbeat}:U:U")
    args += [
        "RRA:AVERAGE:0.50:1:2880",
        "RRA:AVERAGE:0.50:30:4320",
        "RRA:AVERAGE:0.50:360:5840",
        "RRA:AVERAGE:0.50:5:2880",
        "RRA:MAX:0.50:1:2880",
        "RRA:MAX:0.50:30:4320",
        "RRA:MAX:0.50:360:5840",
        "RRA:MAX:0.50:5:2880",
        "RRA:MIN:0.50:1:2880",
        "RRA:MIN:0.50:30:4320",
        "RRA:MIN:0.50:360:5840",
        "RRA:MIN:0.50:5:2880",
    ]

    rrdtool.create(*args)

    return str(rrd_file)


def main() -> None:
    rrd = "/tmp/metrics"
    tss = []
    with open(rrd) as file:
        for line in file:
            ts_dict = json.loads(line)
            tss.append(TS(path=ts_dict["path"], metric_count=ts_dict["metric_count"]))
    print(time.time())
    for ts in tss:
        _create_rrd_from_ts(ts)
    print(time.time())


main()
