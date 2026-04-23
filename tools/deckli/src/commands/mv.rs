use crate::client;
use serde_json::{json, Value};

pub async fn run(
    bridge_url: &str,
    path: &str,
    to: u32,
) -> Result<Value, Box<dyn std::fmt::Display>> {
    let parts: Vec<&str> = path.trim_start_matches('/').split('/').collect();

    match parts.as_slice() {
        ["slides", idx] => {
            let from: u32 = idx
                .parse()
                .map_err(|_| Box::new(MvError(format!("invalid index: {idx}"))) as Box<dyn std::fmt::Display>)?;
            if from == 0 || to == 0 {
                return Err(Box::new(MvError("indices are 1-based".to_string())));
            }
            client::send_command(
                bridge_url,
                "move.slide",
                json!({ "fromIndex": from - 1, "toIndex": to - 1 }),
            )
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::fmt::Display>)
        }
        _ => Err(Box::new(MvError(format!(
            "unrecognized move path: {path}. Expected /slides/N"
        )))),
    }
}

#[derive(Debug)]
struct MvError(String);
impl std::fmt::Display for MvError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
