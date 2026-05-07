use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::Instant;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::cli::Cli;
use crate::error::NotionliError;
use crate::storage::sqlite_exec;
use crate::util::{
    command_exists, default_config_home, default_home, ensure_home, run_shell_capture,
};

#[derive(Debug, Clone)]
pub(crate) enum AuthSource {
    TokenCommand,
    Env,
    ApiKeyFile,
    Oauth,
    Keychain,
    Plaintext,
}

#[derive(Debug, Clone)]
pub(crate) struct AuthToken {
    pub(crate) token: String,
    pub(crate) source: AuthSource,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct OauthCredentials {
    pub(crate) access_token: String,
    pub(crate) refresh_token: String,
    #[serde(default)]
    pub(crate) bot_id: Option<String>,
    #[serde(default)]
    pub(crate) workspace_id: Option<String>,
    #[serde(default)]
    pub(crate) workspace_name: Option<String>,
    #[serde(default)]
    pub(crate) workspace_icon: Option<String>,
    #[serde(default)]
    pub(crate) duplicated_template_id: Option<String>,
    #[serde(default)]
    pub(crate) owner: Option<Value>,
    #[serde(default)]
    pub(crate) obtained_at: Option<String>,
    #[serde(default)]
    pub(crate) refreshed_at: Option<String>,
}

pub(crate) struct Context {
    pub(crate) profile: String,
    pub(crate) api_version: String,
    pub(crate) home: PathBuf,
    pub(crate) config_home: PathBuf,
    pub(crate) profile_dir: PathBuf,
    pub(crate) profile_config_dir: PathBuf,
    pub(crate) db_path: PathBuf,
    pub(crate) started_at: Instant,
    pub(crate) token_cmd: Option<String>,
    pub(crate) pick_first: bool,
    pub(crate) dry_run: bool,
    pub(crate) policy: Option<PathBuf>,
    pub(crate) retry: u32,
    pub(crate) json: bool,
    pub(crate) jsonl: bool,
    pub(crate) format: Option<String>,
    pub(crate) quiet: bool,
}

impl Context {
    pub(crate) fn from_cli(cli: &Cli) -> Result<Self, NotionliError> {
        let requested_home = cli
            .home
            .clone()
            .or_else(|| env::var_os("NOTIONLI_HOME").map(PathBuf::from))
            .unwrap_or_else(default_home);
        let home = ensure_home(requested_home)?;
        let config_home = env::var_os("NOTIONLI_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(default_config_home);
        let profile_dir = home.join("profiles").join(&cli.profile);
        let profile_config_dir = config_home.join("profiles").join(&cli.profile);
        fs::create_dir_all(&profile_dir)?;
        fs::create_dir_all(&profile_config_dir)?;
        fs::create_dir_all(home.join("templates"))?;
        fs::create_dir_all(home.join("queries"))?;
        fs::create_dir_all(home.join("workflows"))?;
        fs::create_dir_all(home.join("files"))?;
        let db_path = profile_dir.join("cache.sqlite");
        let ctx = Self {
            profile: cli.profile.clone(),
            api_version: cli.api_version.clone(),
            home,
            config_home,
            profile_dir,
            profile_config_dir,
            db_path,
            started_at: Instant::now(),
            token_cmd: cli.token_cmd.clone(),
            pick_first: cli.pick_first,
            dry_run: cli.dry_run || !cli.apply,
            policy: cli.policy.clone(),
            retry: cli.retry,
            json: cli.json,
            jsonl: cli.jsonl,
            format: cli.format.clone(),
            quiet: cli.quiet,
        };
        ctx.init_db()?;
        Ok(ctx)
    }

    pub(crate) fn init_db(&self) -> Result<(), NotionliError> {
        let schema = r#"
PRAGMA journal_mode=WAL;
CREATE TABLE IF NOT EXISTS aliases (
  name TEXT PRIMARY KEY,
  object_type TEXT NOT NULL,
  object_id TEXT NOT NULL,
  reference TEXT NOT NULL,
  title TEXT,
  url TEXT,
  updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS objects (
  object_type TEXT NOT NULL,
  object_id TEXT PRIMARY KEY,
  slug TEXT,
  title TEXT,
  url TEXT,
  raw_json TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
CREATE VIRTUAL TABLE IF NOT EXISTS objects_fts USING fts5(object_id, object_type, slug, title, raw_json);
CREATE TABLE IF NOT EXISTS state (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS oplog (
  operation_id TEXT PRIMARY KEY,
  command TEXT NOT NULL,
  target TEXT NOT NULL,
  receipt_json TEXT NOT NULL,
  inverse_command TEXT,
  created_at TEXT NOT NULL,
  status TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS config (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS comment_resolutions (
  comment_id TEXT PRIMARY KEY,
  status TEXT NOT NULL,
  resolved_at TEXT NOT NULL
);
"#;
        sqlite_exec(&self.db_path, schema)
    }

    pub(crate) fn token(&self) -> Result<String, NotionliError> {
        Ok(self.auth_token()?.token)
    }

    pub(crate) fn auth_token(&self) -> Result<AuthToken, NotionliError> {
        if let Some(cmd) = &self.token_cmd {
            return Ok(AuthToken {
                token: run_shell_capture(cmd)?,
                source: AuthSource::TokenCommand,
            });
        }
        if let Ok(value) = env::var("NOTION_API_KEY") {
            if !value.trim().is_empty() {
                return Ok(AuthToken {
                    token: value.trim().to_string(),
                    source: AuthSource::Env,
                });
            }
        }
        if let Some(credentials) = self.oauth_credentials()? {
            if !credentials.access_token.trim().is_empty() {
                return Ok(AuthToken {
                    token: credentials.access_token.trim().to_string(),
                    source: AuthSource::Oauth,
                });
            }
        }
        for path in notion_api_key_file_candidates() {
            if path.exists() {
                let token = fs::read_to_string(&path)?.trim().to_string();
                if !token.is_empty() {
                    return Ok(AuthToken {
                        token,
                        source: AuthSource::ApiKeyFile,
                    });
                }
            }
        }
        let key = format!("notionli.{}", self.profile);
        if command_exists("security") {
            let output = Command::new("security")
                .args(["find-generic-password", "-a", &key, "-s", "notionli", "-w"])
                .output();
            if let Ok(output) = output {
                if output.status.success() {
                    let token = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    if !token.is_empty() {
                        return Ok(AuthToken {
                            token,
                            source: AuthSource::Keychain,
                        });
                    }
                }
            }
        }
        let plaintext = self.profile_dir.join("token.plaintext");
        if plaintext.exists() {
            return Ok(AuthToken {
                token: fs::read_to_string(plaintext)?.trim().to_string(),
                source: AuthSource::Plaintext,
            });
        }
        Err(NotionliError::Auth {
            message: "No Notion token found for this profile.".into(),
        })
    }

    pub(crate) fn oauth_credentials_path(&self) -> PathBuf {
        self.profile_config_dir.join("oauth.json")
    }

    pub(crate) fn oauth_credentials(&self) -> Result<Option<OauthCredentials>, NotionliError> {
        let path = self.oauth_credentials_path();
        if !path.exists() {
            return Ok(None);
        }
        let text = fs::read_to_string(path)?;
        Ok(Some(serde_json::from_str(&text)?))
    }

    pub(crate) fn store_oauth_credentials(
        &self,
        credentials: &OauthCredentials,
    ) -> Result<PathBuf, NotionliError> {
        fs::create_dir_all(&self.profile_config_dir)?;
        let path = self.oauth_credentials_path();
        fs::write(&path, serde_json::to_string_pretty(credentials)?)?;
        set_owner_read_write(&path)?;
        Ok(path)
    }

    pub(crate) fn store_oauth_client_config(
        &self,
        value: &Value,
    ) -> Result<PathBuf, NotionliError> {
        fs::create_dir_all(&self.config_home)?;
        let path = self.config_home.join("oauth-client.json");
        fs::write(&path, serde_json::to_string_pretty(value)?)?;
        set_owner_read_write(&path)?;
        Ok(path)
    }
}

fn notion_api_key_file_candidates() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(config_home) = env::var_os("XDG_CONFIG_HOME") {
        paths.push(PathBuf::from(config_home).join("NOTION_API_KEY"));
    }
    if let Some(home) = env::var_os("HOME") {
        paths.push(PathBuf::from(home).join(".config").join("NOTION_API_KEY"));
    }
    paths
}

#[cfg(unix)]
fn set_owner_read_write(path: &PathBuf) -> Result<(), NotionliError> {
    use std::os::unix::fs::PermissionsExt;

    let mut permissions = fs::metadata(path)?.permissions();
    permissions.set_mode(0o600);
    fs::set_permissions(path, permissions)?;
    Ok(())
}

#[cfg(not(unix))]
fn set_owner_read_write(_path: &PathBuf) -> Result<(), NotionliError> {
    Ok(())
}
