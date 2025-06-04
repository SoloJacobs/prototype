use base64::prelude::*;
use chrono::{DateTime, Utc};
use clap::{ArgAction, Parser, Subcommand};
use serde::Deserialize;
use serde_json::from_str;
use sqlx::{Connection, Executor, PgConnection};
use std::collections::HashMap;
use std::fs;
use std::io::BufRead;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use tokio::sync::mpsc;
use tokio::sync::mpsc::{Receiver, Sender};
use tracing::{debug, error, info, trace};
use tracing_subscriber::prelude::*;
use tracing_subscriber::{EnvFilter, fmt};

struct RRDMetric {
    time: i64,
    path: String,
    // host: String,
    // service: String,
    name: String,
    value: Option<f64>,
}

#[derive(Parser, Debug)]
struct Arguments {
    #[clap(subcommand)]
    command: Command,
    #[arg(short, long, global = true, action = ArgAction::Count)]
    verbose: u8,
}

#[derive(Subcommand, Debug, Clone)]
enum Command {
    Decipher {
        #[clap(long, short)]
        input: PathBuf,
    },
}

#[derive(Deserialize, serde::Serialize)]
#[serde(rename_all = "lowercase")]
enum Type_ {
    Send,
    Recv,
}

#[derive(Deserialize)]
struct Fields {
    type_: Type_,
    id: u64,
    message: String,
}

#[derive(Deserialize)]
struct Log {
    timestamp: String,
    fields: Fields,
}

fn from_ascii(message: &[u8]) -> Option<&str> {
    if message.iter().all(u8::is_ascii) {
        return std::str::from_utf8(message).ok();
    }
    None
}

struct UpdateMessage {
    time: i64,
    path: String,
    metrics: Vec<Option<f64>>,
}

fn parse_float(metric: &str) -> anyhow::Result<Option<f64>> {
    if metric == "U" {
        return Ok(None);
    }
    Ok(Some(metric.parse()?))
}

fn parse(message: &str) -> anyhow::Result<UpdateMessage> {
    let mut parts = message.splitn(3, ' ');
    let _command = parts.next().unwrap();
    let path = parts.next().unwrap();
    let data_str = parts.next().unwrap();
    let mut data_values = data_str.split(':');
    let timestamp: i64 = data_values.next().unwrap().parse()?;
    let metrics: anyhow::Result<Vec<Option<f64>>> =
        data_values.into_iter().map(parse_float).collect();

    return Ok(UpdateMessage {
        time: timestamp,
        path: path.into(),
        metrics: metrics?,
    });
}

async fn decipher(input: &Path, tx: Sender<UpdateMessage>) {
    let file = fs::File::open(input).unwrap();
    let mut update_count = 0;
    for (line, _line_count) in BufReader::new(file).lines().zip(0..) {
        let log: Log = from_str(&line.unwrap()).unwrap();
        let bytes = BASE64_STANDARD.decode(&log.fields.message).unwrap();
        let prompt = match log.fields.type_ {
            Type_::Send => ">>",
            Type_::Recv => continue,
        };
        trace!("{prompt} connection {}", log.fields.id);
        for message in bytes.split(|&b| b == b'\n') {
            match from_ascii(message) {
                Some("") => continue,
                Some(m) => {
                    trace!("{m}");
                    if m.starts_with("UPDATE") {
                        match parse(m) {
                            Ok(update) => tx.send(update).await.unwrap(),
                            Err(e) => {
                                error!("Could not parse: {m}, {e}")
                            }
                        };
                        update_count += 1
                    }
                }
                None => debug!("non-ascii message of length {}", message.len()),
            };
        }
        if update_count % 1000 == 0 {
            info!("processed {update_count}");
        }
    }
    info!("update_count: {update_count}");
}

async fn create_metric(
    metrics: &HashMap<String, i32>,
    conn: &mut PgConnection,
    update: UpdateMessage,
) -> Result<(), sqlx::Error> {
    let time = DateTime::<Utc>::from_timestamp(update.time, 0).expect("Invalid UNIX timestamp");
    let values = update
        .metrics
        .into_iter()
        .zip(0..)
        .map(|(metric, i)| match metric {
            Some(value) => format!("($1, {i}, $2, {value})"),
            None => format!("($1, '{i}', $2, NULL)"),
        })
        .collect::<Vec<String>>()
        .join(",");
    let id = metrics[&update.path];
    let query_str = format!(
        "
        INSERT INTO metrics (partition, name, time, value)
          VALUES {values}
        "
    );

    let query = sqlx::query(&query_str).bind(id).bind(time);
    let result = conn.execute(query).await?;
    trace!("{:#?}", result);
    Ok(())
}

async fn create_table() -> Result<(), sqlx::Error> {
    // We really want to save this instead
    //  host     TEXT              NOT NULL,
    //  service  TEXT              NOT NULL,
    let query = sqlx::query(
        "
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
    ",
    );
    let conn_string = "postgres://postgres:password@localhost/postgres";
    let mut conn = PgConnection::connect(conn_string).await?;
    let result = conn.execute(query).await?;
    trace!("{:#?}", result);
    Ok(())
}

async fn create_partitions(
    mut rx: Receiver<UpdateMessage>,
) -> Result<HashMap<String, i32>, sqlx::Error> {
    let mut seen_metrics = HashMap::new();
    let mut unique_id = 0;

    while let Some(UpdateMessage {
        time,
        path,
        metrics,
    }) = rx.recv().await
    {
        if !seen_metrics.contains_key(&path) {
            seen_metrics.insert(path, unique_id);
            unique_id += 1;
        }
    }
    // We really want to save this instead
    //  host     TEXT              NOT NULL,
    //  service  TEXT              NOT NULL,
    Ok(seen_metrics)
}

async fn create_metrics(metrics: &HashMap<String, i32>, mut rx: Receiver<UpdateMessage>) {
    let conn_string = "postgres://postgres:password@localhost/postgres";
    let mut conn = PgConnection::connect(conn_string).await.unwrap();
    while let Some(update) = rx.recv().await {
        if let Err(e) = create_metric(&metrics, &mut conn, update).await {
            error!("could not create metric {e:?}");
        }
    }
    info!("finished processing");
}

async fn create_metrics_from_file(
    input: &Path,
    metrics: &HashMap<String, i32>,
) -> Result<(), sqlx::Error> {
    let (tx, rx) = mpsc::channel::<UpdateMessage>(32);
    let (_send, _cons) = tokio::join!(decipher(&input, tx), create_metrics(metrics, rx),);
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), sqlx::Error> {
    let arguments = Arguments::parse();
    let filter = EnvFilter::new(match arguments.verbose {
        0 => "info",
        1 => "debug",
        _ => "trace",
    });
    let stderr_layer = fmt::Layer::default()
        .compact()
        .with_writer(std::io::stderr)
        .with_filter(filter);
    tracing_subscriber::registry().with(stderr_layer).init();
    let Command::Decipher { input } = arguments.command;

    let (tx, rx) = mpsc::channel::<UpdateMessage>(32);
    create_table().await?;
    let (_send, cons) = tokio::join!(decipher(&input, tx), create_partitions(rx));
    let metrics = cons.unwrap();
    info!("finished partition");

    create_metrics_from_file(&input, &metrics).await
}
