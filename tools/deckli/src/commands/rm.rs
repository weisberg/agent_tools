use crate::client;
use serde_json::{json, Value};

pub async fn run(
    bridge_url: &str,
    path: &str,
) -> Result<Value, Box<dyn std::fmt::Display>> {
    let parts: Vec<&str> = path.trim_start_matches('/').split('/').collect();

    let (method, params) = match parts.as_slice() {
        ["slides", idx] => {
            let i = parse_1based(idx)?;
            ("rm.slide", json!({ "slideIndex": i }))
        }
        ["slides", idx, "shapes", shape_id] => {
            let i = parse_1based(idx)?;
            ("rm.shape", json!({ "slideIndex": i, "shapeId": *shape_id }))
        }
        _ => {
            return Err(Box::new(RmError(format!(
                "unrecognized rm path: {path}. Expected /slides/N or /slides/N/shapes/ID"
            ))));
        }
    };

    client::send_command(bridge_url, method, params)
        .await
        .map_err(|e| Box::new(e) as Box<dyn std::fmt::Display>)
}

fn parse_1based(s: &str) -> Result<u32, Box<dyn std::fmt::Display>> {
    let n: u32 = s.parse().map_err(|_| Box::new(RmError(format!("invalid index: {s}"))) as Box<dyn std::fmt::Display>)?;
    if n == 0 {
        return Err(Box::new(RmError("index is 1-based, got 0".to_string())));
    }
    Ok(n - 1)
}

#[derive(Debug)]
struct RmError(String);
impl std::fmt::Display for RmError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
