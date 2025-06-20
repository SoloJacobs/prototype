from datetime import datetime, timezone

import matplotlib.pyplot as plt

header = [
    "Device",
    "r/s",
    "rkB/s",
    "rrqm/s",
    "%rrqm",
    "r_await",
    "rareq-sz",
    "w/s",
    "wkB/s",
    "wrqm/s",
    "%wrqm",
    "w_await",
    "wareq-sz",
    "d/s",
    "dkB/s",
    "drqm/s",
    "%drqm",
    "d_await",
    "dareq-sz",
    "f/s",
    "f_await",
    "aqu-sz",
    "%util",
]


with open("/tmp/recording.txt") as file:
    content = file.read()
lines = iter(content.splitlines())
line: str | None = next(lines, None)

tss = []

while line is not None:
    if line.startswith("0"):
        ts: dict[str, str | None] = {"time": line}
        line = next(lines, None)
        ts["stats"] = next(lines, None)
        tss.append(ts)
    line = next(lines, None)


def _transform(tss: dict[str, str]) -> list[dict]:
    format_str = "%m/%d/%y %H:%M:%S"
    stats = []
    for ts in tss:
        data = ts["stats"].split()
        stat = dict(zip(header, [data[0]] + [float(d) for d in data[1:]], strict=True))
        stat["time"] = datetime.strptime(ts["time"], format_str)
        stats.append(stat)
    return stats


stats = _transform(tss)
r_s = []
rkB_s = []
rrqm_s = []
_rrqm = []
r_await = []
rareq_sz = []
w_s = []
wkB_s = []
wrqm_s = []
_wrqm = []
w_await = []
wareq_sz = []
d_s = []
dkB_s = []
drqm_s = []
_drqm = []
d_await = []
dareq_sz = []
f_s = []
f_await = []
aqu_sz = []
_util = []

start = datetime.fromisoformat("2025-06-06T17:08:37.107520Z").replace(
    tzinfo=timezone.utc
)
end = datetime.fromisoformat("2025-06-06T17:08:43.629876Z").replace(tzinfo=timezone.utc)

plt.plot(
    [stat["time"].timestamp() for stat in stats], [stat["wkB/s"] for stat in stats]
)
plt.axvline(x=start.timestamp(), color="r", linestyle="--")
plt.axvline(x=end.timestamp(), color="r", linestyle="--")
# plt.plot([stat["time"] for stat in stats], [stat["w/s"] for stat in stats])
plt.show()
