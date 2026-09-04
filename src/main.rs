use std::path::PathBuf;

use clap::Parser;

/// Simple program to greet a person
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    path: PathBuf,

    /// files to ignore
    #[arg(short, long)]
    ignore: Vec<PathBuf>
}

fn main() {
    let args = Args::parse();

    println!("{:?}", args.ignore);
}