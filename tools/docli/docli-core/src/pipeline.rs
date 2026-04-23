use std::{
    collections::{BTreeMap, BTreeSet},
    io::Write,
    path::{Path, PathBuf},
};

use tempfile::Builder;
use zip::{write::SimpleFileOptions, ZipWriter};

use crate::{
    commit::{commit_durable, commit_fast, commit_paranoid, Durability},
    error::DocliError,
    journal::CommitJournal,
    package::{copy_entry, Package},
};

#[derive(Clone, Debug)]
pub struct SelectorIndex {
    pub root_elements: BTreeMap<String, String>,
}

#[derive(Debug)]
pub struct PipelineContext {
    pub package: Package,
    pub selector_index: SelectorIndex,
    pub xml_parts: BTreeMap<String, Vec<u8>>,
    pub touched_parts: BTreeSet<String>,
    pub warnings: Vec<String>,
}

#[derive(Default)]
pub struct PipelineHooks<'a> {
    pub apply_ops: Option<&'a dyn Fn(&mut PipelineContext) -> Result<(), DocliError>>,
    pub validate: Option<&'a dyn Fn(&PipelineContext) -> Result<(), DocliError>>,
    pub serialize_touched_parts: Option<&'a dyn Fn(&mut PipelineContext) -> Result<(), DocliError>>,
    pub render_check: Option<&'a dyn Fn(&Path) -> Result<(), DocliError>>,
}

#[derive(Clone, Debug)]
pub struct PipelineRequest {
    pub command: String,
    pub source: PathBuf,
    pub output: PathBuf,
    pub durability: Durability,
    pub revalidate_after_write: bool,
}

#[derive(Clone, Debug)]
pub struct PipelineResult {
    pub journal: CommitJournal,
    pub warnings: Vec<String>,
    pub output: PathBuf,
}

pub fn run_noop_pipeline(request: &PipelineRequest) -> Result<PipelineResult, DocliError> {
    run_shadow_pipeline(request, PipelineHooks::default())
}

pub fn run_shadow_pipeline(
    request: &PipelineRequest,
    hooks: PipelineHooks<'_>,
) -> Result<PipelineResult, DocliError> {
    let package = Package::open(&request.source)?;
    let selector_index = build_selector_index(&package)?;
    let mut context = PipelineContext {
        xml_parts: package
            .xml_parts
            .iter()
            .map(|(path, bytes)| (path.clone(), bytes.clone()))
            .collect(),
        package,
        selector_index,
        touched_parts: BTreeSet::new(),
        warnings: Vec::new(),
    };

    if let Some(apply_ops) = hooks.apply_ops {
        apply_ops(&mut context)?;
    }

    if let Some(validate) = hooks.validate {
        validate(&context)?;
    }

    if let Some(serialize_touched_parts) = hooks.serialize_touched_parts {
        serialize_touched_parts(&mut context)?;
    }

    let shadow_path = write_shadow_package(&context, &request.output)?;

    let must_revalidate =
        request.revalidate_after_write || matches!(request.durability, Durability::Paranoid);
    if must_revalidate {
        validate_shadow_package(&shadow_path)?;
    }

    if matches!(request.durability, Durability::Paranoid) {
        if let Some(render_check) = hooks.render_check {
            render_check(&shadow_path)?;
        }
    }

    match request.durability {
        Durability::Fast => commit_fast(&shadow_path, &request.output)?,
        Durability::Durable => commit_durable(&shadow_path, &request.output)?,
        Durability::Paranoid => commit_paranoid(&shadow_path, &request.output, |path| {
            validate_shadow_package(path).map_err(|error| DocliError::RevalidationFailed {
                message: error.to_string(),
            })
        })?,
    }

    let output_hash = Package::open(&request.output)?.source_hash;
    let parts_modified = context.touched_parts.into_iter().collect::<Vec<_>>();
    let parts_unchanged = context
        .package
        .entry_count()
        .saturating_sub(parts_modified.len());

    Ok(PipelineResult {
        journal: CommitJournal {
            source_hash: context.package.source_hash,
            output_hash,
            parts_modified,
            parts_unchanged,
            durability: request.durability.as_str().to_string(),
            revalidated: must_revalidate,
        },
        warnings: context.warnings,
        output: request.output.clone(),
    })
}

