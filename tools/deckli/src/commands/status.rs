use crate::client;
use serde_json::{json, Value};

pub async fn run(bridge_url: &str) -> Result<Value, Box<dyn std::fmt::Display>> {
    let port = extract_port(bridge_url);
    let bridge_up = crate::sidecar::is_bridge_running(port);

    if !bridge_up {
        return Ok(json!({
            "bridge": "disconnected",
            "addin": "unknown",
            "message": "Sidecar bridge is not running. Run `deckli connect` to start."
        }));
    }

    match client::send_command(bridge_url, "ping", Value::Null).await {
        Ok(_) => Ok(json!({
            "bridge": "connected",
            "addin": "connected",
            "message": "Ready"
        })),
        Err(_) => Ok(json!({
            "bridge": "connected",
            "addin": "disconnected",
            "message": "Bridge running but add-in not connected. Open PowerPoint and activate deckli."
        })),
    }
}

fn extract_port(url: &str) -> u16 {
    url.rsplit(':')
        .next()
        .and_then(|s| s.trim_end_matches('/').parse().ok())
        .unwrap_or(9716)
}
