use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

use docli_core::DocliError;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RuleMetadata {
    pub title: String,
    #[serde(default)]
    pub severity: Option<String>,
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KbRule {
    pub path: PathBuf,
    pub metadata: RuleMetadata,
    pub body: String,
}

pub fn load_rules(directory: &Path) -> Result<Vec<KbRule>, DocliError> {
    if !directory.exists() {
        return Ok(Vec::new());
    }

    let mut rules = Vec::new();
    for entry in fs::read_dir(directory).map_err(|error| DocliError::TemplateNotFound {
        template: error.to_string(),
    })? {
        let entry = entry.map_err(|error| DocliError::TemplateNotFound {
            template: error.to_string(),
        })?;
        if !entry
            .file_type()
            .map_err(|error| DocliError::TemplateNotFound {
                template: error.to_string(),
            })?
            .is_file()
        {
            continue;
        }

        let path = entry.path();
        let content = fs::read_to_string(&path).map_err(|error| DocliError::TemplateNotFound {
            template: error.to_string(),
        })?;
        let (front_matter, body) =
            split_front_matter(&content).ok_or_else(|| DocliError::InvalidSpec {
                message: format!("rule file missing YAML front matter: {}", path.display()),
            })?;
        let metadata = serde_yaml::from_str::<RuleMetadata>(front_matter).map_err(|error| {
            DocliError::InvalidSpec {
                message: error.to_string(),
            }
        })?;
        rules.push(KbRule {
            path,
            metadata,
            body: body.to_string(),
        });
    }

    rules.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(rules)
}

fn split_front_matter(content: &str) -> Option<(&str, &str)> {
    let remainder = content.strip_prefix("---\n")?;
    let end = remainder.find("\n---\n")?;
    let front_matter = &remainder[..end];
    let body = &remainder[end + 5..];
    Some((front_matter, body))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::load_rules;

    #[test]
    fn loads_markdown_rules_with_yaml_front_matter() {
        let dir = tempdir().unwrap();
        let rule = dir.path().join("check.md");
        fs::write(
            &rule,
            "---\ntitle: Example Rule\nseverity: warning\n---\nRule body",
        )
        .unwrap();

        let rules = load_rules(dir.path()).unwrap();

        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].metadata.title, "Example Rule");
        assert_eq!(rules[0].body, "Rule body");
    }
}
