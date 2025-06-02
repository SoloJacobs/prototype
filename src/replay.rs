use base64::prelude::*;
use chrono::{DateTime, NaiveDateTime, Utc};
use clap::{ArgAction, Parser, Subcommand};
use serde::Deserialize;
use serde_json::{from_str, to_string};
use sqlx::{Connection, Executor, FromRow, PgConnection};
use std::fs;
use std::io::BufRead;
use std::io::BufReader;
use std::path::{Path, PathBuf};
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

async fn decipher(stdout_filter: EnvFilter, input: &Path) -> Vec<UpdateMessage> {
    let stdout_layer = fmt::Layer::default()
        .compact()
        .with_writer(std::io::stderr)
        .with_filter(stdout_filter);
    tracing_subscriber::registry().with(stdout_layer).init();
    let file = fs::File::open(input).unwrap();
    let mut update_count = 0;
    let mut updates: Vec<UpdateMessage> = Vec::new();
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
                            Ok(update) => updates.push(update),
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
    }
    info!("update_count: {update_count}");
    return updates;
}

async fn create_metric(metric: RRDMetric) -> Result<(), sqlx::Error> {
    let conn_string = "postgres://postgres:password@localhost/postgres";
    let mut conn = PgConnection::connect(conn_string).await?;
    let time = DateTime::<Utc>::from_timestamp(metric.time, 0).expect("Invalid UNIX timestamp");
    let query = sqlx::query(
        "
        INSERT INTO metrics (time, path, name, value)
          VALUES ($1, $2, $3, $4)
        ",
    )
    .bind(time)
    .bind(&metric.path)
    .bind(&metric.name)
    .bind(metric.value);
    trace!("{:#?}", conn.execute(query).await?);
    Ok(())
}

async fn create_table() -> Result<(), sqlx::Error> {
    // We really want to save this instead
    //  host     TEXT              NOT NULL,
    //  service  TEXT              NOT NULL,
    let query = sqlx::query(
        "
    CREATE TABLE metrics (
      time     TIMESTAMPTZ       NOT NULL,
      path     TEXT              NOT NULL,
      name     TEXT              NOT NULL,
      value    DOUBLE PRECISION,
      PRIMARY KEY (path, name, time)
    ) WITH (
      timescaledb.hypertable,
      timescaledb.partition_column='time',
      timescaledb.segmentby='path, name'
    );
    ",
    );
    let conn_string = "postgres://postgres:password@localhost/postgres";
    let mut conn = PgConnection::connect(conn_string).await?;
    println!("{:#?}", conn.execute(query).await?);
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
    let updates = match arguments.command {
        Command::Decipher { input } => decipher(filter, &input).await,
    };
    // create_table().await?;
    for update in updates {
        for (metric, i) in update.metrics.into_iter().zip(1..) {
            let rrd_metric = RRDMetric{
                time: update.time,
                path: update.path.clone(),
                name: format!("{i}"),
                value: metric,
            };
            create_metric(rrd_metric).await?;
        }
    }
    Ok(())
}
