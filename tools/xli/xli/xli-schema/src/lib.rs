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
            "create": command_schema("create"),
            "lint": command_schema("lint"),
            "recalc": command_schema("recalc"),
            "validate": command_schema("validate"),
            "doctor": command_schema("doctor"),
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
        }
    })
}

pub fn emit_command_schema(command: &str) -> Result<Value, XliError> {
    match command {
        "inspect" | "read" | "write" | "format" | "sheet" | "batch" | "create" | "lint"
        | "recalc" | "validate" | "doctor" | "schema" => Ok(command_schema(command)),
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
            "/inspect": { "post": { "requestBody": command_schema("inspect"), "responses": { "200": { "description": "OK" }}}},
            "/read": { "post": { "requestBody": command_schema("read"), "responses": { "200": { "description": "OK" }}}},
            "/write": { "post": { "requestBody": command_schema("write"), "responses": { "200": { "description": "OK" }}}},
            "/format": { "post": { "requestBody": command_schema("format"), "responses": { "200": { "description": "OK" }}}},
            "/sheet": { "post": { "requestBody": command_schema("sheet"), "responses": { "200": { "description": "OK" }}}},
            "/batch": { "post": { "requestBody": command_schema("batch"), "responses": { "200": { "description": "OK" }}}},
            "/create": { "post": { "requestBody": command_schema("create"), "responses": { "200": { "description": "OK" }}}},
            "/lint": { "post": { "requestBody": command_schema("lint"), "responses": { "200": { "description": "OK" }}}},
            "/recalc": { "post": { "requestBody": command_schema("recalc"), "responses": { "200": { "description": "OK" }}}},
            "/validate": { "post": { "requestBody": command_schema("validate"), "responses": { "200": { "description": "OK" }}}},
            "/doctor": { "post": { "requestBody": command_schema("doctor"), "responses": { "200": { "description": "OK" }}}},
            "/schema": { "post": { "requestBody": command_schema("schema"), "responses": { "200": { "description": "OK" }}}}
        }
    })
}

fn command_schema(command: &str) -> Value {
    match command {
        "inspect" => {
            json!({"type":"object","required":["file"],"properties":{"file":{"type":"string"}}})
        }
        "read" => {
            json!({"type":"object","required":["file"],"properties":{"file":{"type":"string"},"address":{"type":"string"},"range":{"type":"string"},"table":{"type":"string"},"limit":{"type":"integer"},"offset":{"type":"integer"},"headers":{"type":"boolean"},"formulas":{"type":"boolean"}}})
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
        "create" => {
            json!({"type":"object","required":["name"],"properties":{"name":{"type":"string"},"sheets":{"type":"string"},"from_csv":{"type":"string"}}})
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
        "schema" => {
            json!({"type":"object","properties":{"command":{"type":"string"},"openapi":{"type":"boolean"}}})
        }
        _ => json!({"type":"object"}),
    }
}
