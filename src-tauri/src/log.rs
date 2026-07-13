//! Always-on file logger. Works in both debug (tauri dev) and release builds.
//! In debug, also echoes to stderr (visible in tauri-cli dev console).
//! In release, the eprintln! lines are swallowed by `windows_subsystem = "windows"`,
//! so the file is the only channel — that's exactly what we need for post-mortem
//! diagnosis without bothering the user with a console window.
//!
//! Log file location: `%USERPROFILE%\batteryshower.log` (e.g. C:\Users\Li\batteryshower.log).

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::sync::{Mutex, OnceLock};

static LOG: OnceLock<Mutex<Option<File>>> = OnceLock::new();

pub fn log_open(path: &Path) {
    let log = LOG.get_or_init(|| Mutex::new(None));
    match OpenOptions::new().create(true).append(true).open(path) {
        Ok(file) => {
            *log.lock().unwrap() = Some(file);
            eprintln!("[BatteryShower:log] writing log to {}", path.display());
        }
        Err(e) => {
            eprintln!(
                "[BatteryShower:log] failed to open log file {}: {}",
                path.display(),
                e
            );
        }
    }
}

pub fn log_write(msg: &str) {
    let ts = chrono::Local::now().format("%H:%M:%S%.3f");
    if let Some(log) = LOG.get() {
        if let Ok(mut guard) = log.lock() {
            if let Some(file) = guard.as_mut() {
                let _ = writeln!(file, "[{}] {}", ts, msg);
                let _ = file.flush();
            }
        }
    }
    eprintln!("[{}] {}", ts, msg);
}
