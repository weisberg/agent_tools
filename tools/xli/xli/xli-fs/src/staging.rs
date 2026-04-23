use std::path::Path;
use tempfile::NamedTempFile;

use xli_core::XliError;

/// Stage a temp file in the same directory as the source workbook.
pub fn stage_temp_file(parent_dir: &Path) -> Result<NamedTempFile, XliError> {
    NamedTempFile::new_in(parent_dir).map_err(|error| XliError::WriteConflict {
        target: parent_dir.display().to_string(),
        details: Some(error.to_string()),
    })
}
