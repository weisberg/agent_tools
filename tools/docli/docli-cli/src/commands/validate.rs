use std::path::PathBuf;

use clap::Args;
use serde::Serialize;

use docli_core::{DocliError, EnvelopeBuilder, Package};
use docli_schema::{
    check_invariants, validate_redlines, validate_structure, ValidationIssue,
};

use crate::envelope::emit;

#[derive(Args)]
pub struct ValidateArgs {
    /// Path to the DOCX file
    file: PathBuf,
    /// Apply automatic repairs
    #[arg(long)]
    repair: bool,
    /// Output file path for repaired document
    #[arg(long)]
    output: Option<PathBuf>,
}

#[derive(Serialize)]
struct ValidateData {
    file: String,
    issues: Vec<IssueInfo>,
    error_count: usize,
    warning_count: usize,
    repaired: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    output_path: Option<String>,
}

#[derive(Serialize)]
struct IssueInfo {
    code: String,
    message: String,
    severity: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    part: Option<String>,
}

impl From<&ValidationIssue> for IssueInfo {
    fn from(issue: &ValidationIssue) -> Self {
        Self {
            code: issue.code.clone(),
            message: issue.message.clone(),
            severity: format!("{:?}", issue.severity).to_lowercase(),
            part: issue.part.clone(),
        }
    }
}

pub fn run(args: ValidateArgs, format: &str, pretty: bool) -> i32 {
    let mut builder = EnvelopeBuilder::new("validate");

    let result = execute(&args, &mut builder);
    match result {
        Ok(data) => {
            let exit = if data.error_count > 0 { 1 } else { 0 };
            let envelope = builder.ok(data);
            if let Err(e) = emit(&envelope, format, pretty) {
                eprintln!("output error: {e}");
                return 1;
            }
            exit
        }
        Err(e) => {
            let envelope = builder.err::<serde_json::Value>(&e);
            let _ = emit(&envelope, format, pretty);
            1
        }
    }
}

fn execute(args: &ValidateArgs, builder: &mut EnvelopeBuilder) -> Result<ValidateData, DocliError> {
    let package = Package::open(&args.file)?;

    let mut all_issues = Vec::new();
    all_issues.extend(validate_structure(&package));
    all_issues.extend(check_invariants(&package));
    all_issues.extend(validate_redlines(&package));

    let error_count = all_issues
        .iter()
        .filter(|i| i.severity == docli_schema::ValidationSeverity::Error)
        .count();
    let warning_count = all_issues
        .iter()
        .filter(|i| i.severity == docli_schema::ValidationSeverity::Warning)
        .count();

    let mut repaired = false;
    let mut output_path = None;

    if args.repair {
        if let Some(ref out) = args.output {
            // Apply repairs to document.xml and write the repaired package
            let mut package = package;
            if let Some(doc_xml) = package.xml_parts.get("word/document.xml").cloned() {
                let repaired_xml = docli_schema::ensure_xml_space_preserve(&doc_xml)?;
                let repaired_xml = docli_schema::repair_durable_id_overflow(&repaired_xml)?;
                package
                    .xml_parts
                    .insert("word/document.xml".to_string(), repaired_xml);
            }

            // Write repaired package using zip
            write_repaired_package(&package, out)?;
            repaired = true;
            output_path = Some(out.display().to_string());
            builder.warn("repairs applied to output file");
        } else {
            builder.warn("--repair requires --output to write the repaired file");
        }
    }

    let issues: Vec<IssueInfo> = all_issues.iter().map(IssueInfo::from).collect();

    Ok(ValidateData {
        file: args.file.display().to_string(),
        issues,
        error_count,
        warning_count,
        repaired,
        output_path,
    })
}

fn write_repaired_package(package: &Package, output: &PathBuf) -> Result<(), DocliError> {
    use std::fs::File;
    use std::io::Write;
    use zip::write::SimpleFileOptions;
    use zip::ZipWriter;

    let file = File::create(output)?;
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default();

    // Write XML parts
    for (name, bytes) in &package.xml_parts {
        zip.start_file(name, options)
            .map_err(|e| DocliError::CommitFailed {
                message: e.to_string(),
            })?;
        zip.write_all(bytes)?;
    }

    // Copy binary parts from original archive
    let mut archive = package.reopen_archive()?;
    for name in &package.binary_parts {
        let mut entry = archive
            .by_name(name)
            .map_err(|e| DocliError::CommitFailed {
                message: e.to_string(),
            })?;
        zip.start_file(name, options)
            .map_err(|e| DocliError::CommitFailed {
                message: e.to_string(),
            })?;
        let mut buf = Vec::new();
        std::io::Read::read_to_end(&mut entry, &mut buf)?;
        zip.write_all(&buf)?;
    }

    zip.finish()
        .map_err(|e| DocliError::CommitFailed {
            message: e.to_string(),
        })?;
    Ok(())
}
