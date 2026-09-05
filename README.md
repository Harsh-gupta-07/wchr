# wchr

**wchr** (watcher) is a lightweight CLI tool that watches your project directory for file changes and automatically re-runs a command — like a tiny, zero-config `nodemon` written in Rust.

Whenever a file changes, `wchr` kills the currently running process (if any) and reruns your command. It respects your `.gitignore`, so build artifacts and `node_modules` are never watched.

---

## Features

- **Auto-reruns** your command on file changes
- **Respects `.gitignore`** — ignores files you don't care about
- **Debounced events** — prevents rapid-fire restarts from burst saves
- **Cross-platform** — works on macOS, Linux, and Windows
- **Written in Rust** — fast and lightweight

---

## Installation

### Homebrew (macOS / Linux)

```sh
brew install Harsh-Gupta-07/tap/wchr
```

### Build from Source

Requires [Rust](https://rustup.rs/) installed.

```sh
git clone https://github.com/Harsh-Gupta-07/wchr.git
cd wchr
cargo install --path .
```

---

## Usage

```sh
wchr "<command>"
```

Run `wchr` in the root of your project with the command you want to watch-and-rerun:

```sh
# Re-run a Node.js server on every save
wchr "node index.js"

# Re-run a Python script
wchr "python main.py"

# Re-run a Rust binary after building
wchr "cargo run"

# Re-run any shell command
wchr "ls /src"
wchr "curl google.com"
```

`wchr` will:

1. Start watching the current directory recursively
2. On any file change (that isn't gitignored), kill the running process and rerun the command

---

## How It Works

- Uses [`notify`](https://crates.io/crates/notify) for cross-platform filesystem events
- Debounces events with a 1-second window to avoid redundant restarts
- Reads `.gitignore` via the [`ignore`](https://crates.io/crates/ignore) crate to filter out irrelevant paths

---

## License

MIT © [Harsh Gupta](https://github.com/Harsh-Gupta-07)
