use docli_core::DocliError;

/// Trait for document creation backends.
pub trait CreateBackend {
    /// Create a DOCX file from a spec, returning raw bytes.
    fn create(&self, spec: &super::spec::CreateSpec) -> Result<Vec<u8>, DocliError>;
}
