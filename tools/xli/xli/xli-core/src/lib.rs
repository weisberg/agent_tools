#![forbid(unsafe_code)]

//! Shared types and helpers used across the XLI workspace.

pub mod addressing;
pub mod envelope;
pub mod error;
pub mod ops;
pub mod style;

pub use addressing::{
    col_to_letter, letter_to_col, parse_address, parse_range, AddressError, CellRef, RangeRef,
};
pub use envelope::{CommitMode, CommitStats, RepairSuggestion, ResponseEnvelope, Status};
pub use error::XliError;
pub use ops::{BatchOp, SheetAction};
pub use style::{resolve_number_format, FillSpec, FontSpec, NumberFormat, StyleSpec};
