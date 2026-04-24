#![forbid(unsafe_code)]

//! Schema emission helpers for XLI commands and result types.

use schemars::schema_for;
use serde_json::{json, Value};
use xli_core::{
    BatchOp, CommitMode, CommitStats, RepairSuggestion, ResponseEnvelope, Status, StyleSpec,
    XliError,
};
use xli_read::{CellData, RangeData, SheetInfo, WorkbookInfo};

pub fn emit_full_schema() -> Value {
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "title": "XLI Command Schema",
        "type": "object",
        "commands": {
            "inspect": command_schema("inspect"),
            "read": command_schema("read"),
            "write": command_schema("write"),
            "format": command_schema("format"),
            "sheet": command_schema("sheet"),
            "batch": command_schema("batch"),
            "apply": command_schema("apply"),
            "create": command_schema("create"),
            "lint": command_schema("lint"),
            "recalc": command_schema("recalc"),
            "validate": command_schema("validate"),
            "doctor": command_schema("doctor"),
            "template": command_schema("template"),
            "schema": command_schema("schema"),
        },
        "results": {
            "ResponseEnvelope": schema_for!(ResponseEnvelope<Value>).schema,
            "CommitStats": schema_for!(CommitStats).schema,
            "Status": schema_for!(Status).schema,
            "CommitMode": schema_for!(CommitMode).schema,
            "WorkbookInfo": schema_for!(WorkbookInfo).schema,
            "SheetInfo": schema_for!(SheetInfo).schema,
            "CellData": schema_for!(CellData).schema,
            "RangeData": schema_for!(RangeData).schema,
            "BatchOp": schema_for!(BatchOp).schema,
            "StyleSpec": schema_for!(StyleSpec).schema,
            "RepairSuggestion": schema_for!(RepairSuggestion).schema,
            "XliError": schema_for!(XliError).schema,
            "RecalcResult": schema_for!(xli_calc::RecalcResult).schema,
            "InspectOutput": schema_for!(WorkbookInfo).schema,
            "ReadCellOutput": schema_for!(CellData).schema,
            "ReadRangeOutput": schema_for!(RangeData).schema,
            "CreateOutput": json!({"type":"object","required":["file","sheets_created"],"properties":{"file":{"type":"string"},"sheets_created":{"type":"integer"}}}),
            "WriteOutput": json!({"type":"object","required":["written","cells","formulas_written"],"properties":{"written":{"type":"integer"},"cells":{"type":"array","items":{"type":"string"}},"formulas_written":{"type":"integer"}}}),
            "FormatOutput": json!({"type":"object","required":["cells_formatted"],"properties":{"cells_formatted":{"type":"integer"}}}),
            "BatchOutput": json!({"type":"object","required":["ops_executed","ops_failed","stats"],"properties":{"ops_executed":{"type":"integer"},"ops_failed":{"type":"integer"},"stats": schema_for!(xli_ooxml::BatchSummary).schema}}),
            "ApplyOutput": json!({"type":"object","required":["template","ops_executed","stats"],"properties":{"template":{"type":"string"},"ops_executed":{"type":"integer"},"stats": schema_for!(xli_ooxml::BatchSummary).schema}}),
            "TemplateListOutput": json!({"type":"object","required":["templates"],"properties":{"templates":{"type":"array","items": schema_for!(xli_kb::TemplateMetadata).schema}}}),
            "TemplatePreview": schema_for!(xli_kb::TemplatePreview).schema,
            "LintOutput": json!({"type":"object"}),
            "ValidateOutput": json!({"type":"object"}),
            "DoctorOutput": json!({"type":"object"}),
        }
    })
}

pub fn emit_command_schema(command: &str) -> Result<Value, XliError> {
    match command {
        "inspect" | "read" | "write" | "format" | "sheet" | "batch" | "apply" | "create"
        | "lint" | "recalc" | "validate" | "doctor" | "template" | "schema" => {
            Ok(command_schema(command))
        }
        _ => Err(XliError::TemplateParamInvalid {
            parameter: "command".to_string(),
            details: format!("Unknown command: {command}"),
        }),
    }
}

