use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::*;

pub(crate) fn run_recipe(cmd: RecipeCommand) -> Result<Outcome, MdliError> {
    match cmd {
        RecipeCommand::Validate(args) => {
            let recipe = load_recipe(&args.recipe)?;
            recipe.validate_schema()?;
            Ok(Outcome::Json(json!({
                "schema": recipe.schema,
                "title": recipe.title,
                "sections": recipe.sections.iter().map(|s| json!({
                    "id": s.id,
                    "path": s.path,
                    "level": s.level,
                    "after": s.after,
                    "before": s.before,
                    "template": s.template,
                    "bindings": s.bindings,
                })).collect::<Vec<_>>(),
            })))
        }
    }
}

pub(crate) fn run_apply(args: ApplyArgs) -> Result<Outcome, MdliError> {
    let mut doc = MarkdownDocument::read(&args.file)?;
    doc.assert_preimage(&args.mutate.preimage_hash)?;
    validate_write_emit(&args.mutate)?;
    let recipe = load_recipe(&args.recipe)?;
    recipe.validate_schema()?;
    let datasets = parse_data_args(&args.data)?;
    let recipe_dir = args
        .recipe
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let recipe_hash = sha256_prefixed(&fs::read(&args.recipe).unwrap_or_default());

    let before = doc.render();
    let mut ops = Vec::new();
    apply_recipe(
        &mut doc,
        &recipe,
        &datasets,
        &recipe_dir,
        &recipe_hash,
        &mut ops,
    )?;
    let changed = before != doc.render();
    Ok(Outcome::Mutated(MutationOutcome {
        document: doc,
        changed,
        ops,
        warnings: Vec::new(),
        flags: args.mutate,
    }))
}

pub(crate) fn run_build(args: BuildArgs) -> Result<Outcome, MdliError> {
    if args.out.exists() && !args.overwrite {
        return Err(MdliError::user(
            "E_WRITE_FAILED",
            format!(
                "{} already exists; pass --overwrite to replace",
                args.out.display()
            ),
        ));
    }
    let recipe = load_recipe(&args.recipe)?;
    recipe.validate_schema()?;
    let datasets = parse_data_args(&args.data)?;
    let recipe_dir = args
        .recipe
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let recipe_hash = sha256_prefixed(&fs::read(&args.recipe).unwrap_or_default());

    let mut starter = String::new();
    if let Some(fm) = &recipe.frontmatter {
        starter.push_str("---\n");
        for (k, v) in fm {
            starter.push_str(&format!("{k}: {}\n", scalar_yaml_string(v)));
        }
        starter.push_str("---\n\n");
    }
    if let Some(title) = &recipe.title {
        starter.push_str(&format!("# {title}\n"));
    }
    let mut doc = MarkdownDocument::from_bytes(Some(args.out.clone()), starter.into_bytes())?;
    let mut ops = Vec::new();
    apply_recipe(
        &mut doc,
        &recipe,
        &datasets,
        &recipe_dir,
        &recipe_hash,
        &mut ops,
    )?;
    fs::write(&args.out, doc.render()).map_err(|e| {
        MdliError::io(
            "E_WRITE_FAILED",
            format!("failed to write {}", args.out.display()),
            e,
        )
    })?;
    Ok(Outcome::Json(json!({
        "out": args.out.display().to_string(),
        "ops": ops,
        "postimage_hash": sha256_prefixed(doc.render().as_bytes()),
    })))
}

fn scalar_yaml_string(value: &Value) -> String {
    match value {
        Value::String(s) => format!("\"{}\"", s.replace('"', "\\\"")),
        Value::Null => "null".to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        other => serde_json::to_string(other).unwrap_or_default(),
    }
}

pub(crate) fn apply_recipe(
    doc: &mut MarkdownDocument,
    recipe: &Recipe,
    datasets: &BTreeMap<String, Value>,
    recipe_dir: &Path,
    recipe_hash: &str,
    ops: &mut Vec<Value>,
) -> Result<(), MdliError> {
    // Optional frontmatter sync for an in-place apply: write keys if not present.
    if let Some(fm) = &recipe.frontmatter {
        for (key, value) in fm {
            let map = parse_frontmatter_map(&doc.lines)?;
            if !map.contains_key(key) {
                set_frontmatter_key(doc, key, Some(value_to_yaml_scalar(value)));
                ops.push(json!({"op": "set_frontmatter", "key": key}));
            }
        }
    }
    for section in &recipe.sections {
        apply_recipe_section(doc, recipe, section, datasets, recipe_dir, recipe_hash, ops)?;
    }
    Ok(())
}

