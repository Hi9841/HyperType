//! Deliberately tiny logger: appends errors and critical events to a single
//! file under %APPDATA%\HyperType. No background threads, no timers, no
//! per-keystroke logging. Debug output to stderr is gated behind the
//! HYPERTYPE_DEBUG environment variable.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

static LOG_PATH: OnceLock<PathBuf> = OnceLock::new();

pub fn init() {
    let path = crate::storage::data_dir().join("hypertype.log");
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = LOG_PATH.set(path);
    info("HyperType started");
}

fn debug_enabled() -> bool {
    std::env::var_os("HYPERTYPE_DEBUG").is_some()
}

fn write_line(level: &str, msg: &str) {
    if let Some(path) = LOG_PATH.get() {
        if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
            let ts = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            let _ = writeln!(file, "[{ts}] [{level}] {msg}");
        }
    }
}

pub fn info(msg: &str) {
    write_line("INFO", msg);
    if debug_enabled() {
        eprintln!("[INFO] {msg}");
    }
}

pub fn error(msg: &str) {
    write_line("ERROR", msg);
    eprintln!("[ERROR] {msg}");
}
