// Template store — filesystem CRUD for ~/.config/clipli/templates/.
// See CLIPLI_SPEC.md §5.5 for full specification.

use std::path::PathBuf;

use chrono::Utc;

use crate::model::{TemplateMeta, TemplateVariable};

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("template '{0}' not found")]
    NotFound(String),
    #[error("template '{0}' already exists (use --force to overwrite)")]
    AlreadyExists(String),
    #[error("filesystem error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),
}

impl StoreError {
    /// Error code string suitable for structured JSON output.
    pub fn code(&self) -> &'static str {
        match self {
            Self::NotFound(_)     => "STORE_NOT_FOUND",
            Self::AlreadyExists(_) => "STORE_ALREADY_EXISTS",
            Self::Io(_)           => "STORE_IO_ERROR",
            Self::Json(_)         => "STORE_IO_ERROR",
        }
    }
}

// ---------------------------------------------------------------------------
// Public data structs
// ---------------------------------------------------------------------------

/// Content passed to `Store::save`.
pub struct SaveContent {
    /// Content of `template.html.j2` (when `is_templatized = true`) or
    /// `template.html` (when `is_templatized = false`).
    pub template_html: String,
    /// When `true` the file is written as `template.html.j2`; otherwise `.html`.
    pub is_templatized: bool,
    /// Template metadata to persist as `meta.json`.
    pub meta: TemplateMeta,
    /// Optional variable schema to persist as `schema.json`.
    pub schema: Option<Vec<TemplateVariable>>,
    /// Cleaned HTML before templatization (`original.html`), optional.
    pub original_html: Option<String>,
    /// Uncleaned original from the pasteboard (`raw.html`), optional.
    pub raw_html: Option<String>,
}

/// Everything returned by `Store::load`.
#[derive(Debug)]
pub struct LoadedTemplate {
    /// Contents of the template HTML file.
    pub template_html: String,
    /// Parsed `meta.json`.
    pub meta: TemplateMeta,
    /// Parsed `schema.json`; empty vec when the file is absent.
    pub schema: Vec<TemplateVariable>,
    /// `true` when the on-disk file is `template.html.j2`.
    #[allow(dead_code)]
    pub is_templatized: bool,
}

/// A version snapshot entry.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct VersionEntry {
    pub id: String,
    pub change_type: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// A search result.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SearchResult {
    pub name: String,
    pub match_field: String,
    pub match_context: String,
    pub description: Option<String>,
}

/// Optional filter for `Store::list`.
pub struct ListFilter {
    /// If set, only return templates whose `tags` contains this value.
    pub tag: Option<String>,
    /// When `true`, only return templates where `meta.templatized == true`.
    pub templatized_only: bool,
}

// ---------------------------------------------------------------------------
// Store
// ---------------------------------------------------------------------------

fn version_id() -> String {
    chrono::Utc::now().format("%Y%m%dT%H%M%SZ").to_string()
}

/// Manages the template directory tree under `~/.config/clipli/templates/`.
pub struct Store {
    root: PathBuf,
}

impl Store {
    // ------------------------------------------------------------------
    // Constructors
    // ------------------------------------------------------------------

    /// Create a `Store` pointing at the default `~/.config/clipli/templates/`
    /// directory, creating it (and any parents) if it does not exist.
    pub fn new() -> Result<Self, StoreError> {
        let config_dir = dirs::config_dir()
            .unwrap_or_else(|| {
                dirs::home_dir()
                    .expect("cannot determine home directory")
                    .join(".config")
            });
        let root = config_dir.join("clipli").join("templates");
        Self::with_root(root)
    }

    /// Create a `Store` at a custom path.  The directory is created if it
    /// does not already exist.  Primarily used in tests with a `TempDir`.
    pub fn with_root(root: impl Into<PathBuf>) -> Result<Self, StoreError> {
        let root = root.into();
        std::fs::create_dir_all(&root)?;
        Ok(Self { root })
    }

    // ------------------------------------------------------------------
    // CRUD
    // ------------------------------------------------------------------

