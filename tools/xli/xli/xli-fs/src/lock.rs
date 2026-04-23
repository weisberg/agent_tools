use fs4::fs_std::FileExt;
use std::fs::{File, OpenOptions};
use std::path::Path;

use xli_core::XliError;

/// Exclusive lock held on a workbook file until dropped.
#[derive(Debug)]
pub struct WorkbookLock {
    file: File,
}

impl WorkbookLock {
    /// Acquire an exclusive lock on an existing workbook file.
    pub fn acquire(path: &Path) -> Result<Self, XliError> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)
            .map_err(|error| match error.kind() {
                std::io::ErrorKind::NotFound => XliError::FileNotFound {
                    path: path.display().to_string(),
                },
                _ => XliError::LockConflict {
                    path: path.display().to_string(),
                },
            })?;

        file.lock_exclusive().map_err(|_| XliError::LockConflict {
            path: path.display().to_string(),
        })?;

        Ok(Self { file })
    }

    /// Access the underlying locked file handle.
    pub fn file(&self) -> &File {
        &self.file
    }
}

impl Drop for WorkbookLock {
    fn drop(&mut self) {
        // Explicitly unlock rather than relying on the OS to release the lock
        // when the File handle closes. On Unix (flock) the implicit release is
        // reliable, but on Windows (LockFile/UnlockFile via fs4) explicit
        // unlocking is the safe cross-platform contract. (Issue #28)
        let _ = self.file.unlock();
    }
}
