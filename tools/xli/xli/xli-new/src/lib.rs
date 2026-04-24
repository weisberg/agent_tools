#![forbid(unsafe_code)]

//! New workbook generation helpers.

mod create;

pub use create::{
    create_blank, create_from_csv, create_from_csv_with_options, create_from_json,
    create_from_markdown, ColumnFormat, CsvCreateOptions,
};
