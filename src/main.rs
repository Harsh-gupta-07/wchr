use std::{fs::canonicalize, path::{Path, PathBuf}, sync::mpsc::channel};

use clap::Parser;
use notify::{Config, Event, EventKind, RecommendedWatcher, Result, Watcher};

/// Simple program to greet a person
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    path: PathBuf,

    /// files to ignore
    #[arg(short, long)]
    ignore: Vec<PathBuf>,
}

fn main() -> Result<()> {
    let args = Args::parse();

    println!("{:?}", args.ignore);

    let (tx, rx) = channel();

    let mut watcher = RecommendedWatcher::new(
        move |res: Result<Event>| {
            tx.send(res).unwrap();
        },
        Config::default(),
    )?;

    watcher.watch(&canonicalize(".").unwrap(), notify::RecursiveMode::Recursive)?;

    println!("Watching {:?}", canonicalize(".").unwrap());
    
    for result in rx{
        match result {
            Ok(r)=>handle(r),
            Err(err)=> eprintln!("Error: {err}")
        }
    }

    Ok(())
}

fn handle(r:Event){
    let mut ig = canonicalize("./target").unwrap();

    if matches!(r.kind, EventKind::Modify(_)){
        for path in &r.paths{
            if !path.starts_with(&ig){
                println!("{:?}", r);
            }
        }
    }
}
