use crate::client;
use serde_json::{json, Value};

/// Parse resource paths like /slides/3/shapes/2 into method + params.
pub async fn run(
    bridge_url: &str,
    path: &str,
) -> Result<Value, Box<dyn std::fmt::Display>> {
    let (method, params) = parse_path(path)?;

    client::send_command(bridge_url, &method, params)
        .await
        .map_err(|e| Box::new(e) as Box<dyn std::fmt::Display>)
}

fn parse_path(path: &str) -> Result<(String, Value), Box<dyn std::fmt::Display>> {
    let parts: Vec<&str> = path.trim_start_matches('/').split('/').collect();

    match parts.as_slice() {
        ["slides"] => Ok(("get.slides".to_string(), Value::Null)),
        ["slides", idx] => {
            let i = parse_index(idx)?;
            Ok(("get.slide".to_string(), json!({ "slideIndex": i })))
        }
        ["slides", idx, "shapes"] => {
            let i = parse_index(idx)?;
            Ok(("get.shapes".to_string(), json!({ "slideIndex": i })))
        }
        ["slides", idx, "shapes", shape_id] => {
            let i = parse_index(idx)?;
            Ok(("get.shape".to_string(), json!({ "slideIndex": i, "shapeId": *shape_id })))
        }
        ["slides", idx, "notes"] => {
            let i = parse_index(idx)?;
            Ok(("get.notes".to_string(), json!({ "slideIndex": i })))
        }
        ["selection"] => Ok(("get.selection".to_string(), Value::Null)),
        _ => Err(Box::new(PathError(format!(
            "unrecognized path: {path}. Expected /slides, /slides/N, /slides/N/shapes/ID, /selection"
        )))),
    }
}

fn parse_index(s: &str) -> Result<u32, Box<dyn std::fmt::Display>> {
    let n: u32 = s
        .parse()
        .map_err(|_| Box::new(PathError(format!("invalid index: {s}"))) as Box<dyn std::fmt::Display>)?;
    if n == 0 {
        return Err(Box::new(PathError("slide index is 1-based, got 0".to_string())));
    }
    Ok(n - 1)
}

#[derive(Debug)]
struct PathError(String);
impl std::fmt::Display for PathError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
