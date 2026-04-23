/// WebSocket client for communicating with the deckli bridge sidecar.
use crate::protocol::{Request, Response};
use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use tokio_tungstenite::{connect_async, tungstenite::Message};

#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    #[error("failed to connect to bridge at {0}: {1}")]
    Connect(String, String),
    #[error("bridge connection closed unexpectedly")]
    Closed,
    #[error("failed to send command: {0}")]
    Send(String),
    #[error("invalid response from bridge: {0}")]
    InvalidResponse(String),
    #[error("command failed: {0}")]
    CommandError(String),
}

/// Send a single command to the bridge and return the result.
pub async fn send_command(
    bridge_url: &str,
    method: &str,
    params: Value,
) -> Result<Value, ClientError> {
    let url = format!("{}/cli", bridge_url);
    let (ws_stream, _) = connect_async(&url)
        .await
        .map_err(|e| ClientError::Connect(url.clone(), e.to_string()))?;

    let (mut write, mut read) = ws_stream.split();

    let request = Request::new(method, params);
    let payload = serde_json::to_string(&request)
        .map_err(|e| ClientError::Send(e.to_string()))?;

    write
        .send(Message::Text(payload))
        .await
        .map_err(|e| ClientError::Send(e.to_string()))?;

    // Wait for the response
    while let Some(msg) = read.next().await {
        match msg {
            Ok(Message::Text(text)) => {
                let response: Response = serde_json::from_str(&text)
                    .map_err(|e| ClientError::InvalidResponse(e.to_string()))?;

                if response.success {
                    return Ok(response.result.unwrap_or(Value::Null));
                } else {
                    let err = response.error.unwrap_or(crate::protocol::ErrorPayload {
                        code: "unknown".to_string(),
                        message: "unknown error".to_string(),
                        suggestion: None,
                    });
                    return Err(ClientError::CommandError(err.message));
                }
            }
            Ok(Message::Close(_)) => return Err(ClientError::Closed),
            Err(e) => return Err(ClientError::InvalidResponse(e.to_string())),
            _ => continue,
        }
    }

    Err(ClientError::Closed)
}

/// Check if the bridge is reachable.
pub async fn ping(bridge_url: &str) -> bool {
    send_command(bridge_url, "ping", Value::Null).await.is_ok()
}
