use base64::prelude::*;
use clap::{ArgAction, Parser, Subcommand};
use serde::Deserialize;
use serde_json::{from_str, to_string};
use std::fs;
use std::io::BufRead;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::process;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};
use tokio::signal::unix::{SignalKind, signal};
use tokio::task::{JoinHandle, JoinSet};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, trace};
use tracing_subscriber::prelude::*;
use tracing_subscriber::{EnvFilter, fmt};

#[derive(Parser, Debug)]
struct Arguments {
    #[clap(subcommand)]
    command: Command,
    #[arg(short, long, global = true, action = ArgAction::Count)]
    verbose: u8,
}

#[derive(Subcommand, Debug, Clone)]
enum Command {
    Record {
        #[clap(long, short)]
        output: PathBuf,
        #[clap(long, short)]
        socket: PathBuf,
        #[clap(long, short)]
        pidfile: PathBuf,
    },
    Replay {
        #[clap(long, short)]
        input: PathBuf,
        #[clap(long, short)]
        socket: PathBuf,
    },
    Decipher {
        #[clap(long, short)]
        input: PathBuf,
    },
}

struct Rename {
    from: PathBuf,
    to: PathBuf,
}

impl Rename {
    fn new(from: PathBuf, to: PathBuf) -> Self {
        fs::rename(&from, &to).unwrap();
        Self { from, to }
    }
}

