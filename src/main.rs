use clap::{Parser, Subcommand};
use std::fs;
use std::path::{Path, PathBuf};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};
use tokio::signal::unix::{SignalKind, signal};
use tokio::task::{JoinHandle, JoinSet};
use tokio_util::sync::CancellationToken;
use tracing::info;
use tracing_subscriber;

#[derive(Parser, Debug)]
struct Arguments {
    #[clap(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug, Clone)]
enum Command {
    Record {
        #[clap(long, short)]
        output: PathBuf,
        #[clap(long, short)]
        socket: PathBuf,
    },
    Replay {
        #[clap(long, short)]
        input: PathBuf,
        #[clap(long, short)]
        socket: PathBuf,
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
        let _ = fs::rename(&self.to, &self.from);
        info!("reset socket");
    }
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
    info!("signal handler setup complete");
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
    let mut count = 0usize;
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
        info!("accepted connection {:?}, {count}", addr);
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
    count: usize,
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
    info!("closing connection {count}");
    let _: (Result<_, _>, Result<_, _>) =
        tokio::join!(from_stream.shutdown(), to_stream.shutdown());
}

#[tokio::main]
async fn record_main(_output: &Path, socket: &Path) {
    tracing_subscriber::fmt()
        .with_env_filter("debug")
        .compact()
        .init();
    let token = CancellationToken::new();
    let handle = setup_signal_handler(token.clone()).await;
    let rename = move_socket(socket);
    let _ = tokio::join!(handle, serve(token, &rename.from, &rename.to));
    drop(rename)
}

fn replay_main(input: &Path, socket: &Path) {
    println!("input: {input:?}, socket: {socket:?}");
}

fn main() {
    let arguments = Arguments::parse();
    match arguments.command {
        Command::Record { output, socket } => record_main(&output, &socket),
        Command::Replay { input, socket } => replay_main(&input, &socket),
    };
}
