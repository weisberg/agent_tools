//! Rendering, conversion, and visual diff adapters for docli.

pub mod diff;
pub mod markdown;
pub mod pandoc;
pub mod poppler;
pub mod soffice;

pub use diff::{semantic_diff, DiffChange, DiffResult, DiffSummary};
pub use markdown::{index_to_markdown, index_to_text};
