use std::{
    path::PathBuf,
    sync::mpsc::{channel, Receiver},
    time::Duration,
};

use notify::{Error, RecursiveMode};
use notify_debouncer_full::{
    DebounceEventResult, DebouncedEvent, Debouncer, RecommendedCache, new_debouncer,
};

pub fn wchr(
    watch_dir: &PathBuf,
) -> Result<
    (
        Debouncer<notify::RecommendedWatcher, RecommendedCache>,
        Receiver<Vec<DebouncedEvent>>,
    ),
    Error,
> {
    println!("Current Directory: {:?}", watch_dir);

    let (tx, rx) = channel();

    let mut debouncer = new_debouncer(
        Duration::from_secs(1),
        None,
        move |result: DebounceEventResult| {
            match result {
                Ok(events) => {
                    if let Err(err) = tx.send(events) {
                        eprintln!("Failed to send events: {err}");
                    }
                }
                Err(err) => {
                    eprintln!("{err:?}");
                }
            }
        },
    )?;

    debouncer.watch(watch_dir, RecursiveMode::Recursive)?;

    println!("Now Watching {:?} for changes.", watch_dir);

    Ok((debouncer, rx))
}
