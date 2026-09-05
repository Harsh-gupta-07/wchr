use std::{fs::canonicalize, sync::mpsc::channel, thread::spawn};

use ::wchr::{output, runner, wchr};
use clap::Parser;
use notify::Result;
use notify_debouncer_full::DebouncedEvent;

/// Simple program to greet a person
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Command to run on save example. wchr "npm run dev"
    cmd: String,
}

fn main() -> Result<()> {
    let args = Args::parse();

    let watch_dir = canonicalize(".").unwrap();

    if !watch_dir.join(".gitignore").exists() {
        println!(
            "{}",
            output::warning("No .gitignore found; watching all directories.")
        );
    } else {
        println!(
            "{}",
            output::success("Using .gitignore to filter watched files.")
        );
    }

    let (_deb, wchr_rx) = wchr::wchr(&watch_dir)?;
    let (r_tx, r_rx) = channel::<Vec<DebouncedEvent>>();

    spawn(move || runner::cmd_runner(r_rx, watch_dir.clone(), args.cmd.clone()));

    while let Ok(events) = wchr_rx.recv() {
        if r_tx.send(events).is_err() {
            eprintln!("{}", output::error("Command runner disconnected."));
            break;
        }
    }

    Ok(())
}
