use crate::client;
use serde_json::{json, Value};
use std::path::Path;

pub async fn run(
    bridge_url: &str,
    file: Option<&Path>,
    stdin: bool,
) -> Result<Value, Box<dyn std::fmt::Display>> {
    let operations: Value = if let Some(path) = file {
        let contents = std::fs::read_to_string(path)
            .map_err(|e| Box::new(BatchError(format!("read {}: {e}", path.display()))) as Box<dyn std::fmt::Display>)?;
        serde_json::from_str(&contents)
            .map_err(|e| Box::new(BatchError(format!("invalid JSON in {}: {e}", path.display()))) as Box<dyn std::fmt::Display>)?
    } else if stdin {
        let mut input = String::new();
        std::io::Read::read_to_string(&mut std::io::stdin(), &mut input)
            .map_err(|e| Box::new(BatchError(format!("read stdin: {e}"))) as Box<dyn std::fmt::Display>)?;
        serde_json::from_str(&input)
            .map_err(|e| Box::new(BatchError(format!("invalid JSON from stdin: {e}"))) as Box<dyn std::fmt::Display>)?
    } else {
        return Err(Box::new(BatchError(
            "specify --file <path> or --stdin".to_string(),
        )));
    };

    client::send_command(bridge_url, "batch", json!({ "operations": operations }))
        .await
        .map_err(|e| Box::new(e) as Box<dyn std::fmt::Display>)
}

#[derive(Debug)]
struct BatchError(String);
impl std::fmt::Display for BatchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
