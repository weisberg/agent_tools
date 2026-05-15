use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::model::PbType;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SensitivePolicy {
    Skip,
    Redact,
    Allow,
}

impl SensitivePolicy {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value.to_ascii_lowercase().as_str() {
            "skip" => Ok(Self::Skip),
            "redact" => Ok(Self::Redact),
            "allow" => Ok(Self::Allow),
            other => Err(format!(
                "unknown sensitive policy '{other}': use skip, redact, or allow"
            )),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub id: String,
    pub captured_at: DateTime<Utc>,
    pub source_app: Option<String>,
    pub pb_type: PbType,
    pub uti: String,
    pub size_bytes: usize,
    pub sha256: String,
    pub payload_path: Option<String>,
    pub redacted: bool,
    pub privacy_reason: Option<String>,
}

#[derive(Debug)]
pub struct HistoryStore {
    root: PathBuf,
}

impl HistoryStore {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn record(
        &self,
        pb_type: PbType,
        data: &[u8],
        source_app: Option<String>,
        policy: SensitivePolicy,
    ) -> Result<HistoryEntry, Box<dyn std::error::Error>> {
        fs::create_dir_all(self.payloads_dir())?;
        let captured_at = Utc::now();
        let hash = sha256_hex(data);
        let id = format!("{}-{}", captured_at.timestamp_millis(), &hash[..12]);
        let sensitive = is_sensitive(pb_type, data);

        let (payload, redacted, privacy_reason) = match (sensitive, policy) {
            (true, SensitivePolicy::Skip) => (
                None,
                true,
                Some("sensitive text detected; payload not stored".to_string()),
            ),
            (true, SensitivePolicy::Redact) => (
                Some(b"[redacted by clipli history privacy policy]\n".to_vec()),
                true,
                Some("sensitive text detected; payload redacted".to_string()),
            ),
            _ => (Some(data.to_vec()), false, None),
        };

        let payload_path = if let Some(payload) = payload {
            let file_name = format!("{}.{}", id, extension_for(pb_type));
            let path = self.payloads_dir().join(&file_name);
            fs::write(&path, payload)?;
            Some(format!("payloads/{file_name}"))
        } else {
            None
        };

        let entry = HistoryEntry {
            id,
            captured_at,
            source_app,
            pb_type,
            uti: pb_type.uti().to_string(),
            size_bytes: data.len(),
            sha256: hash,
            payload_path,
            redacted,
            privacy_reason,
        };
        self.append_entry(&entry)?;
        Ok(entry)
    }

    pub fn list(&self) -> Result<Vec<HistoryEntry>, Box<dyn std::error::Error>> {
        let path = self.index_path();
        if !path.exists() {
            return Ok(Vec::new());
        }
        let contents = fs::read_to_string(path)?;
        let mut entries = Vec::new();
        for line in contents.lines().filter(|line| !line.trim().is_empty()) {
            entries.push(serde_json::from_str::<HistoryEntry>(line)?);
        }
        entries.sort_by(|a, b| b.captured_at.cmp(&a.captured_at));
        Ok(entries)
    }

    pub fn search(&self, query: &str) -> Result<Vec<HistoryEntry>, Box<dyn std::error::Error>> {
        let needle = query.to_ascii_lowercase();
        let mut matches = Vec::new();
        for entry in self.list()? {
            if self.entry_matches(&entry, &needle)? {
                matches.push(entry);
            }
        }
        Ok(matches)
    }

    pub fn get(&self, id: &str) -> Result<HistoryEntry, Box<dyn std::error::Error>> {
        self.list()?
            .into_iter()
            .find(|entry| entry.id == id)
            .ok_or_else(|| format!("history entry '{id}' not found").into())
    }

    pub fn payload(&self, entry: &HistoryEntry) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let rel = entry
            .payload_path
            .as_ref()
            .ok_or_else(|| format!("history entry '{}' has no stored payload", entry.id))?;
        Ok(fs::read(self.root.join(rel))?)
    }

    fn entry_matches(
        &self,
        entry: &HistoryEntry,
        needle: &str,
    ) -> Result<bool, Box<dyn std::error::Error>> {
        if entry.id.to_ascii_lowercase().contains(needle)
            || entry.uti.to_ascii_lowercase().contains(needle)
            || entry.sha256.to_ascii_lowercase().contains(needle)
            || entry
                .source_app
                .as_deref()
                .unwrap_or_default()
                .to_ascii_lowercase()
                .contains(needle)
        {
            return Ok(true);
        }
        if is_text_type(entry.pb_type) {
            if let Ok(payload) = self.payload(entry) {
                if String::from_utf8_lossy(&payload)
                    .to_ascii_lowercase()
                    .contains(needle)
                {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }

    fn append_entry(&self, entry: &HistoryEntry) -> Result<(), Box<dyn std::error::Error>> {
        fs::create_dir_all(&self.root)?;
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.index_path())?;
        writeln!(file, "{}", serde_json::to_string(entry)?)?;
        Ok(())
    }

    fn index_path(&self) -> PathBuf {
        self.root.join("index.jsonl")
    }

    fn payloads_dir(&self) -> PathBuf {
        self.root.join("payloads")
    }
}

pub fn sha256_hex(data: &[u8]) -> String {
    let digest = Sha256::digest(data);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub fn is_text_type(pb_type: PbType) -> bool {
    matches!(
        pb_type,
        PbType::Html | PbType::Rtf | PbType::PlainText | PbType::Svg
    )
}

pub fn is_sensitive(pb_type: PbType, data: &[u8]) -> bool {
    if !is_text_type(pb_type) {
        return false;
    }
    let text = String::from_utf8_lossy(data).to_ascii_lowercase();
    let sensitive_markers = [
        "api_key",
        "apikey",
        "access_token",
        "auth token",
        "bearer ",
        "client_secret",
        "password",
        "private key",
        "secret=",
        "token=",
    ];
    sensitive_markers.iter().any(|marker| text.contains(marker))
}

fn extension_for(pb_type: PbType) -> &'static str {
    match pb_type {
        PbType::Html => "html",
        PbType::Rtf => "rtf",
        PbType::PlainText => "txt",
        PbType::Svg => "svg",
        PbType::Png => "png",
        PbType::Tiff => "tiff",
        PbType::Pdf => "pdf",
        PbType::Unknown => "bin",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sensitive_detector_flags_common_secret_markers() {
        assert!(is_sensitive(
            PbType::PlainText,
            b"NOTION_API_KEY=secret-token"
        ));
        assert!(!is_sensitive(PbType::PlainText, b"normal clipboard text"));
        assert!(!is_sensitive(PbType::Png, b"password"));
    }
}
