//! Read-only DOCX structural indexing and selector resolution.

pub mod hash;
pub mod heading;
pub mod index;
pub mod selector;
pub mod story;

pub use hash::hash_bytes;
pub use heading::resolve_heading_path;
pub use index::{
    CommentSummary, DocumentIndex, HeadingEntry, ImageEntry, ParagraphEntry, TableEntry,
    TrackedChangeSummary,
};
pub use selector::{resolve, ResolvedTarget};
pub use story::StoryPartMap;
