#![forbid(unsafe_code)]

//! Formula recalculation helpers.

mod libreoffice;

pub use libreoffice::{recalc, RecalcResult};
