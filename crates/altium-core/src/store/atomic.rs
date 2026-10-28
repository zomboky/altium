//! Atomic file replacement: a reader sees either the old or the new content,
//! never a partial write, even if the writer is killed.

use std::fs::File;
use std::io::Write;
use std::path::Path;

use crate::error::{Result, StoreError};

/// Writes `contents` to `path` through a temporary file in `tmp_dir`, which
/// must be on the same filesystem, then renames it over `path`.
pub(crate) fn write(path: &Path, tmp_dir: &Path, contents: &[u8]) -> Result<()> {
    let mut builder = tempfile::Builder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        builder.permissions(std::fs::Permissions::from_mode(0o644));
    }
    let mut tmp = builder.tempfile_in(tmp_dir).map_err(StoreError::io(tmp_dir))?;
    tmp.write_all(contents).map_err(StoreError::io(tmp.path()))?;
    tmp.as_file().sync_all().map_err(StoreError::io(tmp.path()))?;
    tmp.persist(path).map_err(|e| StoreError::io(path)(e.error))?;

    // Make the rename itself durable.
    #[cfg(unix)]
    if let Some(parent) = path.parent() {
        File::open(parent)
            .and_then(|dir| dir.sync_all())
            .map_err(StoreError::io(parent))?;
    }
    Ok(())
}
