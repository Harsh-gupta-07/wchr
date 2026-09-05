use std::{
    fmt::Display,
    io::{self, IsTerminal},
};

const BLUE: &str = "\x1b[34m";
const CYAN: &str = "\x1b[36m";
const GREEN: &str = "\x1b[32m";
const MAGENTA: &str = "\x1b[35m";
const RED: &str = "\x1b[31m";
const YELLOW: &str = "\x1b[33m";
const BOLD: &str = "\x1b[1m";
const RESET: &str = "\x1b[0m";

fn message(label: &str, text: impl Display, color: &str, stderr: bool) -> String {
    let message = format!("{label} {text}");
    let force_color = std::env::var_os("FORCE_COLOR").is_some();
    let no_color = std::env::var_os("NO_COLOR").is_some();
    let is_terminal = if stderr {
        io::stderr().is_terminal()
    } else {
        io::stdout().is_terminal()
    };

    if !no_color && (is_terminal || force_color) {
        format!("{color}{BOLD}{message}{RESET}")
    } else {
        message
    }
}

pub fn info(text: impl Display) -> String {
    message("ℹ", text, BLUE, false)
}

pub fn success(text: impl Display) -> String {
    message("✓", text, GREEN, false)
}

pub fn change(text: impl Display) -> String {
    message("↻", text, MAGENTA, false)
}

pub fn command(text: impl Display) -> String {
    message("▶", text, CYAN, false)
}

pub fn warning(text: impl Display) -> String {
    message("!", text, YELLOW, false)
}

pub fn error(text: impl Display) -> String {
    message("✗", text, RED, true)
}
