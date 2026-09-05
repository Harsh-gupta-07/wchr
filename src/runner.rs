use std::{
    path::{Path, PathBuf},
    process::{self, Child, Command},
    sync::mpsc::Receiver,
    time::Duration,
};

use ignore::gitignore::{Gitignore, GitignoreBuilder};
use notify::EventKind::Modify;
use notify_debouncer_full::DebouncedEvent;

use crate::output;

pub fn cmd_runner(rx: Receiver<Vec<DebouncedEvent>>, watch_dir: PathBuf, cmd: String) {
    let mut builder = GitignoreBuilder::new(&watch_dir);
    let gitignore_path = watch_dir.join(".gitignore");
    if gitignore_path.exists() {
        builder.add(watch_dir.join(".gitignore"));
    }

    let gitignore = builder.build().expect("Failed to build .gitignore rules");

    println!("{}", output::command(format!("Running command: '{cmd}'")));
    let mut child: Option<Child> = match start_process(&watch_dir, &cmd) {
        Ok(child) => Some(child),

        Err(err) => {
            eprintln!(
                "{}",
                output::error(format!("Failed to start command: {err}"))
            );

            None
        }
    };

    while let Ok(mut events) = rx.recv() {
        while let Ok(mut more_events) = rx.recv_timeout(Duration::from_millis(500)) {
            events.append(&mut more_events);
        }
        if should_ignore(&gitignore, &events, &watch_dir) {
            continue;
        }

        if events.len() > 0 {
            println!(
                "{}",
                output::change(format!("Saved: {:?}", events[0].paths[0]))
            );
        }

        if let Some(mut running_child) = child.take() {
            match running_child.try_wait() {
                Ok(Some(_status)) => {
                    println!("{}", output::info("Previous command terminated."));
                }
                Ok(None) => {
                    println!("{}", output::warning("Stopping the current command..."));
                    if let Err(err) = stop_process(&mut running_child) {
                        eprintln!(
                            "{}",
                            output::error(format!("Unable to kill previous command: {err}"))
                        );
                    }
                }
                Err(err) => {
                    eprintln!(
                        "{}",
                        output::error(format!("Could not check the previous command: {err}"))
                    );
                    process::exit(1)
                }
            }
        }

        println!("{}", output::command(format!("Running command: '{cmd}'")));
        child = match start_process(&watch_dir, &cmd) {
            Ok(child) => Some(child),

            Err(err) => {
                eprintln!(
                    "{}",
                    output::error(format!("Failed to start command: {err}"))
                );

                None
            }
        };
    }
}

fn should_ignore(gitignore: &Gitignore, events: &[DebouncedEvent], watch_dir: &Path) -> bool {
    for event in events {
        if !matches!(event.kind, Modify(_)) {
            continue;
        }

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

#[cfg(unix)]
fn start_process(watch_dir: &Path, cmd: &str) -> std::io::Result<Child> {
    use std::os::unix::process::CommandExt;

    let mut command = Command::new("sh");

    command.args(["-c", cmd]).current_dir(watch_dir);

    unsafe {
        command.pre_exec(|| {
            // Put the shell and its children in a new process group.
            if libc::setpgid(0, 0) == -1 {
                return Err(std::io::Error::last_os_error());
            }

            Ok(())
        });
    }

    command.spawn()
}

#[cfg(windows)]
fn start_process(watch_dir: &Path, cmd: &str) -> std::io::Result<Child> {
    Command::new("cmd")
        .args(["/C", cmd])
        .current_dir(watch_dir)
        .spawn()
}

#[cfg(unix)]
fn stop_process(child: &mut Child) -> std::io::Result<()> {
    let pid = child.id() as i32;

    // Negative PID means the entire process group.
    let result = unsafe { libc::kill(-pid, libc::SIGTERM) };

    if result == -1 {
        return Err(std::io::Error::last_os_error());
    }

    child.wait()?;

    Ok(())
}

#[cfg(windows)]
fn stop_process(child: &mut Child) -> std::io::Result<()> {
    Command::new("taskkill")
        .args([
            "/PID",
            &child.id().to_string(),
            "/T", // Kill child processes too.
            "/F", // Force termination.
        ])
        .status()?;

    child.wait()?;

    Ok(())
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
