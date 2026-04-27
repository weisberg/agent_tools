use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process;

use crate::*;

#[derive(Debug, Clone)]
pub(crate) struct MarkdownDocument {
    pub(crate) source_path: Option<PathBuf>,
    pub(crate) lines: Vec<String>,
    pub(crate) line_ending: String,
    pub(crate) trailing_newline: bool,
    pub(crate) bom: bool,
    pub(crate) preimage_hash: String,
}

impl MarkdownDocument {
    pub(crate) fn read(path: &Path) -> Result<Self, MdliError> {
        let mut bytes = Vec::new();
        if path == Path::new("-") {
            io::stdin()
                .read_to_end(&mut bytes)
                .map_err(|e| MdliError::io("E_READ_FAILED", "failed to read stdin", e))?;
        } else {
            bytes = fs::read(path).map_err(|e| {
                MdliError::io(
                    "E_READ_FAILED",
                    format!("failed to read {}", path.display()),
                    e,
                )
            })?;
        }
        Self::from_bytes(
            if path == Path::new("-") {
                None
            } else {
                Some(path.to_path_buf())
            },
            bytes,
        )
    }

    pub(crate) fn from_bytes(
        source_path: Option<PathBuf>,
        mut bytes: Vec<u8>,
    ) -> Result<Self, MdliError> {
        let preimage_hash = sha256_prefixed(&bytes);
        let bom = bytes.starts_with(&[0xef, 0xbb, 0xbf]);
        if bom {
            bytes.drain(0..3);
        }
        let text = String::from_utf8(bytes)
            .map_err(|_| MdliError::user("E_INVALID_UTF8", "input is not valid UTF-8"))?;
        let crlf = text.matches("\r\n").count();
        let lf = text.matches('\n').count();
        let line_ending = if crlf > 0 && crlf == lf { "\r\n" } else { "\n" }.to_string();
        let normalized = text.replace("\r\n", "\n");
        let trailing_newline = normalized.ends_with('\n');
        let body = if trailing_newline {
            &normalized[..normalized.len().saturating_sub(1)]
        } else {
            &normalized
        };
        let lines = if body.is_empty() {
            Vec::new()
        } else {
            body.split('\n').map(ToString::to_string).collect()
        };
        Ok(Self {
            source_path,
            lines,
            line_ending,
            trailing_newline,
            bom,
            preimage_hash,
        })
    }

    pub(crate) fn render(&self) -> String {
        let mut out = String::new();
        if self.bom {
            out.push('\u{feff}');
        }
        out.push_str(&self.lines.join(&self.line_ending));
        if self.trailing_newline || !self.lines.is_empty() {
            out.push_str(&self.line_ending);
        }
        out
    }

    pub(crate) fn write_atomic(&self) -> Result<(), MdliError> {
        let path = self
            .source_path
            .as_ref()
            .ok_or_else(|| MdliError::user("E_WRITE_FAILED", "cannot --write when FILE is -"))?;
        let parent = path.parent().unwrap_or_else(|| Path::new("."));
        let file_name = path
            .file_name()
            .and_then(|s| s.to_str())
            .ok_or_else(|| MdliError::user("E_WRITE_FAILED", "invalid target path"))?;
        let tmp_path = parent.join(format!(
            "{file_name}.mdli-tmp.{}.{}",
            process::id(),
            short_hash(self.render().as_bytes())
        ));
        let existing_mode = fs::metadata(path).ok();
        {
            let mut file = fs::File::create(&tmp_path).map_err(|e| {
                MdliError::io("E_WRITE_FAILED", "failed to create temporary file", e)
            })?;
            file.write_all(self.render().as_bytes()).map_err(|e| {
                MdliError::io("E_WRITE_FAILED", "failed to write temporary file", e)
            })?;
            file.sync_all()
                .map_err(|e| MdliError::io("E_WRITE_FAILED", "failed to sync temporary file", e))?;
        }
        if let Some(meta) = existing_mode {
            let _ = fs::set_permissions(&tmp_path, meta.permissions());
        }
        fs::rename(&tmp_path, path)
            .map_err(|e| MdliError::io("E_WRITE_FAILED", "failed to replace target file", e))?;
        Ok(())
    }

    pub(crate) fn assert_preimage(&self, expected: &Option<String>) -> Result<(), MdliError> {
        if let Some(expected) = expected {
            if &self.preimage_hash != expected {
                return Err(MdliError::io(
                    "E_STALE_PREIMAGE",
                    "preimage hash does not match input",
                    io::Error::other("stale preimage"),
                ));
            }
        }
        Ok(())
    }
}
