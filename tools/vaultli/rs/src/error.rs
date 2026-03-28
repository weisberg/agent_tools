use thiserror::Error;

#[derive(Debug, Error)]
pub enum VaultliError {
    #[error("No .kbroot found from {0}")]
    RootNotFound(String),
    #[error("Vault root already exists at {0}")]
    RootExists(String),
    #[error("Path is outside vault root: {0}")]
    PathOutsideRoot(String),
    #[error("File not found: {0}")]
    FileNotFound(String),
    #[error("Expected a markdown file, got {0}")]
    NotMarkdown(String),
    #[error("Malformed frontmatter in {0}")]
    MalformedFrontmatter(String),
    #[error("Invalid frontmatter in {0}: {1}")]
    InvalidFrontmatter(String, String),
    #[error("Missing index file: {0}")]
    IndexMissing(String),
    #[error("Index contains invalid JSON")]
    InvalidIndex,
    #[error("Missing required fields in {0}")]
    MissingRequiredFields(String),
    #[error("Broken source for {0}: {1}")]
    BrokenSource(String, String),
    #[error("No indexed document found for id {0}")]
    IdNotFound(String),
    #[error("Markdown file already contains frontmatter: {0}")]
    FrontmatterExists(String),
    #[error("Sidecar already exists: {0}")]
    SidecarExists(String),
    #[error("Expected a file, got directory: {0}")]
    NotAFile(String),
    #[error("The `jq` executable is required for --jq filtering")]
    JqUnavailable,
    #[error("jq filter failed: {0}")]
    JqFilterFailed(String),
    #[error("jq filter must emit JSON objects")]
    JqFilterInvalid,
    #[error("Unsupported command state: {0}")]
    Unsupported(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

impl VaultliError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::RootNotFound(_) => "ROOT_NOT_FOUND",
            Self::RootExists(_) => "ROOT_EXISTS",
            Self::PathOutsideRoot(_) => "PATH_OUTSIDE_ROOT",
            Self::FileNotFound(_) => "FILE_NOT_FOUND",
            Self::NotMarkdown(_) => "NOT_MARKDOWN",
            Self::MalformedFrontmatter(_) => "MALFORMED_FRONTMATTER",
            Self::InvalidFrontmatter(_, _) => "INVALID_FRONTMATTER",
            Self::IndexMissing(_) => "INDEX_MISSING",
            Self::InvalidIndex => "INDEX_INVALID",
            Self::MissingRequiredFields(_) => "MISSING_REQUIRED_FIELDS",
            Self::BrokenSource(_, _) => "BROKEN_SOURCE",
            Self::IdNotFound(_) => "ID_NOT_FOUND",
            Self::FrontmatterExists(_) => "FRONTMATTER_EXISTS",
            Self::SidecarExists(_) => "SIDECAR_EXISTS",
            Self::NotAFile(_) => "NOT_A_FILE",
            Self::JqUnavailable => "JQ_UNAVAILABLE",
            Self::JqFilterFailed(_) => "JQ_FILTER_FAILED",
            Self::JqFilterInvalid => "JQ_FILTER_INVALID",
            Self::Unsupported(_) => "UNSUPPORTED",
            Self::Io(_) => "IO_ERROR",
            Self::Json(_) => "JSON_ERROR",
        }
    }
}
