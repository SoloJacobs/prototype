# rrdcached -t 4 -w 3600 -z 1800 -f 7200 -s v250 -m 660 -l unix:/omd/sites/v250/tmp/run/rrdcached.sock -p /omd/sites/v250/tmp/rrdcached.pid -j /omd/sites/v250/var/rrdcached -o /omd/sites/v250/var/log/rrdcached.log

GROUP_OWNER=solo

valgrind --tool=massif ./rrdcached -g -t 4 -w 2 -z 1 -f 4 -U solo -s "$GROUP_OWNER" -m 660 \
  -l unix:rrdcached.sock \
  -p rrdcached.pid \
  -j journal \
  -V LOG_DEBUG \
  -o rrdcached.log

