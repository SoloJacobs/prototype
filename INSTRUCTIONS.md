1. If possible copy `var/check_mk/rrd`. Otherwise, `.info` files should be sufficient, e.g.,
```sh
find var/check_mk/rrd/ -type f -name "*.info" -exec cp {} /tmp/rrd/ \;
tar -c /tmp/rrd -f rrd_info.archive
```
2. Install `spy` into bin. Show `rrdcached.socket.original`.
```
spy record -s tmp/run/rrdcached.sock -o var/datalog.jsonl -p rrd.pid -vvv
```
If the  file exists, you get this error:
```
thread 'main' panicked at src/main.rs:201:9:
user error, output file exists: tmp/datalog.jsonl
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
```
Terminate with CTRL-C. If the socket can't be reset, you get this error:
```
2025-05-22T09:00:46.727416Z ERROR spy: could not reset socket: Os { code: 2, kind: NotFound, message: "No such file or directory" }
```
This error *must* not be ignored. Try restarting with `omd restart rrdcached` and test it with 
```sh
etc/init.d/rrdcached flush
$ etc/init.d/rrdcached flush 
Triggering global flush of rrdcached...cannot connect to UNIX-socket at '/omd/sites/v250/tmp/run/rrdcached.sock': Connection refused
failed: 
$ omd restart rrdcached
Stopping rrdcached...waiting for termination...OK
Starting rrdcached...OK
$ etc/init.d/rrdcached flush
Triggering global flush of rrdcached...OK
```
3. Restart the core after finally installing `spy` with `omd restart cmc`. Otherwise, no update logs are written until the core reconnects.
```
spy decipher -i tmp/datalog.jsonl
```
4. Emulate broken socket:
```
omd stop rrdcached
socat UNIX-LISTEN:tmp/run/rrdcached.socket,fork exec:'cat >/dev/null'
```

Warning signs:
If the graphs are `Loading graphs...` and waiting, then there is likely something wrong with forwarding the traffic.
5. Lookout for unattended updates.