    /// Persist a template.
    ///
    /// Returns [`StoreError::AlreadyExists`] when the template directory
    /// already exists and `force` is `false`.  When `force` is `true` the
    /// existing content is silently overwritten.
    pub fn save(&self, name: &str, content: SaveContent, force: bool) -> Result<(), StoreError> {
        let dir = self.template_dir(name);

        if dir.exists() && !force {
            return Err(StoreError::AlreadyExists(name.to_string()));
        }

        // Auto-snapshot before overwrite
        if dir.exists() && force {
            let _ = self.snapshot(name, "overwrite");
        }

        // Atomic write: build in temp dir, then swap
        let tmp_name = format!(".{}.tmp.{}", name, std::process::id());
        let tmp_dir = self.root.join(&tmp_name);
        if tmp_dir.exists() {
            std::fs::remove_dir_all(&tmp_dir)?;
        }
        std::fs::create_dir_all(&tmp_dir)?;

        // Write template HTML
        let template_filename = if content.is_templatized { "template.html.j2" } else { "template.html" };
        std::fs::write(tmp_dir.join(template_filename), &content.template_html)?;

        // Write meta.json
        let mut meta = content.meta;
        meta.updated_at = Utc::now();
        std::fs::write(tmp_dir.join("meta.json"), serde_json::to_string_pretty(&meta)?)?;

        // Write schema.json (optional)
        if let Some(schema) = content.schema {
            std::fs::write(tmp_dir.join("schema.json"), serde_json::to_string_pretty(&schema)?)?;
        }

        // Write original.html (optional)
        if let Some(original) = content.original_html {
            std::fs::write(tmp_dir.join("original.html"), original)?;
        }

        // Write raw.html (optional)
        if let Some(raw) = content.raw_html {
            std::fs::write(tmp_dir.join("raw.html"), raw)?;
        }

        // Atomic swap
        if dir.exists() {
            let versions_dir = dir.join("versions");
            let has_versions = versions_dir.exists();
            let versions_tmp = self.root.join(format!(".{}.versions.{}", name, std::process::id()));
            if has_versions {
                std::fs::rename(&versions_dir, &versions_tmp)?;
            }
            std::fs::remove_dir_all(&dir)?;
            std::fs::rename(&tmp_dir, &dir)?;
            if has_versions {
                std::fs::rename(&versions_tmp, dir.join("versions"))?;
            }
        } else {
            std::fs::rename(&tmp_dir, &dir)?;
        }

        Ok(())
    }

    /// Load a template by name.
    ///
    /// Prefers `template.html.j2` over `template.html` when both exist.
    /// Returns [`StoreError::NotFound`] when the directory or `meta.json` is
    /// absent.
    pub fn load(&self, name: &str) -> Result<LoadedTemplate, StoreError> {
        let dir = self.template_dir(name);
        if !dir.exists() {
            return Err(StoreError::NotFound(name.to_string()));
        }

        // template HTML — prefer .j2
        let (template_html, is_templatized) = {
            let j2 = dir.join("template.html.j2");
            let plain = dir.join("template.html");
            if j2.exists() {
                (std::fs::read_to_string(&j2)?, true)
            } else if plain.exists() {
                (std::fs::read_to_string(&plain)?, false)
            } else {
                return Err(StoreError::NotFound(name.to_string()));
            }
        };

        // meta.json
        let meta_path = dir.join("meta.json");
        if !meta_path.exists() {
            return Err(StoreError::NotFound(name.to_string()));
        }
        let meta_str = std::fs::read_to_string(&meta_path)?;
        let meta: TemplateMeta = serde_json::from_str(&meta_str)?;

        // schema.json — empty vec when absent
        let schema_path = dir.join("schema.json");
        let schema: Vec<TemplateVariable> = if schema_path.exists() {
            let schema_str = std::fs::read_to_string(&schema_path)?;
            serde_json::from_str(&schema_str)?
        } else {
            vec![]
        };

        Ok(LoadedTemplate {
            template_html,
            meta,
            schema,
            is_templatized,
        })
    }

    /// List all templates, applying an optional filter.
    ///
    /// Directories without a readable `meta.json` are silently skipped.
    /// Results are sorted alphabetically by template name.
    pub fn list(&self, filter: Option<&ListFilter>) -> Result<Vec<TemplateMeta>, StoreError> {
        let mut metas: Vec<TemplateMeta> = Vec::new();

        let read_dir = std::fs::read_dir(&self.root)?;

        for entry in read_dir {
            let entry = entry?;
            let path = entry.path();

            if !path.is_dir() {
                continue;
            }

            let meta_path = path.join("meta.json");
            if !meta_path.exists() {
                continue; // skip dirs without meta.json
            }

            let meta_str = match std::fs::read_to_string(&meta_path) {
                Ok(s) => s,
                Err(_) => continue, // skip unreadable files
            };

            let meta: TemplateMeta = match serde_json::from_str(&meta_str) {
                Ok(m) => m,
                Err(_) => continue, // skip malformed JSON
            };

            // Apply filters
            if let Some(f) = filter {
                if f.templatized_only && !meta.templatized {
                    continue;
                }
                if let Some(ref tag) = f.tag {
                    if !meta.tags.contains(tag) {
                        continue;
                    }
                }
            }

            metas.push(meta);
        }

        metas.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(metas)
    }

    /// Delete a template directory and all its contents.
    ///
    /// Returns [`StoreError::NotFound`] when the directory does not exist.
    pub fn delete(&self, name: &str) -> Result<(), StoreError> {
        let dir = self.template_dir(name);
        if !dir.exists() {
            return Err(StoreError::NotFound(name.to_string()));
        }
        std::fs::remove_dir_all(&dir)?;
        Ok(())
    }

