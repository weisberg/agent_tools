use serde::Serialize;
use std::fs::File;
use std::path::Path;
use std::time::Instant;
use tempfile::TempPath;
use zip::ZipArchive;

use xli_core::{CommitStats, XliError};

use crate::fingerprint::fingerprint;
use crate::lock::WorkbookLock;
use crate::staging::stage_temp_file;

/// Options that customize the atomic commit behavior.
#[derive(Clone, Debug, Default)]
pub struct AtomicCommitOptions {
    pub dry_run: bool,
}

/// Metadata returned by a successful atomic commit.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct CommitResult {
    pub fingerprint_before: String,
    pub fingerprint_after: String,
    pub stats: CommitStats,
}

/// Perform a real atomic commit.
pub fn atomic_commit<P, F, T>(
    path: P,
    expect_fingerprint: Option<&str>,
    mutate: F,
) -> Result<(CommitResult, T), XliError>
where
    P: AsRef<Path>,
    F: FnOnce(&Path, &Path) -> Result<T, XliError>,
    T: Serialize,
{
    atomic_commit_with_options(
        path,
        expect_fingerprint,
        AtomicCommitOptions::default(),
        mutate,
    )
}

/// Perform an atomic commit with additional options such as dry-run support.
pub fn atomic_commit_with_options<P, F, T>(
    path: P,
    expect_fingerprint: Option<&str>,
    options: AtomicCommitOptions,
    mutate: F,
) -> Result<(CommitResult, T), XliError>
where
    P: AsRef<Path>,
    F: FnOnce(&Path, &Path) -> Result<T, XliError>,
    T: Serialize,
{
    let path = path.as_ref();
    if !path.exists() {
        return Err(XliError::FileNotFound {
            path: path.display().to_string(),
        });
    }

    let started = Instant::now();
    let _lock = WorkbookLock::acquire(path)?;
    let fingerprint_before = fingerprint(path)?;

    if let Some(expected) = expect_fingerprint {
        if expected != fingerprint_before {
            return Err(XliError::FingerprintMismatch {
                expected: expected.to_string(),
                actual: fingerprint_before,
            });
        }
    }

    let parent_dir = path.parent().unwrap_or_else(|| Path::new("."));
    let staged = stage_temp_file(parent_dir)?;
    let tmp_path = staged.path().to_path_buf();
    let output = mutate(path, &tmp_path)?;
    validate_ooxml_file(&tmp_path)?;
    // Open a fresh fd on the path the mutate closure actually wrote to and
    // fsync it. The NamedTempFile's internal File handle was never written to
    // by the closure (which opens tmp_path independently via its own fd), so
    // syncing staged.as_file() would be a no-op that provides a false
    // durability guarantee. (Issue #19)
    File::open(&tmp_path)
        .and_then(|f| f.sync_all())
        .map_err(io_error)?;

    let fingerprint_after = fingerprint(&tmp_path)?;
    let file_size_before = path.metadata().map_err(io_error)?.len();
    let file_size_after = tmp_path.metadata().map_err(io_error)?.len();
    let result = CommitResult {
        fingerprint_before,
        fingerprint_after,
        stats: CommitStats {
            elapsed_ms: started.elapsed().as_millis() as u64,
            file_size_before,
            file_size_after,
        },
    };

    if options.dry_run {
        return Ok((result, output));
    }

    let staged_path = staged.into_temp_path();
    atomic_replace(&staged_path, path)?;

    Ok((result, output))
}

/// Validate a staged workbook file when it looks like an OOXML workbook.
pub fn validate_ooxml_file(path: &Path) -> Result<(), XliError> {
    if !is_ooxml_workbook(path) {
        return Ok(());
    }

    let file = File::open(path).map_err(io_error)?;
    let mut archive = ZipArchive::new(file).map_err(zip_error)?;
    archive.by_name("[Content_Types].xml").map_err(zip_error)?;
    archive.by_name("xl/workbook.xml").map_err(zip_error)?;
    Ok(())
}

fn atomic_replace(src: &TempPath, dst: &Path) -> Result<(), XliError> {
    std::fs::rename(src, dst).map_err(io_error)
}

fn is_ooxml_workbook(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|ext| ext.to_str()),
        Some("xlsx" | "xlsm" | "xltx" | "xltm")
    )
}

fn io_error(error: std::io::Error) -> XliError {
    XliError::OoxmlCorrupt {
        details: error.to_string(),
    }
}

