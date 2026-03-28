use std::path::Path;

use crate::error::VaultliError;
use crate::paths::canonicalize_or_join;

pub fn make_id(file: &Path, root: &Path) -> Result<String, VaultliError> {
    let root = root.canonicalize()?;
    let file = canonicalize_or_join(file)?;
    let relative = file
        .strip_prefix(&root)
        .map_err(|_| VaultliError::PathOutsideRoot(file.display().to_string()))?;
    let mut rendered = relative.to_string_lossy().replace('\\', "/");
    if rendered.ends_with(".md") {
        rendered.truncate(rendered.len() - 3);
    }
    if rendered
        .rsplit('/')
        .next()
        .map(|name| name.contains('.'))
        .unwrap_or(false)
    {
        if let Some(index) = rendered.rfind('.') {
            rendered.truncate(index);
        }
    }
    Ok(rendered.replace(['_', ' '], "-").to_lowercase())
}
