use std::{
    env::consts::OS, path::PathBuf, process::{Child, Command, exit}, sync::mpsc::Receiver, time::Duration,
};

use ignore::gitignore::{Gitignore, GitignoreBuilder};
use notify_debouncer_full::DebouncedEvent;

pub fn cmd_runner(rx: Receiver<Vec<DebouncedEvent>>, watch_dir: PathBuf, cmd: String) {
    let mut builder = GitignoreBuilder::new(&watch_dir);
    builder.add(watch_dir.join(".gitignore"));
    let gitignore = builder.build().unwrap();

    let mut child: Option<Child> = None;

    while let Ok(mut events) = rx.recv() {
        while let Ok(mut more_events) = rx.recv_timeout(Duration::from_millis(500)) {
            events.append(&mut more_events);
        }
        if should_ignore(&gitignore, &events, &watch_dir) {
            continue;
        }

        println!("Changed : {:?}", events[0].paths[0]);

        if let Some(mut running_child) = child.take() {
            match running_child.try_wait() {
                Ok(Some(_status)) => {
                    println!("Excited Command.")
                }
                Ok(None) => {
                    println!("Stopping current command...");
                    running_child.kill().expect("Failed to kill Process");
                    running_child.wait().expect("Failed to wait for Process");
                }
                Err(err) => {
                    println!("Error in the previous command: {}", err)
                }
            }
        }

        println!("Rerunning Command");
        if OS == "windows" {
            child = Some(
                Command::new("cmd")
                    .args(["/C", &cmd])
                    .current_dir(&watch_dir)
                    .spawn()
                    .expect("Failed to start command"),
            );
        } else if OS=="macos" || OS=="linux"{
            child = Some(
                Command::new("sh")
                    .args(["-c", &cmd])
                    .current_dir(&watch_dir)
                    .spawn()
                    .expect("Failed to start command"),
            )
        }else{
            println!("Unsupported OS.");
            exit(1);
        }
    }
}

fn should_ignore(gitignore: &Gitignore, events: &Vec<DebouncedEvent>, watch_dir: &PathBuf) -> bool {
    for event in events {
        for path in &event.paths {
            let relative_path = match path.strip_prefix(watch_dir) {
                Ok(path) => path,
                Err(_) => continue,
            };

            let matched = gitignore.matched_path_or_any_parents(relative_path, path.is_dir());

            if !matched.is_ignore() {
                return false;
            }
        }
    }

    return true;
}
