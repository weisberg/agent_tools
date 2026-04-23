use std::{
    fs::{self, File, OpenOptions},
    path::Path,
};

use serde::{Deserialize, Serialize};

use crate::error::DocliError;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Durability {
    Fast,
    Durable,
    Paranoid,
}

impl Durability {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Fast => "fast",
            Self::Durable => "durable",
            Self::Paranoid => "paranoid",
        }
    }
}

impl Default for Durability {
    fn default() -> Self {
        Self::Durable
    }
}

pub fn commit_fast(temp_path: &Path, destination: &Path) -> Result<(), DocliError> {
    rename_into_place(temp_path, destination)
}

pub fn commit_durable(temp_path: &Path, destination: &Path) -> Result<(), DocliError> {
    sync_file(temp_path)?;
    rename_into_place(temp_path, destination)?;
    sync_parent_dir(destination)
}

pub fn commit_paranoid<F>(
    temp_path: &Path,
    destination: &Path,
    revalidate: F,
) -> Result<(), DocliError>
where
    F: FnOnce(&Path) -> Result<(), DocliError>,
{
    revalidate(temp_path)?;
    commit_durable(temp_path, destination)
}

fn rename_into_place(temp_path: &Path, destination: &Path) -> Result<(), DocliError> {
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).map_err(|source| DocliError::CommitFailed {
            message: source.to_string(),
        })?;
    }

    #[cfg(windows)]
    if destination.exists() {
        fs::remove_file(destination).map_err(|source| DocliError::CommitFailed {
            message: source.to_string(),
        })?;
    }

    fs::rename(temp_path, destination).map_err(|source| DocliError::CommitFailed {
        message: source.to_string(),
    })
}

fn sync_file(path: &Path) -> Result<(), DocliError> {
    File::open(path)
        .and_then(|file| file.sync_all())
        .map_err(|source| DocliError::CommitFailed {
            message: source.to_string(),
        })
}

fn sync_parent_dir(path: &Path) -> Result<(), DocliError> {
    let Some(parent) = path.parent() else {
        return Ok(());
    };

    #[cfg(unix)]
    {
        OpenOptions::new()
            .read(true)
            .open(parent)
            .and_then(|dir| dir.sync_all())
            .map_err(|source| DocliError::CommitFailed {
                message: source.to_string(),
            })?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{fs, path::Path};

    use tempfile::tempdir;

    use super::{commit_durable, commit_fast, commit_paranoid};

    fn write_temp_file(path: &Path, contents: &[u8]) {
        fs::write(path, contents).unwrap();
    }

    #[test]
    fn durable_commit_renames_file_into_place() {
        let dir = tempdir().unwrap();
        let temp = dir.path().join("shadow.docx");
        let output = dir.path().join("result.docx");

        write_temp_file(&temp, b"shadow-bytes");
        commit_durable(&temp, &output).unwrap();

        assert_eq!(fs::read(&output).unwrap(), b"shadow-bytes");
        assert!(!temp.exists());
    }

    #[test]
    fn fast_commit_renames_file_into_place() {
        let dir = tempdir().unwrap();
        let temp = dir.path().join("shadow.docx");
        let output = dir.path().join("fast.docx");

        write_temp_file(&temp, b"fast");
        commit_fast(&temp, &output).unwrap();

        assert_eq!(fs::read(&output).unwrap(), b"fast");
    }

    #[test]
    fn paranoid_commit_runs_validation_before_rename() {
        let dir = tempdir().unwrap();
        let temp = dir.path().join("shadow.docx");
        let output = dir.path().join("paranoid.docx");

        write_temp_file(&temp, b"paranoid");
        let mut validated = false;
        commit_paranoid(&temp, &output, |_| {
            validated = true;
            Ok(())
        })
        .unwrap();

        assert!(validated);
        assert_eq!(fs::read(&output).unwrap(), b"paranoid");
    }
}
