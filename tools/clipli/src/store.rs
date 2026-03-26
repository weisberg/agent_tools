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
    #[allow(dead_code)]
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

        std::fs::create_dir_all(&dir)?;

        // template HTML — filename depends on templatized flag
        let template_filename = if content.is_templatized {
            "template.html.j2"
        } else {
            "template.html"
        };
        std::fs::write(dir.join(template_filename), &content.template_html)?;

        // meta.json — always refresh `updated_at`
        let mut meta = content.meta;
        meta.updated_at = Utc::now();
        let meta_json = serde_json::to_string_pretty(&meta)?;
        std::fs::write(dir.join("meta.json"), meta_json)?;

        // schema.json (optional)
        if let Some(schema) = content.schema {
            let schema_json = serde_json::to_string_pretty(&schema)?;
            std::fs::write(dir.join("schema.json"), schema_json)?;
        }

        // original.html (optional)
        if let Some(original) = content.original_html {
            std::fs::write(dir.join("original.html"), original)?;
        }

        // raw.html (optional)
        if let Some(raw) = content.raw_html {
            std::fs::write(dir.join("raw.html"), raw)?;
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
}