impl Drop for Rename {
    fn drop(&mut self) {
        match fs::rename(&self.to, &self.from) {
            Ok(()) => info!("reset socket"),
            Err(e) => error!("could not reset socket: {e:?}"),
        }
    }
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

async fn setup_signal_handler(token: CancellationToken) -> JoinHandle<()> {
    let mut sigint = signal(SignalKind::interrupt()).unwrap();
    let mut sigterm = signal(SignalKind::terminate()).unwrap();
    let mut sighup = signal(SignalKind::hangup()).unwrap();
    let mut sigusr1 = signal(SignalKind::user_defined1()).unwrap();
    let mut sigusr2 = signal(SignalKind::user_defined2()).unwrap();
    let mut sigquit = signal(SignalKind::quit()).unwrap();
    let mut sigalrm = signal(SignalKind::alarm()).unwrap();
    let mut sigpipe = signal(SignalKind::pipe()).unwrap();
    let handle = tokio::spawn(async move {
        tokio::select! {
            _ =  sigint.recv() => (),
            _ =  sigterm.recv() => (),
            _ =  sighup.recv() => (),
            _ =  sigusr1.recv() => (),
            _ =  sigusr2.recv() => (),
            _ =  sigquit.recv() => (),
            _ =  sigalrm.recv() => (),
            _ =  sigpipe.recv() => (),
        };
        info!("signal received");
        token.cancel()
    });
    debug!("signal handler setup complete");
    return handle;
}

fn move_socket(socket: &Path) -> Rename {
    info!("moving socket");
    let original: PathBuf = {
        let mut original_name = socket.as_os_str().to_owned();
        original_name.push(".original");
        original_name.into()
    };
    Rename::new(socket.into(), original)
}

async fn serve(token: CancellationToken, from: &Path, to: &Path) {
    let mut set = JoinSet::new();
    let mut count = 0u64;
    let listener = UnixListener::bind(from).unwrap();
    info!("listening to {}", from.as_os_str().to_string_lossy());
    loop {
        let to_stream = tokio::select! {
            to_stream = UnixStream::connect(to) => to_stream.unwrap(),
            _ = token.cancelled() => break,
        };
        let (from_stream, addr) = tokio::select! {
            from_stream = listener.accept() => from_stream.unwrap(),
            _ = token.cancelled() => break,
        };
        count += 1;
        debug!(id = count, "accepted connection {:?}", addr);
        set.spawn(forward_traffic(
            count,
            token.clone(),
            from_stream,
            to_stream,
        ));
    }
    info!("awaiting connections");
    set.join_all().await;
    info!("all connections closed");
}

async fn forward_traffic(
    id: u64,
    token: CancellationToken,
    mut from_stream: UnixStream,
    mut to_stream: UnixStream,
) {
    while !token.is_cancelled() {
        let mut from_buf = [0u8; 1024];
        let mut to_buf = [0u8; 1024];
        tokio::select! {
            from_read = from_stream.read(&mut from_buf) => {
                let n = from_read.unwrap();
                    let message: &str = &BASE64_STANDARD.encode(&from_buf[..n]);
                    trace!(type_="send", id=id, message=message);
                    tokio::select! {
                        write = to_stream.write_all(&from_buf[..n]) => write.unwrap(),
                        _ = token.cancelled() => break,
                    };
                    if n == 0 {
                        break;
                    };
                },
            to_read = to_stream.read(&mut to_buf) => {
                let n = to_read.unwrap();
                    let message: &str = &BASE64_STANDARD.encode(&to_buf[..n]);
                    trace!(type_="recv", id=id, message=message);
                    tokio::select! {
                        write = from_stream.write_all(&to_buf[..n]) => write.unwrap(),
                        _ = token.cancelled() => break,
                    }
                    if n == 0 {
                        break;
                    };
                },
            _ = token.cancelled() => break,
        };
    }
    debug!(id = id, "closing connection");
    let (shutdown_from, shutdown_to) = tokio::join!(from_stream.shutdown(), to_stream.shutdown());
    shutdown_to.map_err(|e| error!(id = id, "to {e:?}")).ok();
    shutdown_from
        .map_err(|e| error!(id = id, "from {e:?}"))
        .ok();
}

#[tokio::main]
async fn record_main(stdout_filter: EnvFilter, output: &Path, socket: &Path, pidfile: &Path) {
    let stdout_layer = fmt::Layer::default().compact().with_filter(stdout_filter);
    let pid = process::id().to_string();
    fs::write(pidfile, pid).unwrap();
    let file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(output)
        .unwrap();

    let json_layer = fmt::Layer::default()
        .json()
        .with_writer(file)
        .with_level(false)
        .with_target(false)
        .with_filter(EnvFilter::new("spy[{type_}]"));
    tracing_subscriber::registry()
        .with(stdout_layer)
        .with(json_layer)
        .init();
    let token = CancellationToken::new();
    let handle = setup_signal_handler(token.clone()).await;
    let rename = move_socket(socket);
    let _ = tokio::join!(handle, serve(token, &rename.from, &rename.to));
    drop(rename)
}

fn xchange_timestamp_update(message: &str) -> String {
    let mut result = message.to_string();
    if result.starts_with("UPDATE") {
        result = result.replace("/opt/omd/sites/ll/var/check_mk/rrd", "/tmp/rrd");
        result = result.replace("rrd 174", "rrd 205");
    }
    result
}

#[tokio::main]
async fn replay_main(stdout_filter: EnvFilter, input: &Path, socket: &Path) {
    let stdout_layer = fmt::Layer::default().compact().with_filter(stdout_filter);
    tracing_subscriber::registry().with(stdout_layer).init();
    let file = fs::File::open(input).unwrap();
    let mut stream = UnixStream::connect(socket).await.unwrap();
    info!("connected to {}", &socket.to_string_lossy());
    let mut buf = [0u8; 1024];
    let mut commands = String::new();
    for (line, line_count) in BufReader::new(file).lines().zip(0..) {
        let log: Log = from_str(&line.unwrap()).unwrap();
        debug!(
            "{line_count}, {}, {}",
            &log.timestamp,
            &to_string(&log.fields.type_).unwrap()
        );
        match log.fields.type_ {
            Type_::Send => {
                let bytes = BASE64_STANDARD.decode(&log.fields.message).unwrap();
                commands += from_ascii(&bytes).unwrap();
                let commands_clone = commands.clone();
                let rrd_commands: Vec<&str> = commands_clone.split('\n').collect();
                commands = rrd_commands.last().unwrap().to_string();

                for &rrd_command in rrd_commands.iter() {
                    let mut modified = xchange_timestamp_update(rrd_command);
                    if modified.is_empty() {
                        continue;
                    }
                    modified.push('\n');
                    info!("sent: '{modified}'");
                    stream.write_all(modified.as_bytes()).await.unwrap();
                }
            }
            Type_::Recv => {
                let _ = stream.read(&mut buf).await;
            }
        };
    }
}

fn from_ascii(message: &[u8]) -> Option<&str> {
    if message.iter().all(u8::is_ascii) {
        return std::str::from_utf8(message).ok();
    }
    None
}

#[tokio::main]
async fn decipher_main(stdout_filter: EnvFilter, input: &Path) {
    let stdout_layer = fmt::Layer::default().compact().with_filter(stdout_filter);
    tracing_subscriber::registry().with(stdout_layer).init();
    let file = fs::File::open(input).unwrap();
    for (line, _line_count) in BufReader::new(file).lines().zip(0..) {
        let log: Log = from_str(&line.unwrap()).unwrap();
        let bytes = BASE64_STANDARD.decode(&log.fields.message).unwrap();
        let prompt = match log.fields.type_ {
            Type_::Send => ">>",
            Type_::Recv => "<<",
        };
        println!("{prompt} connection {}", log.fields.id);
        for message in bytes.split(|&b| b == b'\n') {
            match from_ascii(message) {
                Some("") => continue,
                Some(m) => println!("{m}"),
                None => println!("non-ascii message of length {}", message.len()),
            };
        }
    }
}

fn main() {
    let arguments = Arguments::parse();
    let filter = EnvFilter::new(match arguments.verbose {
        0 => "info",
        1 => "debug",
        _ => "trace",
    });
    match arguments.command {
        Command::Record {
            output,
            socket,
            pidfile,
        } => record_main(filter, &output, &socket, &pidfile),
        Command::Replay { input, socket } => replay_main(filter, &input, &socket),
        Command::Decipher { input } => decipher_main(filter, &input),
    };
}
