use docli_core::Envelope;
use serde::Serialize;

/// Emit an envelope in the requested format to stdout.
pub fn emit<T: Serialize>(envelope: &Envelope<T>, format: &str, pretty: bool) -> Result<(), String> {
    let output = match format {
        "yaml" => serde_yaml::to_string(envelope).map_err(|e| e.to_string())?,
        "text" => {
            // For text output, use pretty JSON as a reasonable fallback
            serde_json::to_string_pretty(envelope).map_err(|e| e.to_string())?
        }
        _ => {
            // json (default)
            if pretty {
                serde_json::to_string_pretty(envelope).map_err(|e| e.to_string())?
            } else {
                serde_json::to_string(envelope).map_err(|e| e.to_string())?
            }
        }
    };
    println!("{output}");
    Ok(())
}