fn apply_recipe_section(
    doc: &mut MarkdownDocument,
    recipe: &Recipe,
    section: &RecipeSection,
    datasets: &BTreeMap<String, Value>,
    recipe_dir: &Path,
    recipe_hash: &str,
    ops: &mut Vec<Value>,
) -> Result<(), MdliError> {
    validate_id(&section.id)?;
    if !(1..=6).contains(&section.level) {
        return Err(MdliError::user(
            "E_RECIPE_INVALID",
            format!("section {} level must be 1..=6", section.id),
        ));
    }

    let index = index_document(doc);
    let exists_by_id = index
        .sections
        .iter()
        .any(|s| s.id.as_deref() == Some(section.id.as_str()));
    if !exists_by_id {
        let title = section
            .path
            .split('>')
            .next_back()
            .map(|s| s.trim().replace("\\>", ">"))
            .filter(|s| !s.is_empty())
            .ok_or_else(|| {
                MdliError::user(
                    "E_RECIPE_INVALID",
                    format!("section {} path is empty", section.id),
                )
            })?;
        if let Ok(found) = resolve_section(&index, &section.path) {
            doc.lines.insert(found.heading, id_marker(&section.id));
            ops.push(json!({
                "op": "assign_id",
                "id": section.id,
                "path": section.path,
            }));
        } else {
            let insert_at = if let Some(after) = section.after.as_deref() {
                resolve_section(&index, after)?.end
            } else if let Some(before) = section.before.as_deref() {
                resolve_section(&index, before)?.start
            } else if let Some(root) = index.sections.iter().find(|s| s.level == 1) {
                root.end
            } else {
                doc.lines.len()
            };
            let mut insertion = Vec::new();
            if insert_at > 0
                && doc
                    .lines
                    .get(insert_at - 1)
                    .map(|l| !l.trim().is_empty())
                    .unwrap_or(false)
            {
                insertion.push(String::new());
            }
            insertion.push(id_marker(&section.id));
            insertion.push(format!("{} {}", "#".repeat(section.level), title));
            insertion.push(String::new());
            doc.lines.splice(insert_at..insert_at, insertion);
            doc.trailing_newline = true;
            ops.push(json!({
                "op": "ensure_section",
                "id": section.id,
                "path": section.path,
                "level": section.level,
                "after_id": section.after,
                "before_id": section.before,
            }));
        }
    }

    let template_path = recipe_dir.join(&section.template);
    if !template_path.exists() {
        return Err(MdliError::user(
            "E_RECIPE_INVALID",
            format!(
                "template {} not found for section {}",
                template_path.display(),
                section.id
            ),
        ));
    }
    let template_text = fs::read_to_string(&template_path).map_err(|e| {
        MdliError::io(
            "E_READ_FAILED",
            format!("failed to read template {}", template_path.display()),
            e,
        )
    })?;

    let mut bound: BTreeMap<String, Value> = BTreeMap::new();
    if !section.bindings.is_empty() {
        for (alias, source) in &section.bindings {
            let value = datasets.get(source).ok_or_else(|| {
                MdliError::user(
                    "E_TEMPLATE_MISSING_DATASET",
                    format!(
                        "section {} binding {} -> {} not found",
                        section.id, alias, source
                    ),
                )
            })?;
            bound.insert(alias.clone(), value.clone());
        }
    } else {
        for (k, v) in datasets {
            bound.insert(k.clone(), v.clone());
        }
    }

    let rendered = render_template(&template_text, &bound)?;
    let block_id = format!(
        "{}.{}",
        section.id,
        recipe
            .settings
            .as_ref()
            .and_then(|s| s.generated_block_suffix.clone())
            .unwrap_or_else(|| "generated".to_string())
    );
    let body_lines = split_body_lines(&rendered);
    upsert_recipe_block(
        doc,
        &section.id,
        &block_id,
        body_lines.clone(),
        recipe,
        recipe_hash,
    )?;
    ops.push(json!({
        "op": "replace_block",
        "id": block_id,
        "section": section.id,
        "parent_section": section.id,
        "text": rendered,
    }));
    Ok(())
}

fn upsert_recipe_block(
    doc: &mut MarkdownDocument,
    section_id: &str,
    block_id: &str,
    body: Vec<String>,
    recipe: &Recipe,
    recipe_hash: &str,
) -> Result<(), MdliError> {
    let index = index_document(doc);
    if index.blocks.iter().any(|b| b.id == block_id) {
        let on_modified = match recipe
            .settings
            .as_ref()
            .and_then(|s| s.on_modified.as_deref())
        {
            Some("force") => OnModified::Force,
            Some("three-way") => OnModified::ThreeWay,
            _ => OnModified::Fail,
        };
        replace_block_with_provenance(doc, block_id, body, &on_modified, Some(recipe_hash))?;
        return Ok(());
    }
    let section = resolve_section(&index, section_id)?;
    let lines = render_block_lines_with_provenance(block_id, body, false, Some(recipe_hash));
    let insert_at = section.end;
    let mut insertion = Vec::new();
    if insert_at > 0
        && doc
            .lines
            .get(insert_at - 1)
            .map(|l| !l.trim().is_empty())
            .unwrap_or(false)
    {
        insertion.push(String::new());
    }
    insertion.extend(lines);
    insertion.push(String::new());
    doc.lines.splice(insert_at..insert_at, insertion);
    Ok(())
}

