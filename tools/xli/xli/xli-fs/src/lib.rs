#![forbid(unsafe_code)]

//! File locking, fingerprinting, staging, and atomic commit helpers.

mod commit;
mod fingerprint;
mod lock;
mod staging;

pub use commit::{
    atomic_commit, atomic_commit_with_options, validate_ooxml_file, AtomicCommitOptions,
    CommitResult,
};
pub use fingerprint::fingerprint;
pub use lock::WorkbookLock;
pub use staging::stage_temp_file;
