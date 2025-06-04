# TimescaleDB performance

* We only benchmarked updates.
* No indexing was done on the table. Indexing will likely slow down inserts, so we did not attempt adding them.
  To make things easier on the DB, we also held all rrd `paths` in memory in a different process, and only passed integer ids to the database itself.
```
    CREATE TABLE metrics (
      partition BIGINT            NOT NULL,
      name      BIGINT            NOT NULL,
      time      TIMESTAMPTZ       NOT NULL,
      value     DOUBLE PRECISION,
      PRIMARY KEY (partition, name, time)
    ) WITH (
      timescaledb.hypertable,
      timescaledb.partition_column='time',
      timescaledb.segmentby='partition, name'
    );
```
* The total operation took 5199 seconds. BLOCK I/O was reported to be 11.6MB.
  However, the final size of the `pgdata` folder was 3.7GB. If one store the rrd `paths` additionally, then the total memory would be at minimum 7GB. This means the database writes don't appear to be captured by `docker stats`. `pidstat` also only reported `70MB` worth of writes. It is likely that we should improve these metrics in the future, and they are likely meaningless for the rrdcached daemon as well.
* It was also attempt manually create partitions:
```
CREATE TABLE metrics_{unique_id} PARTITION of metrics
  FOR VALUES IN ({unique_id});
```
 However, TimescaleDB also uses partitions under the hood. Our hand-rolled implementation did not terminate after two hours (no metric writes done, yet). So, we decided to abandon this approach. We assume that TimescaleDB has properly engineered their solution, and the true limitation is coming from storing the data in PostGre.
* Another approach was attempted to only segment by the rrd 'path` and not by the metric:
```
    CREATE TABLE metrics (
      partition BIGINT            NOT NULL,
      name      BIGINT            NOT NULL,
      time      TIMESTAMPTZ       NOT NULL,
      value     DOUBLE PRECISION,
      PRIMARY KEY (partition, name, time)
    ) WITH (
      timescaledb.hypertable,
      timescaledb.partition_column='time',
      timescaledb.segmentby='partition'
    );
```
  This appeared to make little difference in the overall performance. The total time taken was 5072 seconds.
* We are confident that the low performance is not due to something specific to containerized environment (i.e., the communication via TCP socket with the database rather than a unix socket). This can be seen by running the same test with a database transaction. In this case the test takes around 900 seconds, i.e., it is much faster. Thus, the total overhead caused by the database is 5199 - 900 = 4199 seconds. However, utilizing transactions is not feasible, since fetching from the database would return stale data.
 Moreover, other IO benchmarks will increase the time taken by the benchmark (i.e., creating rrd files). This another indicator, the I/O is disk bound.
* The RAM for the database went to 3GB during the inserts. It never went down afterwards. If the path data was included, then the usage went to 4GB. CPU % was at 14.82% with some fluctuations.

# RRDcached Daemon

* Since we did not capture the actual disk I/O, we also report the takes to write all RRD data.
 1. rrd creation: 16s
 2. writing all updates to disk: 54s
* Heap usage was 14Mb at its peak. CPU% usage were 37.2% for the first trial and 0.3% for the
  second trial. 