pub(crate) fn replace_block_with_provenance(
    doc: &mut MarkdownDocument,
    block_id: &str,
    body: Vec<String>,
    on_modified: &OnModified,
    recipe_hash: Option<&str>,
) -> Result<(), MdliError> {
    let index = index_document(doc);
    let block = resolve_block(&index, block_id)?;
    if block.locked {
        return Err(MdliError::invariant(
            "E_BLOCK_LOCKED",
            format!("block {block_id} is locked"),
        ));
    }
    if let Some(expected) = &block.checksum {
        let actual = checksum_body(&doc.lines[block.start + 1..block.end - 1]);
        if expected != &actual {
            handle_block_conflict(
                doc,
                block_id,
                expected,
                &actual,
                &doc.lines[block.start + 1..block.end - 1].to_vec(),
                &body,
                on_modified,
            )?;
        }
    }
    let rendered = render_block_lines_with_provenance(block_id, body, false, recipe_hash);
    doc.lines.splice(block.start..block.end, rendered);
    Ok(())
}

pub(crate) fn render_block_lines_with_provenance(
    block_id: &str,
    body: Vec<String>,
    locked: bool,
    recipe_hash: Option<&str>,
) -> Vec<String> {
    let checksum = checksum_body(&body);
    let mut lines = Vec::new();
    let mut fields = std::collections::BTreeMap::new();
    fields.insert("v".to_string(), MARKER_VERSION.to_string());
    fields.insert("id".to_string(), block_id.to_string());
    if let Some(hash) = recipe_hash {
        fields.insert("recipe".to_string(), hash.to_string());
    }
    fields.insert("checksum".to_string(), checksum);
    if locked {
        fields.insert("locked".to_string(), "true".to_string());
    }
    lines.push(render_marker("begin", &fields));
    lines.extend(body);
    lines.push(end_marker(block_id));
    lines
}

fn value_to_yaml_scalar(value: &Value) -> String {
    match value {
        Value::Null => "null".to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        Value::String(s) => format!("\"{}\"", s.replace('"', "\\\"")),
        other => serde_json::to_string(other).unwrap_or_default(),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Recipe {
    #[serde(default)]
    pub(crate) schema: String,
    #[serde(default)]
    pub(crate) title: Option<String>,
    #[serde(default)]
    pub(crate) frontmatter: Option<BTreeMap<String, Value>>,
    #[serde(default)]
    pub(crate) settings: Option<RecipeSettings>,
    #[serde(default)]
    pub(crate) sections: Vec<RecipeSection>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RecipeSettings {
    #[serde(default)]
    pub(crate) on_modified: Option<String>,
    #[serde(default)]
    pub(crate) on_missing_dataset: Option<String>,
    #[serde(default)]
    pub(crate) generated_block_suffix: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RecipeSection {
    pub(crate) id: String,
    pub(crate) path: String,
    pub(crate) level: usize,
    #[serde(default)]
    pub(crate) after: Option<String>,
    #[serde(default)]
    pub(crate) before: Option<String>,
    pub(crate) template: String,
    #[serde(default)]
    pub(crate) bindings: BTreeMap<String, String>,
}

impl Recipe {
    pub(crate) fn validate_schema(&self) -> Result<(), MdliError> {
        if self.schema != "mdli/recipe/v1" {
            return Err(MdliError::user(
                "E_RECIPE_INVALID",
                format!("expected schema mdli/recipe/v1, got {}", self.schema),
            ));
        }
        if self.sections.is_empty() {
            return Err(MdliError::user(
                "E_RECIPE_INVALID",
                "recipe defines no sections",
            ));
        }
        let mut seen = std::collections::BTreeSet::new();
        for section in &self.sections {
            validate_id(&section.id)
                .map_err(|_| MdliError::user("E_RECIPE_INVALID", "section id is invalid"))?;
            if !seen.insert(section.id.clone()) {
                return Err(MdliError::user(
                    "E_RECIPE_INVALID",
                    format!("duplicate section id {}", section.id),
                ));
            }
            if section.template.trim().is_empty() {
                return Err(MdliError::user(
                    "E_RECIPE_INVALID",
                    format!("section {} has an empty template path", section.id),
                ));
            }
        }
        Ok(())
    }
}

pub(crate) fn load_recipe(path: &Path) -> Result<Recipe, MdliError> {
    let text = fs::read_to_string(path).map_err(|e| {
        MdliError::io(
            "E_READ_FAILED",
            format!("failed to read recipe {}", path.display()),
            e,
        )
    })?;
    let trimmed = text.trim_start();
    let parsed: Recipe = if trimmed.starts_with('{') {
        serde_json::from_str(&text)
            .map_err(|e| MdliError::user("E_RECIPE_INVALID", format!("invalid JSON recipe: {e}")))?
    } else {
        serde_yaml::from_str(&text)
            .map_err(|e| MdliError::user("E_RECIPE_INVALID", format!("invalid YAML recipe: {e}")))?
    };
    Ok(parsed)
}
