#![forbid(unsafe_code)]

//! Read-only workbook inspection helpers.

pub mod inspect;
pub mod read;

pub use inspect::{inspect, SheetInfo, WorkbookInfo};
pub use read::{
    read_all_sheets, read_cell, read_range, read_table, CellData, CellValueType, RangeData,
};