    // ------------------------------------------------------------------
    // Helpers
    // ------------------------------------------------------------------

    /// Return `true` when a template directory exists.
    #[allow(dead_code)]
    pub fn exists(&self, name: &str) -> bool {
        self.template_dir(name).exists()
    }

    /// Return the path to a template's directory (does not verify existence).
    pub fn template_dir(&self, name: &str) -> PathBuf {
        self.root.join(name)
    }

    /// Return the path to the template HTML file, detecting `.html.j2` vs
    /// `.html`.  Returns `None` when neither file exists.
    pub fn template_file_path(&self, name: &str) -> Option<PathBuf> {
        let dir = self.template_dir(name);
        let j2 = dir.join("template.html.j2");
        if j2.exists() {
            return Some(j2);
        }
        let plain = dir.join("template.html");
        if plain.exists() {
            return Some(plain);
        }
        None
    }

    // ------------------------------------------------------------------
    // Versioning
    // ------------------------------------------------------------------

    /// Create a snapshot of the current template files in `versions/<timestamp>/`.
    pub fn snapshot(&self, name: &str, change_type: &str) -> Result<String, StoreError> {
        let dir = self.template_dir(name);
        if !dir.exists() {
            return Err(StoreError::NotFound(name.to_string()));
        }
        let id = version_id();
        let versions_dir = dir.join("versions");
        let dest = versions_dir.join(&id);
        std::fs::create_dir_all(&dest)?;
        // Copy only files (not subdirectories like versions/) from template dir
        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                std::fs::copy(&path, dest.join(entry.file_name()))?;
            }
        }
        // Write version metadata
        let meta = serde_json::json!({
            "change_type": change_type,
            "timestamp": chrono::Utc::now().to_rfc3339(),
        });
        std::fs::write(dest.join("_version_meta.json"), serde_json::to_string_pretty(&meta)?)?;
        self.prune_versions(name, 20)?;
        Ok(id)
    }

    /// List all version snapshots for a template, newest first.
    pub fn list_versions(&self, name: &str) -> Result<Vec<VersionEntry>, StoreError> {
        let dir = self.template_dir(name);
        if !dir.exists() {
            return Err(StoreError::NotFound(name.to_string()));
        }
        let versions_dir = dir.join("versions");
        if !versions_dir.exists() {
            return Ok(Vec::new());
        }
        let mut entries = Vec::new();
        for entry in std::fs::read_dir(&versions_dir)? {
            let entry = entry?;
            let path = entry.path();
            if !path.is_dir() { continue; }
            let meta_path = path.join("_version_meta.json");
            if !meta_path.exists() { continue; }
            let meta_str = match std::fs::read_to_string(&meta_path) {
                Ok(s) => s,
                Err(_) => continue,
            };
            let meta: serde_json::Value = match serde_json::from_str(&meta_str) {
                Ok(v) => v,
                Err(_) => continue,
            };
            let id = entry.file_name().to_string_lossy().to_string();
            let change_type = meta.get("change_type").and_then(|v| v.as_str()).unwrap_or("unknown").to_string();
            let timestamp_str = meta.get("timestamp").and_then(|v| v.as_str()).unwrap_or("");
            let timestamp = chrono::DateTime::parse_from_rfc3339(timestamp_str)
                .map(|dt| dt.with_timezone(&chrono::Utc))
                .unwrap_or_else(|_| chrono::Utc::now());
            entries.push(VersionEntry { id, change_type, timestamp });
        }
        entries.sort_by(|a, b| b.id.cmp(&a.id)); // newest first
        Ok(entries)
    }

    /// Load a specific version snapshot of a template.
    pub fn load_version(&self, name: &str, version_id: &str) -> Result<LoadedTemplate, StoreError> {
        let dir = self.template_dir(name).join("versions").join(version_id);
        if !dir.exists() {
            return Err(StoreError::NotFound(format!("{} version {}", name, version_id)));
        }
        // Same loading logic as load() but from the version directory
        let (template_html, is_templatized) = {
            let j2 = dir.join("template.html.j2");
            let plain = dir.join("template.html");
            if j2.exists() {
                (std::fs::read_to_string(&j2)?, true)
            } else if plain.exists() {
                (std::fs::read_to_string(&plain)?, false)
            } else {
                return Err(StoreError::NotFound(format!("{} version {}", name, version_id)));
            }
        };
        let meta_path = dir.join("meta.json");
        if !meta_path.exists() {
            return Err(StoreError::NotFound(format!("{} version {}", name, version_id)));
        }
        let meta: TemplateMeta = serde_json::from_str(&std::fs::read_to_string(&meta_path)?)?;
        let schema_path = dir.join("schema.json");
        let schema: Vec<TemplateVariable> = if schema_path.exists() {
            serde_json::from_str(&std::fs::read_to_string(&schema_path)?)?
        } else {
            vec![]
        };
        Ok(LoadedTemplate { template_html, meta, schema, is_templatized })
    }

    /// Restore a version snapshot: snapshots current state, then copies version files back.
    pub fn restore_version(&self, name: &str, version_id: &str) -> Result<(), StoreError> {
        // Snapshot current state first
        self.snapshot(name, "restore")?;
        // Copy all files from the version directory to the template root
        let version_dir = self.template_dir(name).join("versions").join(version_id);
        if !version_dir.exists() {
            return Err(StoreError::NotFound(format!("{} version {}", name, version_id)));
        }
        let template_dir = self.template_dir(name);
        // Delete live files (not versions/)
        for entry in std::fs::read_dir(&template_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                std::fs::remove_file(&path)?;
            }
        }
        // Copy version files to root
        for entry in std::fs::read_dir(&version_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                let fname = entry.file_name();
                // Skip _version_meta.json — it's version-internal
                if fname.to_string_lossy() == "_version_meta.json" { continue; }
                std::fs::copy(&path, template_dir.join(fname))?;
            }
        }
        Ok(())
    }

    /// Keep at most `max` version directories, pruning oldest first.
    fn prune_versions(&self, name: &str, max: usize) -> Result<(), StoreError> {
        let versions_dir = self.template_dir(name).join("versions");
        if !versions_dir.exists() { return Ok(()); }
        let mut dirs: Vec<String> = Vec::new();
        for entry in std::fs::read_dir(&versions_dir)? {
            let entry = entry?;
            if entry.path().is_dir() {
                dirs.push(entry.file_name().to_string_lossy().to_string());
            }
        }
        dirs.sort(); // oldest first (lexicographic on timestamps)
        while dirs.len() > max {
            let oldest = dirs.remove(0);
            let oldest_path = versions_dir.join(&oldest);
            std::fs::remove_dir_all(&oldest_path)?;
        }
        Ok(())
    }

    /// Delete live template files but preserve the versions/ directory.
    pub fn delete_preserving_versions(&self, name: &str) -> Result<(), StoreError> {
        let dir = self.template_dir(name);
        if !dir.exists() {
            return Err(StoreError::NotFound(name.to_string()));
        }
        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() && entry.file_name().to_string_lossy() == "versions" {
                continue; // preserve versions/
            }
            if path.is_dir() {
                std::fs::remove_dir_all(&path)?;
            } else {
                std::fs::remove_file(&path)?;
            }
        }
        Ok(())
    }

    // ------------------------------------------------------------------
    // Search
    // ------------------------------------------------------------------

    /// Full-text search across all templates.
    pub fn search(&self, query: &str, tag_filter: Option<&str>) -> Result<Vec<SearchResult>, StoreError> {
        let query_lower = query.to_lowercase();
        let mut results = Vec::new();

        let read_dir = std::fs::read_dir(&self.root)?;
        for entry in read_dir {
            let entry = entry?;
            let path = entry.path();
            if !path.is_dir() { continue; }
            let meta_path = path.join("meta.json");
            if !meta_path.exists() { continue; }
            let meta_str = match std::fs::read_to_string(&meta_path) {
                Ok(s) => s,
                Err(_) => continue,
            };
            let meta: TemplateMeta = match serde_json::from_str(&meta_str) {
                Ok(m) => m,
                Err(_) => continue,
            };

            // Apply tag filter
            if let Some(tag) = tag_filter {
                if !meta.tags.contains(&tag.to_string()) { continue; }
            }

            let description = meta.description.clone();

            // Check name
            if meta.name.to_lowercase().contains(&query_lower) {
                results.push(SearchResult {
                    name: meta.name.clone(),
                    match_field: "name".to_string(),
                    match_context: meta.name.clone(),
                    description: description.clone(),
                });
                continue;
            }

            // Check description
            if let Some(ref desc) = meta.description {
                if desc.to_lowercase().contains(&query_lower) {
                    let ctx = extract_match_context(desc, &query_lower, 60);
                    results.push(SearchResult {
                        name: meta.name.clone(),
                        match_field: "description".to_string(),
                        match_context: ctx,
                        description: description.clone(),
                    });
                    continue;
                }
            }

            // Check tags
            let mut tag_matched = false;
            for tag in &meta.tags {
                if tag.to_lowercase().contains(&query_lower) {
                    results.push(SearchResult {
                        name: meta.name.clone(),
                        match_field: "tag".to_string(),
                        match_context: tag.clone(),
                        description: description.clone(),
                    });
                    tag_matched = true;
                    break;
                }
            }
            if tag_matched { continue; }

            // Check template HTML content
            let template_path = {
                let j2 = path.join("template.html.j2");
                let html = path.join("template.html");
                if j2.exists() { Some(j2) } else if html.exists() { Some(html) } else { None }
            };
            if let Some(tp) = template_path {
                if let Ok(content) = std::fs::read_to_string(&tp) {
                    if content.to_lowercase().contains(&query_lower) {
                        let ctx = extract_match_context(&content, &query_lower, 60);
                        results.push(SearchResult {
                            name: meta.name.clone(),
                            match_field: "content".to_string(),
                            match_context: ctx,
                            description,
                        });
                    }
                }
            }
        }

        results.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(results)
    }

    // ------------------------------------------------------------------
    // Import / Export
    // ------------------------------------------------------------------

    /// Export a template as a ZIP bundle.
    pub fn export(&self, name: &str, output_path: &std::path::Path) -> Result<(), StoreError> {
        let dir = self.template_dir(name);
        if !dir.exists() {
            return Err(StoreError::NotFound(name.to_string()));
        }

        let file = std::fs::File::create(output_path)
            .map_err(|e| StoreError::Io(e))?;
        let mut zip = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);

        // Write manifest
        let manifest = serde_json::json!({
            "version": 1,
            "name": name,
            "exported_at": chrono::Utc::now().to_rfc3339(),
            "clipli_version": env!("CARGO_PKG_VERSION"),
        });
        zip.start_file("manifest.json", options)
            .map_err(|e| StoreError::Io(std::io::Error::other(e.to_string())))?;
        use std::io::Write;
        zip.write_all(serde_json::to_string_pretty(&manifest)?.as_bytes())
            .map_err(|e| StoreError::Io(e))?;

        // Add all files from template directory (skip versions/)
        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() { continue; } // Skip versions/ and any other subdirs
            let fname = entry.file_name().to_string_lossy().to_string();
            zip.start_file(&fname, options)
                .map_err(|e| StoreError::Io(std::io::Error::other(e.to_string())))?;
            let data = std::fs::read(&path)?;
            zip.write_all(&data)
                .map_err(|e| StoreError::Io(e))?;
        }

        zip.finish()
            .map_err(|e| StoreError::Io(std::io::Error::other(e.to_string())))?;
        Ok(())
    }

    /// Import a template from a ZIP bundle.
    pub fn import(&self, zip_path: &std::path::Path, force: bool, name_override: Option<&str>) -> Result<String, StoreError> {
        let file = std::fs::File::open(zip_path)
            .map_err(|e| StoreError::Io(e))?;
        let mut archive = zip::ZipArchive::new(file)
            .map_err(|e| StoreError::Io(std::io::Error::other(e.to_string())))?;

        // Read manifest to get template name
        let manifest_str = {
            let mut manifest_file = archive.by_name("manifest.json")
                .map_err(|e| StoreError::Io(std::io::Error::other(format!("missing manifest.json: {}", e))))?;
            let mut buf = String::new();
            use std::io::Read;
            manifest_file.read_to_string(&mut buf)
                .map_err(|e| StoreError::Io(e))?;
            buf
        };
        let manifest: serde_json::Value = serde_json::from_str(&manifest_str)?;
        let name = name_override
            .map(|s| s.to_string())
            .or_else(|| manifest.get("name").and_then(|v| v.as_str()).map(|s| s.to_string()))
            .ok_or_else(|| StoreError::Io(std::io::Error::other("manifest.json missing 'name' field")))?;

        if !validate_name(&name) {
            return Err(StoreError::Io(std::io::Error::other(format!("invalid template name: {}", name))));
        }

        let dir = self.template_dir(&name);
        if dir.exists() && !force {
            return Err(StoreError::AlreadyExists(name.clone()));
        }
        if dir.exists() && force {
            // Snapshot before overwriting
            let _ = self.snapshot(&name, "import");
        }

        std::fs::create_dir_all(&dir)?;

        // Extract all files except manifest.json
        for i in 0..archive.len() {
            let mut entry = archive.by_index(i)
                .map_err(|e| StoreError::Io(std::io::Error::other(e.to_string())))?;
            let fname = entry.name().to_string();
            if fname == "manifest.json" { continue; }
            // Security: reject path traversal
            if fname.contains("..") || fname.starts_with('/') { continue; }
            let dest = dir.join(&fname);
            let mut outfile = std::fs::File::create(&dest)?;
            use std::io::Read;
            let mut buf = Vec::new();
            entry.read_to_end(&mut buf)
                .map_err(|e| StoreError::Io(e))?;
            use std::io::Write;
            outfile.write_all(&buf)?;
        }

        Ok(name)
    }
}

