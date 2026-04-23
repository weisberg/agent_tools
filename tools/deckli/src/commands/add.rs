use crate::client;
use crate::units::Points;
use crate::AddTarget;
use serde_json::{json, Value};

pub async fn run(
    bridge_url: &str,
    target: AddTarget,
) -> Result<Value, Box<dyn std::fmt::Display>> {
    let (method, params) = match target {
        AddTarget::Slide { layout, at } => {
            let mut p = json!({ "layoutName": layout });
            if let Some(pos) = at {
                p["position"] = json!(pos - 1);
            }
            ("add.slide", p)
        }
        AddTarget::Shape { slide, shape_type, left, top, width, height, fill, text } => {
            let mut p = json!({
                "slideIndex": slide - 1,
                "type": shape_type,
                "left": parse_pts(&left)?,
                "top": parse_pts(&top)?,
                "width": parse_pts(&width)?,
                "height": parse_pts(&height)?,
            });
            if let Some(f) = fill { p["fill"] = json!(f); }
            if let Some(t) = text { p["text"] = json!(t); }
            ("add.shape", p)
        }
        AddTarget::Image { slide, src, left, top, width, height } => {
            // Read image file and base64-encode it
            let data = std::fs::read(&src).map_err(|e| {
                Box::new(AddError(format!("cannot read {}: {e}", src.display()))) as Box<dyn std::fmt::Display>
            })?;
            use base64::Engine;
            let b64 = base64::engine::general_purpose::STANDARD.encode(&data);

            let ext = src.extension().and_then(|e| e.to_str()).unwrap_or("png");
            ("add.image", json!({
                "slideIndex": slide - 1,
                "imageBase64": b64,
                "format": ext,
                "left": parse_pts(&left)?,
                "top": parse_pts(&top)?,
                "width": parse_pts(&width)?,
                "height": parse_pts(&height)?,
            }))
        }
        AddTarget::Table { slide, data, left, top, width, height } => {
            let table_data: Value = serde_json::from_str(&data).map_err(|e| {
                Box::new(AddError(format!("invalid --data JSON: {e}"))) as Box<dyn std::fmt::Display>
            })?;
            ("add.table", json!({
                "slideIndex": slide - 1,
                "data": table_data,
                "left": parse_pts(&left)?,
                "top": parse_pts(&top)?,
                "width": parse_pts(&width)?,
                "height": parse_pts(&height)?,
            }))
        }
    };

    client::send_command(bridge_url, method, params)
        .await
        .map_err(|e| Box::new(e) as Box<dyn std::fmt::Display>)
}

fn parse_pts(s: &str) -> Result<f64, Box<dyn std::fmt::Display>> {
    Points::parse(s)
        .map(|p| p.0)
        .map_err(|e| Box::new(AddError(e.to_string())) as Box<dyn std::fmt::Display>)
}

#[derive(Debug)]
struct AddError(String);
impl std::fmt::Display for AddError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
