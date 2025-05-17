use clap::{Parser, Subcommand};
use std::path::PathBuf;

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

fn main() {
    let arguments = Arguments::parse();
    println!("arg: {arguments:?}");
}
