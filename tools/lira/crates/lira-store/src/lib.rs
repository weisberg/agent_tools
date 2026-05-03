use anyhow::Result;
use serde::Serialize;
use std::path::PathBuf;

#[derive(Debug, Serialize)]
pub struct InitReport {
    pub root: PathBuf,
    pub existed: bool,
    pub created: Vec<PathBuf>,
}

pub fn lira_home() -> Result<PathBuf> {
    if let Ok(path) = std::env::var("LIRA_HOME") {
        return Ok(PathBuf::from(path));
    }

    let home = std::env::var("HOME")?;
    Ok(PathBuf::from(home).join(".lira"))
}

pub fn project_dir() -> Result<PathBuf> {
    Ok(lira_home()?.join("projects"))
}

pub fn init_workspace(dry_run: bool) -> Result<InitReport> {
    let root = lira_home()?;
    let existed = root.exists();
    let mut created = Vec::new();

    for rel in ["", "projects", "index", "gh-cache", "locks", "logs"] {
        let path = if rel.is_empty() {
            root.clone()
        } else {
            root.join(rel)
        };
        if !path.exists() {
            created.push(path.clone());
            if !dry_run {
                std::fs::create_dir_all(&path)?;
            }
        }
    }

    let config_path = root.join("config.yaml");
    if !config_path.exists() {
        created.push(config_path.clone());
        if !dry_run {
            let cfg = serde_yaml::to_string(&serde_json::json!({"schema_version": 3}))?;
            std::fs::write(&config_path, cfg)?;
        }
    }

    Ok(InitReport {
        root,
        existed,
        created,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lira_home_uses_override() {
        let before = std::env::var("LIRA_HOME").ok();
        std::env::set_var("LIRA_HOME", "/tmp/lira-home-test");
        let got = lira_home().expect("home");
        assert_eq!(got, PathBuf::from("/tmp/lira-home-test"));
        if let Some(prev) = before {
            std::env::set_var("LIRA_HOME", prev);
        } else {
            std::env::remove_var("LIRA_HOME");
        }
    }
}
