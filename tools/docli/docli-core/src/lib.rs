//! Core types and package transaction primitives for docli.

pub mod commit;
pub mod envelope;
pub mod error;
pub mod job;
pub mod journal;
pub mod package;
pub mod pipeline;
pub mod units;

pub use commit::{commit_durable, commit_fast, commit_paranoid, Durability};
pub use envelope::{Envelope, EnvelopeBuilder, ErrEnvelope, ErrorDetail, OkEnvelope};
pub use error::{DocliError, ErrorCode};
pub use job::{
    CellRef, ColumnsBlock, ContentBlock, FontSpec, ImageBlock, InlineRun, Job, LinkBlock,
    Operation, ParagraphBlock, ParagraphContent, Position, Scope, Story, StyleOverride, TableBlock,
    Target, TocBlock,
};
pub use journal::CommitJournal;
pub use package::{Package, PartEntry, PartInventory};
pub use pipeline::{
    run_noop_pipeline, run_shadow_pipeline, PipelineContext, PipelineHooks, PipelineRequest,
    PipelineResult, SelectorIndex,
};
