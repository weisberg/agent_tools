#![forbid(unsafe_code)]

mod blocks;
mod cli;
mod context;
mod diff;
mod document;
mod error;
mod frontmatter;
mod ids;
mod index;
mod lint;
mod marker;
mod model;
mod output;
mod plan;
mod recipe;
mod sections;
mod selector;
mod tables;
mod template;
mod tree;
mod util;
mod validate;

pub use cli::main_entry;

pub(crate) const OUTPUT_SCHEMA: &str = "mdli/output/v1";
pub(crate) const MARKER_VERSION: &str = "1";

pub(crate) use blocks::*;
pub(crate) use cli::*;
pub(crate) use context::*;
pub(crate) use diff::*;
pub(crate) use document::*;
pub(crate) use error::*;
pub(crate) use frontmatter::*;
pub(crate) use ids::*;
pub(crate) use index::*;
pub(crate) use lint::*;
pub(crate) use marker::*;
pub(crate) use model::*;
pub(crate) use output::*;
pub(crate) use plan::*;
pub(crate) use recipe::*;
pub(crate) use sections::*;
pub(crate) use selector::*;
pub(crate) use tables::*;
pub(crate) use template::*;
pub(crate) use tree::*;
pub(crate) use util::*;
pub(crate) use validate::*;
