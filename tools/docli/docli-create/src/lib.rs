//! Greenfield document creation for docli.

pub mod backend;
pub mod docx_rs_backend;
pub mod spec;

pub use backend::CreateBackend;
pub use docx_rs_backend::DocxRsBackend;
pub use spec::CreateSpec;
