use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::style::StyleSpec;

/// Sheet management actions supported by batch operations.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum SheetAction {
    Add {
        name: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        after: Option<String>,
    },
    Delete {
        name: String,
    },
    Rename {
        from: String,
        to: String,
    },
    Copy {
        from: String,
        to: String,
    },
    Reorder {
        sheets: Vec<String>,
    },
    Hide {
        name: String,
    },
    Unhide {
        name: String,
    },
}

/// NDJSON micro-operations accepted by `xli batch`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum BatchOp {
    Write {
        address: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        value: Option<serde_json::Value>,
        #[serde(skip_serializing_if = "Option::is_none")]
        formula: Option<String>,
    },
    Format {
        range: String,
        #[serde(flatten)]
        style: StyleSpec,
    },
    Sheet {
        action: SheetAction,
    },
}
