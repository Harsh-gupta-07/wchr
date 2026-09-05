use std::{
    env::consts::OS,
    path::{Path, PathBuf},
    process::{exit, Child, Command},
    sync::mpsc::Receiver,
    time::Duration,
};

use ignore::gitignore::{Gitignore, GitignoreBuilder};
use notify::{EventKind::Modify};
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
        
        println!("{:?}", events);
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
        } else if OS == "macos" || OS == "linux" {
            child = Some(
                Command::new("sh")
                    .args(["-c", &cmd])
                    .current_dir(&watch_dir)
                    .spawn()
                    .expect("Failed to start command"),
            )
        } else {
            println!("Unsupported OS.");
            exit(1);
        }
    }
}

fn should_ignore(gitignore: &Gitignore, events: &[DebouncedEvent], watch_dir: &Path) -> bool {
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

    true
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use notify::Event;
    use notify_debouncer_full::DebouncedEvent;

    use super::*;

    fn gitignore(watch_dir: &Path) -> Gitignore {
        let mut builder = GitignoreBuilder::new(watch_dir);
        builder.add_line(None, "target/").unwrap();
        builder.add_line(None, "*.log").unwrap();
        builder.build().unwrap()
    }

    fn event(path: &Path) -> DebouncedEvent {
        DebouncedEvent::new(
            Event::default().add_path(path.to_path_buf()),
            Instant::now(),
        )
    }

    #[test]
    fn ignores_empty_event_batches() {
        let watch_dir = PathBuf::from("/project");
        let matcher = gitignore(&watch_dir);

        assert!(should_ignore(&matcher, &[], &watch_dir));
    }

    #[test]
    fn ignores_matching_files_and_directories() {
        let watch_dir = PathBuf::from("/project");
        let matcher = gitignore(&watch_dir);
        let events = vec![event(&watch_dir.join("target/debug/app"))];

        assert!(should_ignore(&matcher, &events, &watch_dir));
    }

    #[test]
    fn does_not_ignore_files_that_do_not_match() {
        let watch_dir = PathBuf::from("/project");
        let matcher = gitignore(&watch_dir);
        let events = vec![event(&watch_dir.join("src/main.rs"))];

        assert!(!should_ignore(&matcher, &events, &watch_dir));
    }

    #[test]
    fn reruns_when_a_batch_contains_an_unignored_file() {
        let watch_dir = PathBuf::from("/project");
        let matcher = gitignore(&watch_dir);
        let events = vec![
            event(&watch_dir.join("debug.log")),
            event(&watch_dir.join("src/main.rs")),
        ];

        assert!(!should_ignore(&matcher, &events, &watch_dir));
    }

    #[test]
    fn ignores_paths_outside_the_watch_directory() {
        let watch_dir = PathBuf::from("/project");
        let matcher = gitignore(&watch_dir);
        let events = vec![event(Path::new("/other-project/file.txt"))];

        assert!(should_ignore(&matcher, &events, &watch_dir));
    }
}
