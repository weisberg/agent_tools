use serde_json::{json, Value};

pub async fn run(bridge_url: &str) -> Result<Value, Box<dyn std::fmt::Display>> {
    let port = 9716u16; // TODO: extract from bridge_url

    if !crate::sidecar::is_bridge_running(port) {
        tracing::info!("Starting sidecar bridge...");
        crate::sidecar::start_bridge().map_err(|e| Box::new(e) as Box<dyn std::fmt::Display>)?;

        // Give the sidecar a moment to bind
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;

        if !crate::sidecar::is_bridge_running(port) {
            return Ok(json!({
                "bridge": "failed",
                "message": "Sidecar did not start. Check `node` is installed and deckli-bridge.mjs is accessible."
            }));
        }
    }

    // Ping to check add-in
    match crate::client::send_command(bridge_url, "ping", Value::Null).await {
        Ok(_) => Ok(json!({
            "bridge": "connected",
            "addin": "connected",
            "message": "Ready — bridge and add-in connected"
        })),
        Err(_) => Ok(json!({
            "bridge": "connected",
            "addin": "disconnected",
            "message": "Bridge running. Open PowerPoint and activate the deckli add-in."
        })),
    }
}
