use crate::client;
use serde_json::{json, Value};
use std::path::Path;

pub async fn run(
    bridge_url: &str,
    slide: Option<u32>,
    all: bool,
    out: Option<&Path>,
) -> Result<Value, Box<dyn std::fmt::Display>> {
    if all {
        return render_all(bridge_url, out).await;
    }

    let slide_index = slide.unwrap_or(1);
    if slide_index == 0 {
        return Err(Box::new(RenderError("slide index is 1-based".to_string())));
    }

    let result = client::send_command(
        bridge_url,
        "render.slide",
        json!({ "slideIndex": slide_index - 1 }),
    )
    .await
    .map_err(|e| Box::new(e) as Box<dyn std::fmt::Display>)?;

    // If --out specified, decode base64 and write to file
    if let Some(path) = out {
        if let Some(b64) = result.get("image_base64").and_then(|v| v.as_str()) {
            use base64::Engine;
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(b64)
                .map_err(|e| Box::new(RenderError(format!("base64 decode: {e}"))) as Box<dyn std::fmt::Display>)?;
            std::fs::write(path, &bytes)
                .map_err(|e| Box::new(RenderError(format!("write {}: {e}", path.display()))) as Box<dyn std::fmt::Display>)?;
            return Ok(json!({
                "slideIndex": slide_index,
                "saved": path.display().to_string(),
                "bytes": bytes.len(),
            }));
        }
    }

    Ok(result)
}

async fn render_all(
    bridge_url: &str,
    out: Option<&Path>,
) -> Result<Value, Box<dyn std::fmt::Display>> {
    // First get slide count
    let info = client::send_command(bridge_url, "inspect", Value::Null)
        .await
        .map_err(|e| Box::new(e) as Box<dyn std::fmt::Display>)?;

    let count = info
        .get("slideCount")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u32;

    let mut results = Vec::new();
    for i in 0..count {
        let result = client::send_command(
            bridge_url,
            "render.slide",
            json!({ "slideIndex": i }),
        )
        .await
        .map_err(|e| Box::new(e) as Box<dyn std::fmt::Display>)?;

        if let Some(dir) = out {
            if let Some(b64) = result.get("image_base64").and_then(|v| v.as_str()) {
                use base64::Engine;
                let bytes = base64::engine::general_purpose::STANDARD
                    .decode(b64)
                    .map_err(|e| Box::new(RenderError(format!("base64 decode: {e}"))) as Box<dyn std::fmt::Display>)?;
                std::fs::create_dir_all(dir)
                    .map_err(|e| Box::new(RenderError(e.to_string())) as Box<dyn std::fmt::Display>)?;
                let file = dir.join(format!("slide_{}.png", i + 1));
                std::fs::write(&file, &bytes)
                    .map_err(|e| Box::new(RenderError(e.to_string())) as Box<dyn std::fmt::Display>)?;
                results.push(json!({ "slide": i + 1, "saved": file.display().to_string() }));
            }
        } else {
            results.push(result);
        }
    }

    Ok(json!({ "slides": results, "count": count }))
}

#[derive(Debug)]
struct RenderError(String);
impl std::fmt::Display for RenderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