// ---------------------------------------------------------------------------
// Validation helpers
// ---------------------------------------------------------------------------

/// Return `true` when `name` is a valid template slug: `[a-zA-Z0-9_-]+`.
///
/// Empty strings are rejected.
pub fn validate_name(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    name.chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// Extract a context snippet around the first occurrence of `query_lower` in `text`.
fn extract_match_context(text: &str, query_lower: &str, window: usize) -> String {
    let text_lower = text.to_lowercase();
    if let Some(pos) = text_lower.find(query_lower) {
        let start = pos.saturating_sub(window / 2);
        let end = (pos + query_lower.len() + window / 2).min(text.len());
        let snippet = &text[start..end];
        let snippet = snippet.trim().replace('\n', " ").replace('\r', "");
        if start > 0 { format!("...{}", snippet) } else { snippet }
    } else {
        text.chars().take(window).collect()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use crate::model::{TemplateMeta, TemplateVariable, VarType};
    use tempfile::TempDir;

    // ------------------------------------------------------------------
    // Helpers
    // ------------------------------------------------------------------

    fn make_store(dir: &TempDir) -> Store {
        Store::with_root(dir.path().join("templates")).unwrap()
    }

    fn make_meta(name: &str) -> TemplateMeta {
        TemplateMeta {
            name: name.to_string(),
            description: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            source_app: None,
            source_pb_types: vec!["public.html".to_string()],
            templatized: false,
            variables: vec![],
            tags: vec![],
        }
    }

    fn make_save_content(name: &str) -> SaveContent {
        SaveContent {
            template_html: "<p>Hello</p>".to_string(),
            is_templatized: false,
            meta: make_meta(name),
            schema: None,
            original_html: None,
            raw_html: None,
        }
    }

    // ------------------------------------------------------------------
    // 1. save → load round-trip
    // ------------------------------------------------------------------

    #[test]
    fn save_load_round_trip() {
        let dir = TempDir::new().unwrap();
        let store = make_store(&dir);

        let html = "<p>Round-trip content</p>".to_string();
        let content = SaveContent {
            template_html: html.clone(),
            is_templatized: false,
            meta: make_meta("round_trip"),
            schema: None,
            original_html: None,
            raw_html: None,
        };

        store.save("round_trip", content, false).unwrap();
        let loaded = store.load("round_trip").unwrap();

        assert_eq!(loaded.template_html, html);
        assert_eq!(loaded.meta.name, "round_trip");
        assert!(!loaded.is_templatized);
    }

    // ------------------------------------------------------------------
    // 2. save without force when template exists → AlreadyExists
    // ------------------------------------------------------------------

    #[test]
    fn save_no_force_already_exists() {
        let dir = TempDir::new().unwrap();
        let store = make_store(&dir);

        store.save("dup", make_save_content("dup"), false).unwrap();

        let err = store.save("dup", make_save_content("dup"), false).unwrap_err();
        assert!(matches!(err, StoreError::AlreadyExists(ref n) if n == "dup"));
        assert_eq!(err.code(), "STORE_ALREADY_EXISTS");
    }

    // ------------------------------------------------------------------
    // 3. save with force=true when template exists → succeeds, overwrites
    // ------------------------------------------------------------------

    #[test]
    fn save_force_overwrites() {
        let dir = TempDir::new().unwrap();
        let store = make_store(&dir);

        store.save("over", make_save_content("over"), false).unwrap();

        let new_html = "<p>Overwritten</p>".to_string();
        let content = SaveContent {
            template_html: new_html.clone(),
            ..make_save_content("over")
        };
        store.save("over", content, true).unwrap();

        let loaded = store.load("over").unwrap();
        assert_eq!(loaded.template_html, new_html);
    }

    // ------------------------------------------------------------------
    // 4. delete then load → NotFound
    // ------------------------------------------------------------------

    #[test]
    fn delete_then_load_not_found() {
        let dir = TempDir::new().unwrap();
        let store = make_store(&dir);

        store.save("del_me", make_save_content("del_me"), false).unwrap();
        store.delete("del_me").unwrap();

        let err = store.load("del_me").unwrap_err();
        assert!(matches!(err, StoreError::NotFound(ref n) if n == "del_me"));
        assert_eq!(err.code(), "STORE_NOT_FOUND");
    }

    // ------------------------------------------------------------------
    // 5. exists returns true after save, false after delete
    // ------------------------------------------------------------------

    #[test]
    fn exists_after_save_and_delete() {
        let dir = TempDir::new().unwrap();
        let store = make_store(&dir);

        assert!(!store.exists("exist_test"));
        store.save("exist_test", make_save_content("exist_test"), false).unwrap();
        assert!(store.exists("exist_test"));
        store.delete("exist_test").unwrap();
        assert!(!store.exists("exist_test"));
    }

    // ------------------------------------------------------------------
    // 6. list returns all templates, filtered by tag
    // ------------------------------------------------------------------

    #[test]
    fn list_all_and_filter_by_tag() {
        let dir = TempDir::new().unwrap();
        let store = make_store(&dir);

        // Save two templates: one with tag "finance", one without
        let mut meta_a = make_meta("alpha");
        meta_a.tags = vec!["finance".to_string()];
        store.save("alpha", SaveContent { meta: meta_a, ..make_save_content("alpha") }, false).unwrap();

        store.save("beta", make_save_content("beta"), false).unwrap();

        // All templates
        let all = store.list(None).unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].name, "alpha"); // alphabetical
        assert_eq!(all[1].name, "beta");

        // Filtered by tag
        let filtered = store.list(Some(&ListFilter {
            tag: Some("finance".to_string()),
            templatized_only: false,
        })).unwrap();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].name, "alpha");
    }

    // ------------------------------------------------------------------
    // 7. list with templatized_only=true only returns templatized templates
    // ------------------------------------------------------------------

    #[test]
    fn list_templatized_only() {
        let dir = TempDir::new().unwrap();
        let store = make_store(&dir);

        let mut meta_t = make_meta("templ");
        meta_t.templatized = true;
        store.save("templ", SaveContent {
            is_templatized: true,
            meta: meta_t,
            ..make_save_content("templ")
        }, false).unwrap();

        store.save("raw_tmpl", make_save_content("raw_tmpl"), false).unwrap();

        let results = store.list(Some(&ListFilter {
            tag: None,
            templatized_only: true,
        })).unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "templ");
    }

    // ------------------------------------------------------------------
    // 8. save with schema → load returns schema
    // ------------------------------------------------------------------

    #[test]
    fn save_and_load_schema() {
        let dir = TempDir::new().unwrap();
        let store = make_store(&dir);

        let vars = vec![
            TemplateVariable {
                name: "revenue".to_string(),
                var_type: VarType::Currency,
                default_value: None,
                description: Some("Total revenue".to_string()),
            },
        ];

        let content = SaveContent {
            schema: Some(vars.clone()),
            ..make_save_content("schema_test")
        };
        store.save("schema_test", content, false).unwrap();

        let loaded = store.load("schema_test").unwrap();
        assert_eq!(loaded.schema.len(), 1);
        assert_eq!(loaded.schema[0].name, "revenue");
    }

    // ------------------------------------------------------------------
    // 9. validate_name tests
    // ------------------------------------------------------------------

    #[test]
    fn validate_name_valid() {
        assert!(validate_name("my_template"));
        assert!(validate_name("MyTemplate"));
        assert!(validate_name("template-1"));
        assert!(validate_name("a"));
        assert!(validate_name("UPPER_CASE"));
        assert!(validate_name("mix_1-2"));
    }

    #[test]
    fn validate_name_invalid() {
        assert!(!validate_name("my template"));  // space
        assert!(!validate_name("../evil"));       // path traversal
        assert!(!validate_name(""));              // empty
        assert!(!validate_name("hello/world"));   // slash
        assert!(!validate_name("foo.bar"));       // dot
        assert!(!validate_name("abc!"));          // exclamation
    }

    // ------------------------------------------------------------------
    // 10. schema file absent → load returns empty schema (not an error)
    // ------------------------------------------------------------------

    #[test]
    fn load_missing_schema_returns_empty_vec() {
        let dir = TempDir::new().unwrap();
        let store = make_store(&dir);

        // Save without schema
        store.save("no_schema", make_save_content("no_schema"), false).unwrap();

        // Confirm schema.json was not written
        let schema_path = store.template_dir("no_schema").join("schema.json");
        assert!(!schema_path.exists());

        let loaded = store.load("no_schema").unwrap();
        assert!(loaded.schema.is_empty());
    }

    // ------------------------------------------------------------------
    // Bonus: template_file_path detects correct extension
    // ------------------------------------------------------------------

    #[test]
    fn template_file_path_detects_extension() {
        let dir = TempDir::new().unwrap();
        let store = make_store(&dir);

        // Non-existent
        assert!(store.template_file_path("ghost").is_none());

        // Plain .html
        store.save("plain", make_save_content("plain"), false).unwrap();
        let p = store.template_file_path("plain").unwrap();
        assert!(p.to_str().unwrap().ends_with("template.html"));

        // Templatized .html.j2
        let mut meta_j2 = make_meta("jinja");
        meta_j2.templatized = true;
        store.save("jinja", SaveContent {
            is_templatized: true,
            meta: meta_j2,
            ..make_save_content("jinja")
        }, false).unwrap();
        let p2 = store.template_file_path("jinja").unwrap();
        assert!(p2.to_str().unwrap().ends_with("template.html.j2"));
    }

    // ------------------------------------------------------------------
    // Bonus: delete a non-existent template → NotFound
    // ------------------------------------------------------------------

    #[test]
    fn delete_nonexistent_is_not_found() {
        let dir = TempDir::new().unwrap();
        let store = make_store(&dir);
        let err = store.delete("ghost").unwrap_err();
        assert!(matches!(err, StoreError::NotFound(_)));
    }

    // ------------------------------------------------------------------
    // Versioning tests
    // ------------------------------------------------------------------

    #[test]
    fn snapshot_creates_version_dir() {
        let dir = TempDir::new().unwrap();
        let store = make_store(&dir);
        store.save("snap_test", make_save_content("snap_test"), false).unwrap();
        let id = store.snapshot("snap_test", "test").unwrap();
        let version_dir = store.template_dir("snap_test").join("versions").join(&id);
        assert!(version_dir.exists());
        assert!(version_dir.join("meta.json").exists());
        assert!(version_dir.join("_version_meta.json").exists());
    }

    #[test]
    fn list_versions_returns_entries() {
        let dir = TempDir::new().unwrap();
        let store = make_store(&dir);
        store.save("ver_test", make_save_content("ver_test"), false).unwrap();
        store.snapshot("ver_test", "edit").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(1100));
        store.snapshot("ver_test", "overwrite").unwrap();
        let versions = store.list_versions("ver_test").unwrap();
        assert_eq!(versions.len(), 2);
        assert_eq!(versions[0].change_type, "overwrite"); // newest first
    }

    #[test]
    fn load_version_returns_snapshot_content() {
        let dir = TempDir::new().unwrap();
        let store = make_store(&dir);
        store.save("lv_test", make_save_content("lv_test"), false).unwrap();
        let id = store.snapshot("lv_test", "edit").unwrap();
        // Wait so the auto-snapshot in force-save gets a distinct version ID
        std::thread::sleep(std::time::Duration::from_millis(1100));
        // Modify live template
        let new_content = SaveContent {
            template_html: "<p>Modified</p>".to_string(),
            ..make_save_content("lv_test")
        };
        store.save("lv_test", new_content, true).unwrap();
        // Load version should have original content
        let loaded = store.load_version("lv_test", &id).unwrap();
        assert_eq!(loaded.template_html, "<p>Hello</p>");
    }

    #[test]
    fn restore_version_reverts_content() {
        let dir = TempDir::new().unwrap();
        let store = make_store(&dir);
        store.save("rv_test", make_save_content("rv_test"), false).unwrap();
        let id = store.snapshot("rv_test", "edit").unwrap();
        // Wait so subsequent snapshots get distinct version IDs (second-resolution)
        std::thread::sleep(std::time::Duration::from_millis(1100));
        // Modify live
        let new_content = SaveContent {
            template_html: "<p>Changed</p>".to_string(),
            ..make_save_content("rv_test")
        };
        store.save("rv_test", new_content, true).unwrap();
        // Wait again so restore's auto-snapshot gets a distinct ID
        std::thread::sleep(std::time::Duration::from_millis(1100));
        // Restore
        store.restore_version("rv_test", &id).unwrap();
        let loaded = store.load("rv_test").unwrap();
        assert_eq!(loaded.template_html, "<p>Hello</p>");
    }

    #[test]
    fn force_save_auto_snapshots() {
        let dir = TempDir::new().unwrap();
        let store = make_store(&dir);
        store.save("fs_test", make_save_content("fs_test"), false).unwrap();
        // Force save should auto-snapshot
        let new_content = SaveContent {
            template_html: "<p>New</p>".to_string(),
            ..make_save_content("fs_test")
        };
        store.save("fs_test", new_content, true).unwrap();
        let versions = store.list_versions("fs_test").unwrap();
        assert!(!versions.is_empty());
        assert_eq!(versions[0].change_type, "overwrite");
    }

    #[test]
    fn delete_preserving_versions_keeps_history() {
        let dir = TempDir::new().unwrap();
        let store = make_store(&dir);
        store.save("dpv_test", make_save_content("dpv_test"), false).unwrap();
        store.snapshot("dpv_test", "test").unwrap();
        store.delete_preserving_versions("dpv_test").unwrap();
        // Live template files should be gone
        assert!(!store.template_dir("dpv_test").join("meta.json").exists());
        // But versions/ should still exist
        assert!(store.template_dir("dpv_test").join("versions").exists());
    }

    #[test]
    fn search_finds_by_name() {
        let dir = TempDir::new().unwrap();
        let store = make_store(&dir);
        store.save("quarterly_report", make_save_content("quarterly_report"), false).unwrap();
        store.save("monthly_summary", make_save_content("monthly_summary"), false).unwrap();
        let results = store.search("quarterly", None).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "quarterly_report");
    }

    #[test]
    fn export_import_round_trip() {
        let dir = TempDir::new().unwrap();
        let store = make_store(&dir);
        store.save("export_test", make_save_content("export_test"), false).unwrap();

        let zip_path = dir.path().join("export_test.clipli");
        store.export("export_test", &zip_path).unwrap();
        assert!(zip_path.exists());

        let imported_name = store.import(&zip_path, false, Some("imported_test")).unwrap();
        assert_eq!(imported_name, "imported_test");

        let loaded = store.load("imported_test").unwrap();
        assert_eq!(loaded.template_html, "<p>Hello</p>");
    }
}
