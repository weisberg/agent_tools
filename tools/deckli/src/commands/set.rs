use crate::client;
use crate::units::Points;
use serde_json::{json, Value};

#[allow(clippy::too_many_arguments)]
pub async fn run(
    bridge_url: &str,
    path: &str,
    value: Option<&str>,
    size: Option<f64>,
    bold: Option<bool>,
    italic: Option<bool>,
    left: Option<&str>,
    top: Option<&str>,
    width: Option<&str>,
    height: Option<&str>,
) -> Result<Value, Box<dyn std::fmt::Display>> {
    let (method, params) = parse_set_path(path, value, size, bold, italic, left, top, width, height)?;

    client::send_command(bridge_url, &method, params)
        .await
        .map_err(|e| Box::new(e) as Box<dyn std::fmt::Display>)
}

#[allow(clippy::too_many_arguments)]
fn parse_set_path(
    path: &str,
    value: Option<&str>,
    size: Option<f64>,
    bold: Option<bool>,
    italic: Option<bool>,
    left: Option<&str>,
    top: Option<&str>,
    width: Option<&str>,
    height: Option<&str>,
) -> Result<(String, Value), Box<dyn std::fmt::Display>> {
    let parts: Vec<&str> = path.trim_start_matches('/').split('/').collect();

    match parts.as_slice() {
        // /slides/N/shapes/ID/text "value"
        ["slides", slide_idx, "shapes", shape_id, "text"] => {
            let i = parse_1based(slide_idx)?;
            Ok(("set.text".to_string(), json!({
                "slideIndex": i,
                "shapeId": *shape_id,
                "text": value.unwrap_or(""),
            })))
        }
        // /slides/N/shapes/ID/fill "#hex"
        ["slides", slide_idx, "shapes", shape_id, "fill"] => {
            let i = parse_1based(slide_idx)?;
            Ok(("set.fill".to_string(), json!({
                "slideIndex": i,
                "shapeId": *shape_id,
                "color": value.unwrap_or(""),
            })))
        }
        // /slides/N/shapes/ID/font --size 24 --bold
        ["slides", slide_idx, "shapes", shape_id, "font"] => {
            let i = parse_1based(slide_idx)?;
            let mut params = json!({
                "slideIndex": i,
                "shapeId": *shape_id,
            });
            if let Some(s) = size { params["size"] = json!(s); }
            if let Some(b) = bold { params["bold"] = json!(b); }
            if let Some(it) = italic { params["italic"] = json!(it); }
            Ok(("set.font".to_string(), params))
        }
        // /slides/N/shapes/ID/geometry --left 1in ...
        ["slides", slide_idx, "shapes", shape_id, "geometry"] => {
            let i = parse_1based(slide_idx)?;
            let mut params = json!({
                "slideIndex": i,
                "shapeId": *shape_id,
            });
            if let Some(l) = left { params["left"] = json!(Points::parse(l).map_err(fmt_err)?.0); }
            if let Some(t) = top { params["top"] = json!(Points::parse(t).map_err(fmt_err)?.0); }
            if let Some(w) = width { params["width"] = json!(Points::parse(w).map_err(fmt_err)?.0); }
            if let Some(h) = height { params["height"] = json!(Points::parse(h).map_err(fmt_err)?.0); }
            Ok(("set.geometry".to_string(), params))
        }
        _ => Err(Box::new(SetPathError(format!(
            "unrecognized set path: {path}. Expected /slides/N/shapes/ID/{{text,fill,font,geometry}}"
        )))),
    }
}

fn parse_1based(s: &str) -> Result<u32, Box<dyn std::fmt::Display>> {
    let n: u32 = s.parse().map_err(|_| Box::new(SetPathError(format!("invalid index: {s}"))) as Box<dyn std::fmt::Display>)?;
    if n == 0 {
        return Err(Box::new(SetPathError("index is 1-based, got 0".to_string())));
    }
    Ok(n - 1)
}

fn fmt_err(e: crate::units::UnitError) -> Box<dyn std::fmt::Display> {
    Box::new(SetPathError(e.to_string()))
}

#[derive(Debug)]
struct SetPathError(String);
impl std::fmt::Display for SetPathError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
