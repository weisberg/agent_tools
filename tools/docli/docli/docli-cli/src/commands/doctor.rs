use std::path::PathBuf;
use std::process::Command;

use clap::Args;
use serde::Serialize;

use docli_core::EnvelopeBuilder;

use crate::envelope::emit;

#[derive(Args)]
pub struct DoctorArgs;

#[derive(Serialize)]
struct DoctorData {
    tools: Vec<ToolCheck>,
    kb: KbCheck,
    temp: TempCheck,
}

#[derive(Serialize)]
struct ToolCheck {
    name: String,
    available: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    version: Option<String>,
}

#[derive(Serialize)]
struct KbCheck {
    root: Option<String>,
    exists: bool,
    readable: bool,
}

#[derive(Serialize)]
struct TempCheck {
    path: String,
    writable: bool,
}

pub fn run(_args: DoctorArgs, format: &str, pretty: bool) -> i32 {
    let builder = EnvelopeBuilder::new("doctor");

    let tools = vec![
        check_tool("pandoc"),
        check_tool("soffice"),
        check_tool("pdftoppm"),
    ];

    let kb = check_kb();
    let temp = check_temp();

    let data = DoctorData { tools, kb, temp };
    let envelope = builder.ok(data);
    if let Err(e) = emit(&envelope, format, pretty) {
        eprintln!("output error: {e}");
        return 1;
    }
    0
}

fn check_tool(name: &str) -> ToolCheck {
    let which = Command::new("which").arg(name).output();

    match which {
        Ok(output) if output.status.success() => {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let version = Command::new(name)
                .arg("--version")
                .output()
                .ok()
                .filter(|o| o.status.success())
                .map(|o| {
                    String::from_utf8_lossy(&o.stdout)
                        .lines()
                        .next()
                        .unwrap_or_default()
                        .to_string()
                });
            ToolCheck {
                name: name.to_string(),
                available: true,
                path: Some(path),
                version,
            }
        }
        _ => ToolCheck {
            name: name.to_string(),
            available: false,
            path: None,
            version: None,
        },
    }
}

fn check_kb() -> KbCheck {
    let root = std::env::var("DOCLI_KB_ROOT").ok();
    let (exists, readable) = if let Some(ref root) = root {
        let path = PathBuf::from(root);
        let exists = path.exists();
        let readable = exists && path.read_dir().is_ok();
        (exists, readable)
    } else {
        (false, false)
    };
    KbCheck {
        root,
        exists,
        readable,
    }
}

fn check_temp() -> TempCheck {
    let tmp = std::env::temp_dir();
    let writable = tempfile_writable(&tmp);
    TempCheck {
        path: tmp.display().to_string(),
        writable,
    }
}

fn tempfile_writable(dir: &PathBuf) -> bool {
    let test_path = dir.join(".docli-doctor-check");
    if std::fs::write(&test_path, b"ok").is_ok() {
        let _ = std::fs::remove_file(&test_path);
        true
    } else {
        false
    }
}
