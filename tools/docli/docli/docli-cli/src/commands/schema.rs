use clap::Args;
use schemars::schema_for;
use serde::Serialize;

use docli_core::{EnvelopeBuilder, Job, Operation, Target};

use crate::envelope::emit;

#[derive(Args)]
pub struct SchemaArgs {
    /// Type name to print schema for: Job, Operation, Target (default: all)
    #[arg(long, rename_all = "verbatim")]
    r#type: Option<String>,
}

#[derive(Serialize)]
struct SchemaData {
    schemas: Vec<TypeSchema>,
}

#[derive(Serialize)]
struct TypeSchema {
    name: String,
    schema: serde_json::Value,
}

pub fn run(args: SchemaArgs, format: &str, pretty: bool) -> i32 {
    let builder = EnvelopeBuilder::new("schema");

    let type_name = args.r#type.as_deref();
    let mut schemas = Vec::new();

    let all = type_name.is_none();

    if all || type_name == Some("Job") {
        let s = schema_for!(Job);
        schemas.push(TypeSchema {
            name: "Job".to_string(),
            schema: serde_json::to_value(s).unwrap_or_default(),
        });
    }
    if all || type_name == Some("Operation") {
        let s = schema_for!(Operation);
        schemas.push(TypeSchema {
            name: "Operation".to_string(),
            schema: serde_json::to_value(s).unwrap_or_default(),
        });
    }
    if all || type_name == Some("Target") {
        let s = schema_for!(Target);
        schemas.push(TypeSchema {
            name: "Target".to_string(),
            schema: serde_json::to_value(s).unwrap_or_default(),
        });
    }

    if schemas.is_empty() {
        eprintln!(
            "unknown type: {}. Available: Job, Operation, Target",
            type_name.unwrap_or("(none)")
        );
        return 1;
    }

    let data = SchemaData { schemas };
    let envelope = builder.ok(data);
    if let Err(e) = emit(&envelope, format, pretty) {
        eprintln!("output error: {e}");
        return 1;
    }
    0
}
