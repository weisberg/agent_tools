use chrono::Utc;
use serde_json::{json, Value};

use crate::error::NotionliError;
use crate::util::{looks_like_date, unquote};

pub(crate) fn compile_where(expr: &str) -> Result<Value, NotionliError> {
    let parts = expr.split(" and ").collect::<Vec<_>>();
    if parts.len() > 1 {
        return Ok(
            json!({ "and": parts.into_iter().map(compile_single_condition).collect::<Result<Vec<_>, _>>()? }),
        );
    }
    compile_single_condition(expr)
}

pub(crate) fn compile_single_condition(expr: &str) -> Result<Value, NotionliError> {
    for op in ["!=", ">=", "<=", "=", ">", "<"] {
        if let Some((left, right)) = expr.split_once(op) {
            return compile_property_condition(left.trim(), op, &unquote(right.trim()));
        }
    }
    Err(NotionliError::Validation {
        message: format!("Unsupported where expression: {expr}"),
    })
}

pub(crate) fn compile_property_condition(
    prop: &str,
    op: &str,
    value: &str,
) -> Result<Value, NotionliError> {
    let value = if value == "today" {
        Utc::now().date_naive().to_string()
    } else {
        value.to_string()
    };
    if looks_like_date(&value) {
        let comparator = match op {
            "=" => "equals",
            "!=" => "does_not_equal",
            "<=" => "on_or_before",
            ">=" => "on_or_after",
            "<" => "before",
            ">" => "after",
            _ => "equals",
        };
        return Ok(json!({ "property": prop, "date": { comparator: value } }));
    }
    if let Ok(number) = value.parse::<f64>() {
        let comparator = match op {
            "=" => "equals",
            "!=" => "does_not_equal",
            "<=" => "less_than_or_equal_to",
            ">=" => "greater_than_or_equal_to",
            "<" => "less_than",
            ">" => "greater_than",
            _ => "equals",
        };
        return Ok(json!({ "property": prop, "number": { comparator: number } }));
    }
    let comparator = match op {
        "=" => "equals",
        "!=" => "does_not_equal",
        _ => {
            return Err(NotionliError::Validation {
                message: format!("Operator {op} only supports date/number comparisons."),
            })
        }
    };
    Ok(json!({ "property": prop, "select": { comparator: value } }))
}

pub(crate) fn compile_sort(expr: &str) -> Value {
    Value::Array(
        expr.split(',')
            .map(|part| {
                let mut words = part.split_whitespace();
                let property = words.next().unwrap_or("").to_string();
                let direction = match words.next().unwrap_or("asc").to_lowercase().as_str() {
                    "desc" | "descending" => "descending",
                    _ => "ascending",
                };
                json!({ "property": property, "direction": direction })
            })
            .collect(),
    )
}
