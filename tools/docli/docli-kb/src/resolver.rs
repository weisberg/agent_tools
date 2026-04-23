use std::{
    fs,
    path::{Path, PathBuf},
};

use docli_core::DocliError;

use crate::rules::load_rules;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KbValidationIssue {
    pub path: PathBuf,
    pub message: String,
}

#[derive(Clone, Debug)]
pub struct KbResolver {
    kb_root: PathBuf,
}

impl KbResolver {
    pub fn new(kb_root: impl Into<PathBuf>) -> Self {
        Self {
            kb_root: kb_root.into(),
        }
    }

    pub fn kb_root(&self) -> &Path {
        &self.kb_root
    }

    pub fn templates_root(&self) -> PathBuf {
        self.kb_root.join("templates/docli")
    }

    pub fn resolve_uri(&self, uri: &str) -> Result<PathBuf, DocliError> {
        let relative = uri
            .strip_prefix("kb://")
            .ok_or_else(|| DocliError::RefNotFound {
                reference: uri.to_string(),
            })?;
        let path = self.templates_root().join(relative);
        if !path.exists() {
            return Err(DocliError::RefNotFound {
                reference: uri.to_string(),
            });
        }
        Ok(path)
    }

    pub fn list_entries(&self, category: &str) -> Result<Vec<String>, DocliError> {
        let directory = self.templates_root().join(category);
        let entries = fs::read_dir(&directory).map_err(|_| DocliError::TemplateNotFound {
            template: category.to_string(),
        })?;
        let mut names = entries
            .filter_map(Result::ok)
            .filter(|entry| entry.file_type().ok().is_some_and(|kind| kind.is_file()))
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        names.sort();
        Ok(names)
    }

    pub fn validate_all(&self) -> Result<Vec<KbValidationIssue>, DocliError> {
        let mut issues = Vec::new();
        let templates_root = self.templates_root();

        for category in ["styles", "specs", "sections", "assets", "rules", "examples"] {
            let path = templates_root.join(category);
            if !path.exists() {
                issues.push(KbValidationIssue {
                    path,
                    message: format!("missing KB category directory: {category}"),
                });
            }
        }

        for rule in load_rules(&templates_root.join("rules"))? {
            if rule.metadata.title.trim().is_empty() {
                issues.push(KbValidationIssue {
                    path: rule.path,
                    message: "rule title must not be empty".to_string(),
                });
            }
        }

        Ok(issues)
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::KbResolver;

    #[test]
    fn resolves_kb_uris_into_templates_namespace() {
        let dir = tempdir().unwrap();
        let target = dir
            .path()
            .join("templates/docli/sections/cuped-methodology.yaml");
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(&target, "section: ok").unwrap();

        let resolver = KbResolver::new(dir.path());
        let resolved = resolver
            .resolve_uri("kb://sections/cuped-methodology.yaml")
            .unwrap();

        assert_eq!(resolved, target);
    }

    #[test]
    fn lists_entries_in_a_category() {
        let dir = tempdir().unwrap();
        let styles = dir.path().join("templates/docli/styles");
        fs::create_dir_all(&styles).unwrap();
        fs::write(styles.join("a.yaml"), "").unwrap();
        fs::write(styles.join("b.yaml"), "").unwrap();

        let resolver = KbResolver::new(dir.path());
        assert_eq!(
            resolver.list_entries("styles").unwrap(),
            vec!["a.yaml".to_string(), "b.yaml".to_string()]
        );
    }
}