pub fn emit_openapi() -> Value {
    json!({
        "openapi": "3.0.3",
        "info": {
            "title": "XLI CLI",
            "version": "0.1.0"
        },
        "paths": {
            "/inspect": openapi_path("inspect"),
            "/read": openapi_path("read"),
            "/write": openapi_path("write"),
            "/format": openapi_path("format"),
            "/sheet": openapi_path("sheet"),
            "/batch": openapi_path("batch"),
            "/apply": openapi_path("apply"),
            "/create": openapi_path("create"),
            "/lint": openapi_path("lint"),
            "/recalc": openapi_path("recalc"),
            "/validate": openapi_path("validate"),
            "/doctor": openapi_path("doctor"),
            "/template": openapi_path("template"),
            "/schema": openapi_path("schema")
        }
    })
}

fn openapi_path(command: &str) -> Value {
    json!({
        "post": {
            "requestBody": {
                "required": true,
                "content": {
                    "application/json": {
                        "schema": command_schema(command)
                    }
                }
            },
            "responses": {
                "200": { "description": "OK" }
            }
        }
    })
}

fn command_schema(command: &str) -> Value {
    match command {
        "inspect" => {
            json!({"type":"object","required":["file"],"properties":{"file":{"type":"string"}}})
        }
        "read" => {
            json!({"type":"object","required":["file"],"properties":{"file":{"type":"string"},"address":{"type":"string"},"range":{"type":"string"},"sheet":{"type":"string"},"table":{"type":"string"},"limit":{"type":"integer"},"offset":{"type":"integer"},"headers":{"type":"boolean"},"formulas":{"type":"boolean"},"format":{"type":"string","enum":["json","markdown","json-full"]}}})
        }
        "write" => {
            json!({"type":"object","required":["file","address"],"properties":{"file":{"type":"string"},"address":{"type":"string"},"value":{},"formula":{"type":"string"},"sheet":{"type":"string"},"expect_fingerprint":{"type":"string"},"dry_run":{"type":"boolean"}}})
        }
        "format" => {
            json!({"type":"object","required":["file","range"],"properties":{"file":{"type":"string"},"range":{"type":"string"},"bold":{"type":"boolean"},"italic":{"type":"boolean"},"font_color":{"type":"string"},"fill":{"type":"string"},"number_format":{"type":"string","description":"Excel number format string or alias: currency, currency_2dp, accounting, accounting_2dp, percent, percent_int, percent_1dp, integer, standard, text, date_iso, datetime_iso"},"column_width":{"type":"number"},"dry_run":{"type":"boolean"}}})
        }
        "sheet" => {
            json!({"type":"object","required":["file","action"],"properties":{"file":{"type":"string"},"action":{"type":"string"}}})
        }
        "batch" => {
            json!({"type":"object","required":["file"],"properties":{"file":{"type":"string"},"stdin":{"type":"boolean"},"file_input":{"type":"string"},"dry_run":{"type":"boolean"}}})
        }
        "apply" => {
            json!({"type":"object","required":["file","template"],"properties":{"file":{"type":"string"},"template":{"type":"string"},"param":{"type":"array","items":{"type":"string"}},"expect_fingerprint":{"type":"string"},"dry_run":{"type":"boolean"}}})
        }
        "create" => {
            json!({"type":"object","required":["name"],"properties":{"name":{"type":"string"},"sheets":{"type":"string"},"from_csv":{"type":"string"},"from_markdown":{"type":"string"},"from_json":{"type":"string"}}})
        }
        "lint" => {
            json!({"type":"object","required":["file"],"properties":{"file":{"type":"string"},"rules":{"type":"string"},"severity":{"type":"string"}}})
        }
        "recalc" => {
            json!({"type":"object","required":["file"],"properties":{"file":{"type":"string"},"timeout":{"type":"integer"}}})
        }
        "validate" => {
            json!({"type":"object","required":["file"],"properties":{"file":{"type":"string"}}})
        }
        "doctor" => {
            json!({"type":"object","required":["file"],"properties":{"file":{"type":"string"},"skip_recalc":{"type":"boolean"},"timeout":{"type":"integer"}}})
        }
        "template" => {
            json!({"type":"object","required":["action"],"properties":{"action":{"type":"string","enum":["list","preview","validate"]},"name":{"type":"string"},"param":{"type":"array","items":{"type":"string"}}}})
        }
        "schema" => {
            json!({"type":"object","properties":{"command":{"type":"string"},"result":{"type":"string"},"openapi":{"type":"boolean"}}})
        }
        _ => json!({"type":"object"}),
    }
}
