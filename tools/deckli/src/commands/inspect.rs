use crate::client;
use serde_json::{json, Value};

pub async fn run(
    bridge_url: &str,
    masters: bool,
    theme: bool,
    slide: Option<u32>,
) -> Result<Value, Box<dyn std::fmt::Display>> {
    let method = if masters {
        "inspect.masters"
    } else if theme {
        "inspect.theme"
    } else if let Some(idx) = slide {
        return client::send_command(bridge_url, "get.slide", json!({ "slideIndex": idx - 1 }))
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::fmt::Display>);
    } else {
        "inspect"
    };

    client::send_command(bridge_url, method, Value::Null)
        .await
        .map_err(|e| Box::new(e) as Box<dyn std::fmt::Display>)
}
