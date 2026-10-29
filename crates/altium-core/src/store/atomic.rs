//! Atomic file replacement: a reader sees either the old or the new content,
//! never a partial write, even if the writer is killed.

use std::fs;
use std::io::{self, Write};
use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};

use crate::error::{Result, StoreError};

/// `ERROR_ACCESS_DENIED` and `ERROR_SHARING_VIOLATION`: on Windows, another
/// program (a sync client, an antivirus) holds one of the files without
/// allowing it to be replaced, usually for a few milliseconds.
const IN_USE: [i32; 2] = [5, 32];
const IN_USE_TIMEOUT: Duration = Duration::from_secs(1);
const RETRY_EVERY: Duration = Duration::from_millis(10);

/// Writes `contents` to `path` through a temporary file in `tmp_dir`, which
/// must be on the same filesystem, then renames it over `path`.
pub(crate) fn write(path: &Path, tmp_dir: &Path, contents: &[u8]) -> Result<()> {
    #[allow(unused_mut)] // Only Unix sets permissions.
    let mut builder = tempfile::Builder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        builder.permissions(std::fs::Permissions::from_mode(0o644));
    }
    let mut tmp = builder.tempfile_in(tmp_dir).map_err(StoreError::io(tmp_dir))?;
    tmp.write_all(contents).map_err(StoreError::io(tmp.path()))?;
    tmp.as_file().sync_all().map_err(StoreError::io(tmp.path()))?;

    // Not `persist`: on Windows it calls `MoveFileExW`, which refuses to
    // replace a file that another process has open, even only to read it.
    // `fs::rename` uses POSIX semantics there. `keep` closes the temporary
    // file and clears its Windows "temporary" attribute.
    let tmp = tmp
        .into_temp_path()
        .keep()
        .map_err(|e| StoreError::io(&e.path)(e.error))?;
    if let Err(error) = rename(&tmp, path) {
        let _ = fs::remove_file(&tmp);
        return Err(StoreError::io(path)(error));
    }

    // Make the rename itself durable.
    #[cfg(unix)]
    if let Some(parent) = path.parent() {
        fs::File::open(parent)
            .and_then(|dir| dir.sync_all())
            .map_err(StoreError::io(parent))?;
    }
    Ok(())
}

/// `fs::rename`, retried for a moment while Windows reports a file in use.
fn rename(from: &Path, to: &Path) -> io::Result<()> {
    let deadline = Instant::now() + IN_USE_TIMEOUT;
    loop {
        match fs::rename(from, to) {
            Err(e) if cfg!(windows) && e.raw_os_error().is_some_and(|code| IN_USE.contains(&code)) => {
                if Instant::now() >= deadline {
                    return Err(e);
                }
                thread::sleep(RETRY_EVERY);
            }
            result => return result,
        }
    }
}