fn build_selector_index(package: &Package) -> Result<SelectorIndex, DocliError> {
    let mut root_elements = BTreeMap::new();
    for (path, bytes) in &package.xml_parts {
        let xml = std::str::from_utf8(bytes).map_err(|error| DocliError::InvalidDocx {
            message: error.to_string(),
        })?;
        let document = roxmltree::Document::parse(xml)?;
        root_elements.insert(
            path.clone(),
            document.root_element().tag_name().name().to_string(),
        );
    }
    Ok(SelectorIndex { root_elements })
}

fn write_shadow_package(
    context: &PipelineContext,
    output_path: &Path,
) -> Result<PathBuf, DocliError> {
    let parent = output_path
        .parent()
        .ok_or_else(|| DocliError::CommitFailed {
            message: format!("output path has no parent: {}", output_path.display()),
        })?;
    let temp_file = Builder::new()
        .prefix("docli-shadow-")
        .suffix(".docx")
        .tempfile_in(parent)
        .map_err(|source| DocliError::CommitFailed {
            message: source.to_string(),
        })?;

    let writer = temp_file
        .reopen()
        .map_err(|source| DocliError::CommitFailed {
            message: source.to_string(),
        })?;
    let mut zip_writer = ZipWriter::new(writer);
    let mut archive = context.package.reopen_archive()?;

    for name in context.package.inventory.entries.keys() {
        let entry = archive.by_name(name)?;
        let options = SimpleFileOptions::default()
            .compression_method(entry.compression())
            .unix_permissions(0o644);
        drop(entry);

        let bytes = if let Some(updated) = context
            .touched_parts
            .contains(name)
            .then(|| context.xml_parts.get(name))
            .flatten()
        {
            updated.clone()
        } else {
            copy_entry(&mut archive, name)?
        };

        zip_writer
            .start_file(name, options)
            .map_err(|source| DocliError::CommitFailed {
                message: source.to_string(),
            })?;
        zip_writer
            .write_all(&bytes)
            .map_err(|source| DocliError::CommitFailed {
                message: source.to_string(),
            })?;
    }

    zip_writer
        .finish()
        .map_err(|source| DocliError::CommitFailed {
            message: source.to_string(),
        })?;

    let (_file, path) = temp_file.keep().map_err(|error| DocliError::CommitFailed {
        message: error.error.to_string(),
    })?;

    Ok(path)
}

fn validate_shadow_package(path: &Path) -> Result<(), DocliError> {
    Package::open(path)
        .map(|_| ())
        .map_err(|error| DocliError::RevalidationFailed {
            message: error.to_string(),
        })
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use tempfile::tempdir;

    use super::{run_noop_pipeline, PipelineRequest};
    use crate::{commit::Durability, package::Package};

    fn fixture_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/minimal.docx")
    }

    #[test]
    fn noop_pipeline_copies_docx_and_records_journal() {
        let dir = tempdir().unwrap();
        let output = dir.path().join("copied.docx");
        let request = PipelineRequest {
            command: "test".to_string(),
            source: fixture_path(),
            output: output.clone(),
            durability: Durability::Durable,
            revalidate_after_write: false,
        };

        let result = run_noop_pipeline(&request).unwrap();
        let output_package = Package::open(&output).unwrap();

        assert!(output.exists());
        assert!(result.journal.parts_modified.is_empty());
        assert_eq!(result.journal.parts_unchanged, output_package.entry_count());
        assert_eq!(result.output, output);
    }
}
