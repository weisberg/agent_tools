use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use docli_core::{ContentBlock, DocliError};

/// Specification for creating a new document.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CreateSpec {
    /// Optional document title.
    pub title: Option<String>,
    /// Page setup.
    #[serde(default)]
    pub page: PageSetup,
    /// Content blocks to create.
    pub content: Vec<ContentBlock>,
    /// Template variables for rendering.
    #[serde(default)]
    pub vars: Map<String, Value>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct PageSetup {
    /// Page size: "letter", "a4", etc.
    pub size: Option<String>,
    /// Orientation: "portrait" or "landscape".
    pub orientation: Option<String>,
    /// Page margins.
    pub margins: Option<Margins>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Margins {
    pub top: Option<String>,
    pub bottom: Option<String>,
    pub left: Option<String>,
    pub right: Option<String>,
}

impl CreateSpec {
    /// Parse a `CreateSpec` from a YAML string.
    pub fn from_yaml(yaml: &str) -> Result<Self, DocliError> {
        serde_yaml::from_str(yaml).map_err(|e| DocliError::InvalidSpec {
            message: e.to_string(),
        })
    }

    /// Parse from YAML with template rendering via docli-kb.
    /// Renders minijinja variables in the YAML content before parsing.
    pub fn from_yaml_with_vars(yaml: &str, vars: &Map<String, Value>) -> Result<Self, DocliError> {
        let rendered = docli_kb::render_template(yaml, vars)?;
        Self::from_yaml(&rendered)
    }

    /// Resolve any `$ref` content blocks using the KB resolver.
    pub fn resolve_refs(&mut self, resolver: &docli_kb::KbResolver) -> Result<(), DocliError> {
        let mut resolved = Vec::new();
        for block in self.content.drain(..) {
            match &block {
                ContentBlock::Ref { ref_uri } => {
                    let path = resolver.resolve_uri(ref_uri)?;
                    let content =
                        std::fs::read_to_string(&path).map_err(|e| DocliError::RefNotFound {
                            reference: format!("{}: {}", ref_uri, e),
                        })?;
                    let blocks: Vec<ContentBlock> =
                        serde_yaml::from_str(&content).map_err(|e| DocliError::InvalidSpec {
                            message: format!("failed to parse {}: {}", ref_uri, e),
                        })?;
                    resolved.extend(blocks);
                }
                _ => resolved.push(block),
            }
        }
        self.content = resolved;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_valid_yaml() {
        let yaml = r#"
title: "Test Document"
page:
  size: letter
  orientation: portrait
content:
  - heading1: "Introduction"
  - paragraph: "Hello, world."
  - bullets:
      - "Item one"
      - "Item two"
"#;
        let spec = CreateSpec::from_yaml(yaml).unwrap();
        assert_eq!(spec.title.as_deref(), Some("Test Document"));
        assert_eq!(spec.content.len(), 3);
        assert_eq!(spec.page.size.as_deref(), Some("letter"));
    }

    #[test]
    fn rejects_invalid_yaml() {
        let yaml = "content: [[[invalid";
        let result = CreateSpec::from_yaml(yaml);
        assert!(result.is_err());
    }

    #[test]
    fn from_yaml_with_vars_renders_template() {
        let yaml = r#"
title: "{{ doc_title }}"
content:
  - heading1: "{{ doc_title }}"
  - paragraph: "Created by {{ author }}."
"#;
        let mut vars = Map::new();
        vars.insert("doc_title".into(), Value::String("My Report".into()));
        vars.insert("author".into(), Value::String("Alice".into()));

        let spec = CreateSpec::from_yaml_with_vars(yaml, &vars).unwrap();
        assert_eq!(spec.title.as_deref(), Some("My Report"));
        assert_eq!(spec.content.len(), 2);
    }

    #[test]
    fn resolve_refs_errors_on_missing_ref() {
        let dir = tempfile::tempdir().unwrap();
        let resolver = docli_kb::KbResolver::new(dir.path());

        let yaml = r#"
content:
  - $ref: "kb://sections/nonexistent.yaml"
"#;
        let mut spec = CreateSpec::from_yaml(yaml).unwrap();
        let result = spec.resolve_refs(&resolver);
        assert!(result.is_err());
    }
}
