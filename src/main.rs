use clap::{Parser, Subcommand};
use std::fs;
use std::path::{Path, PathBuf};
use tokio::signal::unix::{SignalKind, signal};
use tokio::task::JoinHandle;
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

#[tokio::main]
async fn record_main(_output: &Path, socket: &Path) {
    let _subscriber = tracing_subscriber::fmt()
        .with_env_filter("debug")
        .compact()
        .init();
    let token = CancellationToken::new();
    let handle = setup_signal_handler(token.clone()).await;
    let rename = move_socket(socket);
    tokio::select! {
        _ = handle => (),
        _ = token.cancelled() => (),
    };
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
