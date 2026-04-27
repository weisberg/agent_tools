use std::process;

use serde_json::{json, Value};

use crate::*;

pub(crate) enum Outcome {
    Text(String),
    Json(Value),
    Mutated(MutationOutcome),
}

#[derive(Debug)]
pub(crate) struct MutationOutcome {
    pub(crate) document: MarkdownDocument,
    pub(crate) changed: bool,
    pub(crate) ops: Vec<Value>,
    pub(crate) warnings: Vec<Value>,
    pub(crate) flags: MutateArgs,
}

pub(crate) fn emit_success(outcome: Outcome, force_json: bool) {
    match outcome {
        Outcome::Text(text) if force_json => print_json(&json!({
            "schema": OUTPUT_SCHEMA,
            "ok": true,
            "result": {"text": text}
        })),
        Outcome::Text(text) => {
            print!("{text}");
            if !text.ends_with('\n') {
                println!();
            }
        }
        Outcome::Json(result) => print_json(&json!({
            "schema": OUTPUT_SCHEMA,
            "ok": true,
            "result": result
        })),
        Outcome::Mutated(mutation) => emit_mutation(mutation, force_json),
    }
}

pub(crate) fn emit_error(err: &MdliError, force_json: bool) {
    if force_json {
        let mut payload = json!({
            "code": err.code(),
            "message": err.message()
        });
        if let (Some(details), Some(obj)) = (err.details(), payload.as_object_mut()) {
            obj.insert("details".to_string(), details.clone());
        }
        print_json(&json!({
            "schema": OUTPUT_SCHEMA,
            "ok": false,
            "error": payload,
        }));
    } else {
        eprintln!("{}: {}", err.code(), err.message());
        if let Some(details) = err.details() {
            if let Ok(serialized) = serde_json::to_string_pretty(details) {
                eprintln!("{}: details {}", err.code(), serialized);
            }
        }
    }
}

pub(crate) fn emit_mutation(mutation: MutationOutcome, force_json: bool) {
    let result = mutation.result_json();
    let emit_json = force_json || mutation.flags.emit == EmitMode::Json;

    if mutation.flags.write {
        if let Err(err) = mutation.document.write_atomic() {
            emit_error(&err, emit_json);
            process::exit(err.exit_code());
        }
        if emit_json {
            print_json(&json!({"schema": OUTPUT_SCHEMA, "ok": true, "result": result}));
        } else {
            eprintln!(
                "{}",
                if mutation.changed {
                    "mdli: wrote updated document"
                } else {
                    "mdli: no changes"
                }
            );
        }
        return;
    }

    match mutation.flags.emit {
        EmitMode::Document if force_json => print_json(&json!({
            "schema": OUTPUT_SCHEMA,
            "ok": true,
            "result": {"document": mutation.document.render()}
        })),
        EmitMode::Document => print!("{}", mutation.document.render()),
        EmitMode::Plan | EmitMode::Json => print_json(&json!({
            "schema": OUTPUT_SCHEMA,
            "ok": true,
            "result": result
        })),
    }
}

impl MutationOutcome {
    pub(crate) fn result_json(&self) -> Value {
        json!({
            "changed": self.changed,
            "preimage_hash": self.document.preimage_hash,
            "postimage_hash": sha256_prefixed(self.document.render().as_bytes()),
            "ops": self.ops,
            "warnings": self.warnings,
        })
    }
}

pub(crate) fn print_json(value: &Value) {
    println!(
        "{}",
        serde_json::to_string_pretty(value).unwrap_or_else(|_| "{\"ok\":false}".to_string())
    );
}