fn zip_error(error: zip::result::ZipError) -> XliError {
    XliError::OoxmlCorrupt {
        details: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        atomic_commit, atomic_commit_with_options, fingerprint, validate_ooxml_file,
        AtomicCommitOptions,
    };
    use rust_xlsxwriter::Workbook;
    use std::fs;
    use std::io::Write;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Barrier};
    use std::thread;
    use std::time::Duration;
    use tempfile::{tempdir, NamedTempFile};
    use xli_core::XliError;

    #[test]
    fn atomic_commit_updates_file_and_fingerprint() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("book.txt");
        fs::write(&path, "before").expect("write");
        let before = fingerprint(&path).expect("before");

        let (result, value) = atomic_commit(&path, None, |src, dst| {
            let contents = fs::read_to_string(src).expect("read");
            fs::write(dst, format!("{contents}-after")).expect("write");
            Ok::<_, XliError>("done")
        })
        .expect("commit");

        assert_eq!(value, "done");
        assert_eq!(result.fingerprint_before, before);
        assert_ne!(result.fingerprint_before, result.fingerprint_after);
        assert_eq!(fs::read_to_string(&path).expect("read"), "before-after");
    }

    #[test]
    fn fingerprint_mismatch_skips_mutate_and_preserves_file() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("book.txt");
        fs::write(&path, "before").expect("write");
        let called = AtomicBool::new(false);

        let error = atomic_commit(&path, Some("sha256:nope"), |_, _| {
            called.store(true, Ordering::SeqCst);
            Ok::<_, XliError>(())
        })
        .expect_err("should fail");

        assert!(matches!(error, XliError::FingerprintMismatch { .. }));
        assert!(!called.load(Ordering::SeqCst));
        assert_eq!(fs::read_to_string(&path).expect("read"), "before");
    }

    #[test]
    fn mutate_error_cleans_temp_and_preserves_original() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("book.txt");
        fs::write(&path, "before").expect("write");
        let before_entries = dir.path().read_dir().expect("read_dir").count();

        let error = atomic_commit(&path, None, |_, dst| {
            fs::write(dst, "temp").expect("write");
            Err::<(), _>(XliError::WriteConflict {
                target: "book.txt".to_string(),
                details: Some("boom".to_string()),
            })
        })
        .expect_err("should fail");

        assert!(matches!(error, XliError::WriteConflict { .. }));
        assert_eq!(fs::read_to_string(&path).expect("read"), "before");
        let after_entries = dir.path().read_dir().expect("read_dir").count();
        assert_eq!(before_entries, after_entries);
    }

    #[test]
    fn dry_run_returns_hypothetical_fingerprint_without_modifying_file() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("book.txt");
        fs::write(&path, "before").expect("write");
        let before = fingerprint(&path).expect("fingerprint");

        let (result, _) = atomic_commit_with_options(
            &path,
            None,
            AtomicCommitOptions { dry_run: true },
            |_, dst| {
                fs::write(dst, "after").expect("write");
                Ok::<_, XliError>(())
            },
        )
        .expect("dry run");

        assert_eq!(result.fingerprint_before, before);
        assert_ne!(result.fingerprint_before, result.fingerprint_after);
        assert_eq!(fs::read_to_string(&path).expect("read"), "before");
    }

    #[test]
    fn missing_file_returns_file_not_found() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("missing.txt");
        let error =
            atomic_commit(&path, None, |_, _| Ok::<_, XliError>(())).expect_err("missing file");
        assert!(matches!(error, XliError::FileNotFound { .. }));
    }

    #[test]
    fn stage_temp_file_uses_source_directory() {
        let dir = tempdir().expect("tempdir");
        let staged = crate::stage_temp_file(dir.path()).expect("stage");
        assert_eq!(staged.path().parent(), Some(dir.path()));
    }

    #[test]
    fn concurrent_locks_serialize_commits() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("book.txt");
        fs::write(&path, "start").expect("write");
        let barrier = Arc::new(Barrier::new(2));

        let first_path = path.clone();
        let first_barrier = Arc::clone(&barrier);
        let first = thread::spawn(move || {
            atomic_commit(&first_path, None, |src, dst| {
                let contents = fs::read_to_string(src).expect("read");
                fs::write(dst, format!("{contents}-one")).expect("write");
                first_barrier.wait();
                thread::sleep(Duration::from_millis(100));
                Ok::<_, XliError>(())
            })
            .expect("first commit");
        });

        barrier.wait();
        let second_path = path.clone();
        let second = thread::spawn(move || {
            atomic_commit(&second_path, None, |src, dst| {
                let contents = fs::read_to_string(src).expect("read");
                fs::write(dst, format!("{contents}-two")).expect("write");
                Ok::<_, XliError>(())
            })
            .expect("second commit");
        });

        first.join().expect("join first");
        second.join().expect("join second");
        assert_eq!(fs::read_to_string(&path).expect("read"), "start-one-two");
    }

    #[test]
    fn validates_generated_xlsx() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("valid.xlsx");
        let mut workbook = Workbook::new();
        workbook
            .add_worksheet()
            .write_string(0, 0, "hello")
            .expect("write");
        workbook.save(&path).expect("save");

        validate_ooxml_file(&path).expect("valid workbook");
    }

    #[test]
    fn rejects_corrupt_xlsx() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("bad.xlsx");
        fs::write(&path, "not a zip").expect("write");
        let error = validate_ooxml_file(&path).expect_err("should fail");
        assert!(matches!(error, XliError::OoxmlCorrupt { .. }));
    }

    macro_rules! fingerprint_stability_test {
        ($name:ident, $contents:expr) => {
            #[test]
            fn $name() {
                let dir = tempdir().expect("tempdir");
                let path = dir.path().join("book.txt");
                fs::write(&path, $contents).expect("write");
                let first = fingerprint(&path).expect("first");
                let second = fingerprint(&path).expect("second");
                assert_eq!(first, second);
            }
        };
    }

    macro_rules! dry_run_preserves_contents_test {
        ($name:ident, $contents:expr) => {
            #[test]
            fn $name() {
                let dir = tempdir().expect("tempdir");
                let path = dir.path().join("book.txt");
                fs::write(&path, $contents).expect("write");
                atomic_commit_with_options(
                    &path,
                    None,
                    AtomicCommitOptions { dry_run: true },
                    |_, dst| {
                        fs::write(dst, format!("{}-changed", $contents)).expect("write");
                        Ok::<_, XliError>(())
                    },
                )
                .expect("dry run");
                assert_eq!(fs::read_to_string(&path).expect("read"), $contents);
            }
        };
    }

    macro_rules! changed_commit_updates_contents_test {
        ($name:ident, $contents:expr) => {
            #[test]
            fn $name() {
                let dir = tempdir().expect("tempdir");
                let path = dir.path().join("book.txt");
                fs::write(&path, $contents).expect("write");
                let before = fingerprint(&path).expect("before");
                let (result, _) = atomic_commit(&path, None, |_, dst| {
                    fs::write(dst, format!("{}-changed", $contents)).expect("write");
                    Ok::<_, XliError>(())
                })
                .expect("commit");
                assert_eq!(result.fingerprint_before, before);
                assert_ne!(result.fingerprint_before, result.fingerprint_after);
            }
        };
    }

    fingerprint_stability_test!(fingerprint_stable_ascii, "alpha");
    fingerprint_stability_test!(fingerprint_stable_multiline, "alpha\nbeta");
    fingerprint_stability_test!(fingerprint_stable_empty, "");
    fingerprint_stability_test!(fingerprint_stable_numeric, "1234567890");
    fingerprint_stability_test!(fingerprint_stable_longer, "the quick brown fox jumps");

    dry_run_preserves_contents_test!(dry_run_preserves_ascii, "alpha");
    dry_run_preserves_contents_test!(dry_run_preserves_multiline, "alpha\nbeta");
    dry_run_preserves_contents_test!(dry_run_preserves_empty, "");
    dry_run_preserves_contents_test!(dry_run_preserves_numeric, "1234567890");
    dry_run_preserves_contents_test!(dry_run_preserves_longer, "the quick brown fox jumps");

    changed_commit_updates_contents_test!(commit_changes_ascii, "alpha");
    changed_commit_updates_contents_test!(commit_changes_multiline, "alpha\nbeta");
    changed_commit_updates_contents_test!(commit_changes_empty, "");
    changed_commit_updates_contents_test!(commit_changes_numeric, "1234567890");
    changed_commit_updates_contents_test!(commit_changes_longer, "the quick brown fox jumps");

    #[test]
    fn staged_temp_can_be_written_in_same_directory() {
        let dir = tempdir().expect("tempdir");
        let mut staged = crate::stage_temp_file(dir.path()).expect("stage");
        staged.write_all(b"hello").expect("write");
        staged.as_file().sync_all().expect("sync");
        assert_eq!(staged.path().parent(), Some(dir.path()));
    }

    #[test]
    fn dry_run_temp_file_is_cleaned_up() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("book.txt");
        fs::write(&path, "before").expect("write");
        let before_entries = dir.path().read_dir().expect("read_dir").count();

        atomic_commit_with_options(
            &path,
            None,
            AtomicCommitOptions { dry_run: true },
            |_, dst| {
                fs::write(dst, "after").expect("write");
                Ok::<_, XliError>(())
            },
        )
        .expect("dry run");

        let after_entries = dir.path().read_dir().expect("read_dir").count();
        assert_eq!(before_entries, after_entries);
    }

    #[test]
    fn fingerprint_handles_named_temp_files() {
        let mut file = NamedTempFile::new().expect("temp file");
        file.write_all(b"hello").expect("write");
        file.as_file().sync_all().expect("sync");
        let fp = fingerprint(file.path()).expect("fingerprint");
        assert!(fp.starts_with("sha256:"));
    }
}
