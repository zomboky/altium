//! Errors raised while reading or writing a `.altium/` directory.

use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

pub type Result<T, E = StoreError> = std::result::Result<T, E>;

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("{}: {source}", path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    /// A file exists but cannot be parsed. `line` is 1-based when known.
    #[error("{}{}: {message}", file.display(), line.map(|l| format!(":{l}")).unwrap_or_default())]
    Invalid {
        file: PathBuf,
        line: Option<usize>,
        message: String,
    },

    #[error("{} is not an Altium project (no .altium/ directory)", .0.display())]
    NotAProject(PathBuf),

    #[error("{} already exists", .0.display())]
    AlreadyExists(PathBuf),

    #[error(".altium/ uses schema v{found} but this binary supports up to v{supported}: update Altium")]
    ReadOnly { found: u32, supported: u32 },

    #[error("no migration from schema v{0}")]
    NoMigration(u32),

    #[error("timed out waiting for the lock on {}", .0.display())]
    LockTimeout(PathBuf),

    #[error("invalid memory node id `{0}` (expected lowercase letters, digits and dashes)")]
    InvalidNodeId(String),

    #[error("cannot find the user config directory (is HOME set?)")]
    NoConfigDir,
}

impl StoreError {
    pub fn invalid(file: &Path, line: Option<usize>, message: impl fmt::Display) -> Self {
        Self::Invalid {
            file: file.to_path_buf(),
            line,
            message: message.to_string(),
        }
    }

    /// Wraps an I/O error with the path it happened on, for `map_err`.
    pub(crate) fn io(path: &Path) -> impl FnOnce(io::Error) -> Self {
        let path = path.to_path_buf();
        move |source| Self::Io { path, source }
    }
}
