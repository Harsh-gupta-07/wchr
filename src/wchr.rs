use std::{
    path::PathBuf, process, sync::mpsc::{Receiver, channel}, time::Duration,
};

use notify::{Error, RecursiveMode};
use notify_debouncer_full::{
    DebounceEventResult, DebouncedEvent, Debouncer, RecommendedCache, new_debouncer,
};

use crate::output;

pub fn wchr(
    watch_dir: &PathBuf,
) -> Result<
    (
        Debouncer<notify::RecommendedWatcher, RecommendedCache>,
        Receiver<Vec<DebouncedEvent>>,
    ),
    Error,
> {
    println!(
        "{}",
        output::info(format!("Watching directory: {}", watch_dir.display()))
    );

    let (tx, rx) = channel();

    let mut debouncer = new_debouncer(
        Duration::from_secs(1),
        None,
        move |result: DebounceEventResult| match result {
            Ok(events) => {
                if let Err(err) = tx.send(events) {
                    eprintln!(
                        "{:?}",
                        output::error(format!("Failed to send file-change events: {err}"))
                    );
                    process::exit(1);
                }
            }
            Err(err) => {
                let errors = err
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("; ");
                eprintln!("{}", output::error(format!("File watcher error: {errors}")));
                process::exit(1);
            }
        },
    )?;

    debouncer.watch(watch_dir, RecursiveMode::Recursive)?;

    println!(
        "{}",
        output::success(format!("Watching {} for changes...", watch_dir.display()))
    );

    Ok((debouncer, rx))
}
