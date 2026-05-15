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

#[derive(Debug, Default, Clone)]
pub struct HistoryFilter {
    pub source_app: Option<String>,
    pub pb_type: Option<PbType>,
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
}

impl HistoryFilter {
    pub fn is_empty(&self) -> bool {
        self.source_app.is_none()
            && self.pb_type.is_none()
            && self.from.is_none()
            && self.to.is_none()
    }

    pub fn matches(&self, entry: &HistoryEntry) -> bool {
        if let Some(ref source_app) = self.source_app {
            let haystack = entry
                .source_app
                .as_deref()
                .unwrap_or_default()
                .to_ascii_lowercase();
            if !haystack.contains(&source_app.to_ascii_lowercase()) {
                return false;
            }
        }
        if let Some(pb_type) = self.pb_type {
            if entry.pb_type != pb_type {
                return false;
            }
        }
        if let Some(from) = self.from {
            if entry.captured_at < from {
                return false;
            }
        }
        if let Some(to) = self.to {
            if entry.captured_at > to {
                return false;
            }
        }
        true
    }
}

#[derive(Debug, Serialize)]
pub struct PruneResult {
    pub removed: usize,
    pub kept: usize,
    pub dry_run: bool,
    pub removed_entries: Vec<HistoryEntry>,
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
        let _lock = self.acquire_lock()?;
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
        entries.sort_by_key(|entry| std::cmp::Reverse(entry.captured_at));
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

    pub fn list_filtered(
        &self,
        filter: &HistoryFilter,
    ) -> Result<Vec<HistoryEntry>, Box<dyn std::error::Error>> {
        Ok(self
            .list()?
            .into_iter()
            .filter(|entry| filter.matches(entry))
            .collect())
    }

    pub fn search_filtered(
        &self,
        query: &str,
        filter: &HistoryFilter,
    ) -> Result<Vec<HistoryEntry>, Box<dyn std::error::Error>> {
        let needle = query.to_ascii_lowercase();
        let mut matches = Vec::new();
        for entry in self.list_filtered(filter)? {
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

    pub fn prune(
        &self,
        filter: &HistoryFilter,
        keep_latest: Option<usize>,
        dry_run: bool,
    ) -> Result<PruneResult, Box<dyn std::error::Error>> {
        if filter.is_empty() && keep_latest.is_none() {
            return Err("history prune requires at least one filter or --keep-latest".into());
        }

        let _lock = if dry_run {
            None
        } else {
            Some(self.acquire_lock()?)
        };
        let entries = self.list()?;
        let mut kept = Vec::new();
        let mut removed = Vec::new();
        let mut matching_seen = 0usize;

        for entry in entries {
            if filter.matches(&entry) {
                matching_seen += 1;
                if keep_latest
                    .map(|keep| matching_seen <= keep)
                    .unwrap_or(false)
                {
                    kept.push(entry);
                } else {
                    removed.push(entry);
                }
            } else {
                kept.push(entry);
            }
        }

        if !dry_run {
            self.write_index(&kept)?;
            for entry in &removed {
                if let Some(ref rel) = entry.payload_path {
                    let path = self.root.join(rel);
                    if path.exists() {
                        fs::remove_file(path)?;
                    }
                }
            }
        }

        Ok(PruneResult {
            removed: removed.len(),
            kept: kept.len(),
            dry_run,
            removed_entries: removed,
        })
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

    fn write_index(&self, entries: &[HistoryEntry]) -> Result<(), Box<dyn std::error::Error>> {
        fs::create_dir_all(&self.root)?;
        let tmp = self.root.join("index.jsonl.tmp");
        let mut file = fs::File::create(&tmp)?;
        for entry in entries {
            writeln!(file, "{}", serde_json::to_string(entry)?)?;
        }
        fs::rename(tmp, self.index_path())?;
        Ok(())
    }

    fn index_path(&self) -> PathBuf {
        self.root.join("index.jsonl")
    }

    fn payloads_dir(&self) -> PathBuf {
        self.root.join("payloads")
    }

    fn lock_path(&self) -> PathBuf {
        self.root.join(".history.lock")
    }

    fn acquire_lock(&self) -> Result<HistoryLock, Box<dyn std::error::Error>> {
        fs::create_dir_all(&self.root)?;
        let path = self.lock_path();
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut file) => {
                writeln!(file, "pid={}", std::process::id())?;
                Ok(HistoryLock { path })
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Err(format!(
                "history store is locked at {}; another clipli history/watch process may be running",
                path.display()
            )
            .into()),
            Err(e) => Err(e.into()),
        }
    }
}

#[derive(Debug)]
struct HistoryLock {
    path: PathBuf,
}

impl Drop for HistoryLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
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
