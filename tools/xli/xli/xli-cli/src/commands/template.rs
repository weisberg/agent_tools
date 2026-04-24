use anyhow::Result;
use clap::{Args, Subcommand};
use schemars::JsonSchema;
use serde::Serialize;
use std::collections::BTreeMap;

use crate::output;

#[derive(Debug, Args)]
pub struct TemplateArgs {
    #[command(subcommand)]
    pub action: TemplateAction,
}

#[derive(Debug, Subcommand)]
pub enum TemplateAction {
    List,
    Preview(TemplatePreviewArgs),
    Validate(TemplateValidateArgs),
}

#[derive(Debug, Args)]
pub struct TemplatePreviewArgs {
    pub name: String,
    #[arg(long = "param")]
    pub params: Vec<String>,
}

#[derive(Debug, Args)]
pub struct TemplateValidateArgs {
    pub name: String,
    #[arg(long = "param")]
    pub params: Vec<String>,
}

#[derive(Clone, Debug, Serialize, JsonSchema)]
struct TemplateListOutput {
    templates: Vec<xli_kb::TemplateMetadata>,
}

pub fn run(args: TemplateArgs, human: bool) -> Result<bool> {
    match args.action {
        TemplateAction::List => output::emit(
            &output::ok_envelope(
                "template",
                serde_json::json!({ "action": "list" }),
                TemplateListOutput {
                    templates: xli_kb::list_templates(),
                },
                Vec::new(),
                false,
                xli_core::CommitMode::None,
                None,
                None,
                xli_core::CommitStats::default(),
            ),
            human,
        ),
        TemplateAction::Preview(preview) => {
            let params = parse_params(&preview.params);
            match xli_kb::preview_template(&preview.name, &params) {
                Ok(result) => output::emit(
                    &output::ok_envelope(
                        "template",
                        serde_json::json!({
                            "action": "preview",
                            "name": preview.name,
                            "params": params,
                        }),
                        result,
                        Vec::new(),
                        false,
                        xli_core::CommitMode::None,
                        None,
                        None,
                        xli_core::CommitStats::default(),
                    ),
                    human,
                ),
                Err(error) => output::emit(
                    &output::error_envelope::<xli_kb::TemplatePreview>(
                        "template",
                        Some(serde_json::json!({ "action": "preview", "name": preview.name })),
                        error,
                    ),
                    human,
                ),
            }
        }
        TemplateAction::Validate(validate) => {
            let params = parse_params(&validate.params);
            match xli_kb::validate_template(&validate.name, &params) {
                Ok(()) => output::emit(
                    &output::ok_envelope(
                        "template",
                        serde_json::json!({
                            "action": "validate",
                            "name": validate.name,
                            "params": params,
                        }),
                        serde_json::json!({ "valid": true }),
                        Vec::new(),
                        false,
                        xli_core::CommitMode::None,
                        None,
                        None,
                        xli_core::CommitStats::default(),
                    ),
                    human,
                ),
                Err(error) => output::emit(
                    &output::error_envelope::<serde_json::Value>(
                        "template",
                        Some(serde_json::json!({ "action": "validate", "name": validate.name })),
                        error,
                    ),
                    human,
                ),
            }
        }
    }
}

pub fn parse_params(values: &[String]) -> BTreeMap<String, String> {
    values
        .iter()
        .filter_map(|value| {
            value
                .split_once('=')
                .map(|(key, val)| (key.to_string(), val.to_string()))
        })
        .collect()
}
