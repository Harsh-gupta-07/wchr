use std::{
    io,
    path::{Path, PathBuf},
    process::{self, Child, Command},
    sync::mpsc::Receiver,
    time::Duration,
};
#[cfg(unix)]
use std::{
    process::Stdio,
    sync::atomic::{AtomicI32, Ordering},
    thread,
    time::Instant,
};

use ignore::gitignore::{Gitignore, GitignoreBuilder};
use notify::EventKind::Modify;
use notify_debouncer_full::DebouncedEvent;

use crate::output;

/// How long a command gets to exit after SIGTERM before it is killed.
#[cfg(unix)]
const STOP_TIMEOUT: Duration = Duration::from_secs(5);

/// Process group of the running command, so it can be stopped when wchr exits.
#[cfg(unix)]
static CHILD_PGID: AtomicI32 = AtomicI32::new(0);

#[cfg(unix)]
fn exit_signals() -> libc::sigset_t {
    unsafe {
        let mut set: libc::sigset_t = std::mem::zeroed();
        libc::sigemptyset(&mut set);
        libc::sigaddset(&mut set, libc::SIGINT);
        libc::sigaddset(&mut set, libc::SIGTERM);
        libc::sigaddset(&mut set, libc::SIGHUP);
        set
    }
}

/// Stops the running command before exiting on Ctrl-C, SIGTERM or SIGHUP.
///
/// The command runs in its own process group, so Ctrl-C in the terminal only
/// reaches wchr and would otherwise leave the command running. Must be called
/// before any other thread is spawned, so every thread inherits the blocked signals.
#[cfg(unix)]
pub fn handle_exit_signals() {
    let set = exit_signals();
    unsafe { libc::pthread_sigmask(libc::SIG_BLOCK, &set, std::ptr::null_mut()) };

    thread::spawn(move || {
        let mut signal = 0;
        unsafe { libc::sigwait(&set, &mut signal) };

        let pgid = CHILD_PGID.load(Ordering::SeqCst);
        if pgid > 0 {
            println!("{}", output::warning("Stopping the current command..."));
            if let Err(err) = stop_group(pgid) {
                eprintln!(
                    "{}",
                    output::error(format!("Unable to kill the command: {err}"))
                );
            }
        }

        process::exit(128 + signal);
    });
}

#[cfg(windows)]
pub fn handle_exit_signals() {}

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

                    // The shell has exited, but processes it started may still be running.
                    #[cfg(unix)]
                    if let Err(err) = stop_process(&mut running_child) {
                        eprintln!(
                            "{}",
                            output::error(format!("Unable to kill previous command: {err}"))
                        );
                    }
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
fn start_process(watch_dir: &Path, cmd: &str) -> io::Result<Child> {
    use std::os::unix::process::CommandExt;

    let mut command = Command::new("sh");

    command
        .args(["-c", cmd])
        .current_dir(watch_dir)
        // The command runs in a background process group, where reading from the
        // terminal (e.g. a dev server's keyboard shortcuts) gets it stopped by
        // SIGTTIN/SIGTTOU. A stopped process ignores SIGTERM and keeps its ports.
        .stdin(Stdio::null());

    let signals = exit_signals();

    unsafe {
        command.pre_exec(move || {
            // Put the shell and its children in a new process group.
            if libc::setpgid(0, 0) == -1 {
                return Err(io::Error::last_os_error());
            }

            // Undo the signal mask set up by handle_exit_signals.
            libc::sigprocmask(libc::SIG_UNBLOCK, &signals, std::ptr::null_mut());

            Ok(())
        });
    }

    let child = command.spawn()?;
    CHILD_PGID.store(child.id() as i32, Ordering::SeqCst);

    Ok(child)
}

#[cfg(windows)]
fn start_process(watch_dir: &Path, cmd: &str) -> std::io::Result<Child> {
    Command::new("cmd")
        .args(["/C", cmd])
        .current_dir(watch_dir)
        .spawn()
}

#[cfg(unix)]
fn stop_process(child: &mut Child) -> io::Result<()> {
    CHILD_PGID.store(0, Ordering::SeqCst);

    stop_group(child.id() as i32)
}

/// Terminates every process in the group and waits for all of them to exit,
/// not just the shell, so ports are free before the command is restarted.
#[cfg(unix)]
fn stop_group(pgid: i32) -> io::Result<()> {
    signal_group(pgid, libc::SIGTERM)?;
    // A stopped process only acts on SIGTERM once it is continued.
    signal_group(pgid, libc::SIGCONT)?;

    if wait_for_group(pgid, STOP_TIMEOUT) {
        return Ok(());
    }

    println!(
        "{}",
        output::warning("Command did not exit in time; killing it.")
    );
    signal_group(pgid, libc::SIGKILL)?;
    wait_for_group(pgid, STOP_TIMEOUT);

    Ok(())
}

#[cfg(unix)]
fn signal_group(pgid: i32, signal: i32) -> io::Result<()> {
    // Negative PID means the entire process group.
    if unsafe { libc::kill(-pgid, signal) } == -1 {
        let err = io::Error::last_os_error();

        // ESRCH means the whole group has already exited.
        if err.raw_os_error() != Some(libc::ESRCH) {
            return Err(err);
        }
    }

    Ok(())
}

/// Returns true once no process in the group is left, or false on timeout.
#[cfg(unix)]
fn wait_for_group(pgid: i32, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;

    loop {
        // Reap the shell, otherwise its zombie keeps the group alive.
        unsafe { libc::waitpid(pgid, std::ptr::null_mut(), libc::WNOHANG) };

        if unsafe { libc::kill(-pgid, 0) } == -1
            && io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH)
        {
            return true;
        }

        if Instant::now() >= deadline {
            return false;
        }

        thread::sleep(Duration::from_millis(50));
    }
}

#[cfg(windows)]
fn stop_process(child: &mut Child) -> io::Result<()> {
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
