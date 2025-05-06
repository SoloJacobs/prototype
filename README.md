Source code, which should not go into the main repository.

## What are typical interactions of Checkmk with rrdcached?

This is only a ruff overview, and does attempt to give a complete list.
We can use `auditctl` as follows to find processes, which connect to `rrdcached.sock` as follows:

```sh
auditctl -a exit,always -F arch=b64 -S connect -F path=/opt/omd/sites/v250/tmp/run/rrdcached.sock -k test_audit_final
```

This gives the following list of processes:
```
/omd/versions/2.5.0-2025.05.05.cee/bin/python3/omd/sites/v250/bin/omd backup test
/omd/sites/v250/bin/cmc /omd/sites/v250/var/check_mk/core/config.pb
/usr/bin/perl /omd/sites/v250/lib/pnp4nagios/process_perfdata.pl -n -c /omd/sites/v250/etc/pnp4nagios/process_perfdata.cfg -b /o
```

Hovering over the graphs triggers a `FLUSH` and a `FETCHBIN`, however this happens via the socket which the core has open.
`omd backup` triggers a `SUSPEND`, the core sends an `UPDATE` signal.
`pnp4nagios` seems to be used by `npcd`.

From reading the source code:

* `omd/packages/rrdtool/skel/etc/init.d/rrdcached flush` will send `FLUSHALL` to `rrdcached` daemon. 
* `get_rrd_cache_stats` in doc treasures will send `STATS`.
* `packages/livestatus/src/RRDColumn.cc` provides a read-only interface for reading the RRDs.
* `packages/cmc/src/Core.cc:core::processPerfdata` provides a write interface for the RRDs.
* Host renaming uses the `rrdcached.sock` to determine whether the `daemon` is running.
* `cmk-create-rrd` is responsible for creating rrds (but unclear how it is called)
* `cmk-convert-rrds` can convert RRDs from nagios format to Checkmk format.
