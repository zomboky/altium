//! Advisory lock serializing read-modify-write cycles across processes.

use std::fs::{OpenOptions, TryLockError};
use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};

use crate::error::StoreError;

const RETRY_EVERY: Duration = Duration::from_millis(5);

/// Runs `f` while holding an exclusive lock on `lock_path`, waiting at most
/// `timeout` for it. The lock is released when `f` returns, and by the OS if
/// the process dies.
///
/// Not reentrant: calling it again from `f` on the same path times out.
pub(crate) fn with_lock<T, E: From<StoreError>>(
    lock_path: &Path,
    timeout: Duration,
    f: impl FnOnce() -> Result<T, E>,
) -> Result<T, E> {
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(lock_path)
        .map_err(StoreError::io(lock_path))?;

    let deadline = Instant::now() + timeout;
    loop {
        match file.try_lock() {
            Ok(()) => break,
            Err(TryLockError::WouldBlock) if Instant::now() < deadline => thread::sleep(RETRY_EVERY),
            Err(TryLockError::WouldBlock) => return Err(StoreError::LockTimeout(lock_path.to_path_buf()).into()),
            Err(TryLockError::Error(e)) => return Err(StoreError::io(lock_path)(e).into()),
        }
    }

    let result = f();
    // Dropping the file releases the lock too; unlocking explicitly just
    // makes it happen before the result is handed back.
    let _ = file.unlock();
    result
}
