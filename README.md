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
* `diskspace` and `omd cp` directly delete/copy files.

## How does Nagios use the RRDs?

Nagios has a plug-in interface known as 'Nagios Event Broker (NEB)'.
These plug-ins are configured via `/omd/sites/{site}/tmp/nagios/nagios.cfg`

```
broker_module=/omd/sites/heute/lib/mk-livestatus/livestatus.o num_client_threads=20 crash_reports_path=/omd/sites/heute/var/check_mk/crashes license_usage_history_path=/omd/sites/heute/var/check_mk/licensing/history.json mk_inventory_path=/omd/sites/heute/var/check_mk/inventory robotmk_html_log_path=/omd/sites/heute/var/robotmk/html_logs mk_logwatch_path=/omd/sites/heute/var/check_mk/logwatch prediction_path=/omd/sites/heute/var/check_mk/prediction state_file_created_file=/omd/sites/heute/var/check_mk/licensing/state_file_created licensed_state_file=/omd/sites/heute/var/check_mk/licensing/licensed_state pnp_path=/omd/sites/heute/var/pnp4nagios/perfdata edition=enterprise /omd/sites/heute/tmp/run/live
broker_module=/omd/sites/heute/lib/npcdmod.o config_file=/omd/sites/heute/etc/pnp4nagios/npcd.cfg
```

`livestatus.o` lives in `packages/neb`, which uses `packages/livestatus` to read the RRDs.
pnp4nagios is an addon to NagiosCore which analyzes performance data provided by plugins and stores them automatically into RRD-databases.

To have a drop-in replacement for RRDs, both componentes would need to replaced.
