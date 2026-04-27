use std::fs;
use std::io::{self, Read};
use std::path::Path;

use sha2::{Digest, Sha256};

use crate::*;

pub(crate) fn read_text_path(path: &Path) -> Result<String, MdliError> {
    if path == Path::new("-") {
        let mut text = String::new();
        io::stdin()
            .read_to_string(&mut text)
            .map_err(|e| MdliError::io("E_READ_FAILED", "failed to read stdin", e))?;
        Ok(text)
    } else {
        fs::read_to_string(path).map_err(|e| {
            MdliError::io(
                "E_READ_FAILED",
                format!("failed to read {}", path.display()),
                e,
            )
        })
    }
}

pub(crate) fn split_body_lines(text: &str) -> Vec<String> {
    let normalized = text.replace("\r\n", "\n");
    let body = normalized.trim_end_matches('\n');
    if body.is_empty() {
        Vec::new()
    } else {
        body.split('\n').map(ToString::to_string).collect()
    }
}

pub(crate) fn validate_write_emit(flags: &MutateArgs) -> Result<(), MdliError> {
    if flags.write && flags.emit == EmitMode::Document {
        return Err(MdliError::user(
            "E_INVALID_OUTPUT_MODE",
            "--write and --emit document are mutually exclusive",
        ));
    }
    Ok(())
}

pub(crate) fn sha256_prefixed(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    format!("sha256:{digest:x}")
}

pub(crate) fn short_hash(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    format!("{digest:x}").chars().take(10).collect()
}
