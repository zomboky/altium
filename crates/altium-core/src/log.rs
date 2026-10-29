//! The Altium log, where hooks and the background server report errors
//! instead of showing them: `$XDG_STATE_HOME/altium/altium.log` on Linux,
//! `%LOCALAPPDATA%\altium\altium.log` on Windows, or in
//! `$ALTIUM_CONFIG_DIR` when set.

use std::env;
use std::fs::{self, OpenOptions};
use std::io::Write as _;
use std::path::PathBuf;

use crate::registry::CONFIG_DIR_VAR;
use crate::time::Timestamp;

/// Size above which the log is moved to `altium.log.1`.
const MAX_BYTES: u64 = 1024 * 1024;

pub fn path() -> Option<PathBuf> {
    let dir = match env::var_os(CONFIG_DIR_VAR).filter(|d| !d.is_empty()) {
        Some(dir) => PathBuf::from(dir),
        None => dirs::state_dir().or_else(dirs::data_local_dir)?.join("altium"),
    };
    Some(dir.join("altium.log"))
}

/// Appends a line to the log. Never fails: logging must not break a hook.
pub fn write(source: &str, message: &str) {
    let Some(path) = path() else { return };
    if let Some(dir) = path.parent() {
        let _ = fs::create_dir_all(dir);
    }
    if fs::metadata(&path).is_ok_and(|m| m.len() > MAX_BYTES) {
        let _ = fs::rename(&path, path.with_extension("log.1"));
    }
    let line = format!("{} {source}: {}\n", Timestamp::now(), message.replace('\n', " | "));
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&path) {
        let _ = file.write_all(line.as_bytes());
    }
}
