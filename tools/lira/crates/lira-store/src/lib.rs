use fs2::FileExt;
use lira_core::{
    filesystem_error, validate_project_key, Counters, LiraError, LiraResult, Project, Ticket,
    Workflow, SCHEMA_VERSION,
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::json;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize)]
pub struct InitReport {
    pub root: PathBuf,
    pub existed: bool,
    pub created: Vec<PathBuf>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct JsonlEvent {
    pub schema_version: u32,
    pub timestamp: String,
    pub action: String,
    pub ticket: Option<String>,
    pub project: Option<String>,
    pub result: String,
    pub details: serde_json::Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct DoctorReport {
    pub root: PathBuf,
    pub ok: bool,
    pub projects: Vec<ProjectDoctorReport>,
    pub issues: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProjectDoctorReport {
    pub key: String,
    pub tickets: usize,
    pub issues: Vec<String>,
}

pub struct WorkspaceLock {
    file: File,
}

impl Drop for WorkspaceLock {
    fn drop(&mut self) {
        let _ = self.file.unlock();
    }
}

pub fn lira_home() -> LiraResult<PathBuf> {
    if let Ok(path) = std::env::var("LIRA_HOME") {
        return Ok(PathBuf::from(path));
    }

    let home = std::env::var("HOME")
        .map_err(|_| LiraError::new("E_HOME_NOT_FOUND", "HOME is not set."))?;
    Ok(PathBuf::from(home).join(".lira"))
}

pub fn project_dir() -> LiraResult<PathBuf> {
    Ok(lira_home()?.join("projects"))
}

pub fn init_workspace(dry_run: bool) -> LiraResult<InitReport> {
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
                create_dir_all(&path)?;
            }
        }
    }

    let config_path = root.join("config.yaml");
    if !config_path.exists() {
        created.push(config_path.clone());
        if !dry_run {
            write_yaml_atomic(&config_path, &json!({"schema_version": SCHEMA_VERSION}))?;
        }
    }

    Ok(InitReport {
        root,
        existed,
        created,
    })
}

pub fn create_project(key: &str, name: &str) -> LiraResult<Project> {
    validate_project_key(key)?;
    init_workspace(false)?;
    let _lock = workspace_lock("project-create")?;

    let project_root = project_root(key)?;
    if project_root.exists() {
        return Err(LiraError::new(
            "E_PROJECT_EXISTS",
            format!("Project '{key}' already exists."),
        ));
    }

    create_dir_all(&project_root)?;
    create_dir_all(&project_root.join("tickets"))?;
    create_dir_all(&project_root.join("links"))?;
    let workflow = Workflow::default_for(key);
    for status in &workflow.statuses {
        create_dir_all(&project_root.join("tickets").join(&status.id))?;
    }

    let project = Project::new(key, name);
    write_yaml_atomic(&project_root.join("project.yaml"), &project)?;
    write_yaml_atomic(&project_root.join("workflow.yaml"), &workflow)?;
    write_yaml_atomic(&project_root.join("counters.yaml"), &Counters::new(key))?;
    Ok(project)
}

pub fn list_projects() -> LiraResult<Vec<Project>> {
    let dir = project_dir()?;
    let mut projects = Vec::new();
    if !dir.exists() {
        return Ok(projects);
    }
    for entry in std::fs::read_dir(&dir).map_err(filesystem_error)? {
        let entry = entry.map_err(filesystem_error)?;
        if entry.path().is_dir() {
            let path = entry.path().join("project.yaml");
            if path.exists() {
                projects.push(read_yaml(&path)?);
            }
        }
    }
    projects.sort_by(|a: &Project, b: &Project| a.key.cmp(&b.key));
    Ok(projects)
}

pub fn read_project(key: &str) -> LiraResult<Project> {
    let path = project_root(key)?.join("project.yaml");
    if !path.exists() {
        return Err(project_not_found(key));
    }
    read_yaml(&path)
}

pub fn read_workflow(key: &str) -> LiraResult<Workflow> {
    let path = project_root(key)?.join("workflow.yaml");
    if path.exists() {
        read_yaml(&path)
    } else {
        Ok(Workflow::default_for(key))
    }
}

pub fn allocate_ticket_id(project: &str) -> LiraResult<String> {
    let _lock = workspace_lock(&format!("counter-{project}"))?;
    let path = project_root(project)?.join("counters.yaml");
    let mut counters: Counters = if path.exists() {
        read_yaml(&path)?
    } else {
        Counters::new(project)
    };
    let id = format!("{project}-{}", counters.next_ticket);
    counters.next_ticket += 1;
    write_yaml_atomic(&path, &counters)?;
    Ok(id)
}

pub fn write_ticket(ticket: &Ticket) -> LiraResult<PathBuf> {
    let _lock = workspace_lock(&format!("ticket-{}", ticket.id))?;
    let workflow = read_workflow(&ticket.project)?;
    lira_core::validate_ticket(ticket, &workflow)?;
    let path = ticket_path_for(&ticket.project, &ticket.status, &ticket.id)?;
    create_dir_all(path.parent().expect("ticket path has parent"))?;
    write_yaml_atomic(&path, ticket)?;
    Ok(path)
}

pub fn update_ticket<F>(id: &str, mut update: F) -> LiraResult<Ticket>
where
    F: FnMut(&mut Ticket) -> LiraResult<()>,
{
    let _lock = workspace_lock(&format!("ticket-{id}"))?;
    let (path, mut ticket) = read_ticket_with_path(id)?;
    update(&mut ticket)?;
    let workflow = read_workflow(&ticket.project)?;
    lira_core::validate_ticket(&ticket, &workflow)?;
    let new_path = ticket_path_for(&ticket.project, &ticket.status, &ticket.id)?;
    if path == new_path {
        write_yaml_atomic(&path, &ticket)?;
    } else {
        create_dir_all(new_path.parent().expect("ticket path has parent"))?;
        write_yaml_atomic(&path, &ticket)?;
        std::fs::rename(&path, &new_path).map_err(filesystem_error)?;
    }
    Ok(ticket)
}

pub fn read_ticket(id: &str) -> LiraResult<Ticket> {
    read_ticket_with_path(id).map(|(_, ticket)| ticket)
}

pub fn read_ticket_with_path(id: &str) -> LiraResult<(PathBuf, Ticket)> {
    let project = project_from_ticket_id(id)?;
    let root = project_root(project)?;
    if !root.exists() {
        return Err(project_not_found(project));
    }
    let ticket_root = root.join("tickets");
    for entry in std::fs::read_dir(&ticket_root).map_err(filesystem_error)? {
        let entry = entry.map_err(filesystem_error)?;
        if !entry.path().is_dir() {
            continue;
        }
        let path = entry.path().join(format!("{id}.yaml"));
        if path.exists() {
            let ticket = read_yaml(&path)?;
            return Ok((path, ticket));
        }
    }
    Err(
        LiraError::new("E_TICKET_NOT_FOUND", format!("Ticket '{id}' not found."))
            .suggestion("lira ls --json", "list local tickets"),
    )
}

pub fn list_tickets(
    project_filter: Option<&str>,
    status_filter: Option<&str>,
) -> LiraResult<Vec<Ticket>> {
    let mut tickets: Vec<Ticket> = Vec::new();
    let projects = if let Some(project) = project_filter {
        vec![read_project(project)?]
    } else {
        list_projects()?
    };

    for project in projects {
        let ticket_root = project_root(&project.key)?.join("tickets");
        if !ticket_root.exists() {
            continue;
        }
        for status_entry in std::fs::read_dir(&ticket_root).map_err(filesystem_error)? {
            let status_entry = status_entry.map_err(filesystem_error)?;
            if !status_entry.path().is_dir() {
                continue;
            }
            let status = status_entry.file_name().to_string_lossy().to_string();
            if status_filter.is_some_and(|wanted| wanted != status) {
                continue;
            }
            for ticket_entry in std::fs::read_dir(status_entry.path()).map_err(filesystem_error)? {
                let ticket_entry = ticket_entry.map_err(filesystem_error)?;
                if ticket_entry.path().extension().and_then(|ext| ext.to_str()) == Some("yaml") {
                    tickets.push(read_yaml(&ticket_entry.path())?);
                }
            }
        }
    }

    tickets.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(tickets)
}

pub fn append_log(event: JsonlEvent) -> LiraResult<()> {
    init_workspace(false)?;
    let date = event
        .timestamp
        .split('T')
        .next()
        .unwrap_or("unknown-date")
        .to_string();
    let path = lira_home()?.join("logs").join(format!("{date}.jsonl"));
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(filesystem_error)?;
    let line = serde_json::to_string(&event).map_err(filesystem_error)?;
    writeln!(file, "{line}").map_err(filesystem_error)?;
    file.flush().map_err(filesystem_error)?;
    Ok(())
}

pub fn doctor() -> LiraResult<DoctorReport> {
    let root = lira_home()?;
    let mut issues = Vec::new();
    for rel in ["projects", "index", "gh-cache", "locks", "logs"] {
        if !root.join(rel).exists() {
            issues.push(format!("missing {}", root.join(rel).display()));
        }
    }

    let mut projects = Vec::new();
    for project in list_projects()? {
        let mut project_issues = Vec::new();
        let workflow = read_workflow(&project.key)?;
        let tickets = list_tickets(Some(&project.key), None)?;
        for ticket in &tickets {
            if let Err(err) = lira_core::validate_ticket(ticket, &workflow) {
                project_issues.push(format!("{}: {}", ticket.id, err.message));
            }
            let expected = ticket_path_for(&ticket.project, &ticket.status, &ticket.id)?;
            if !expected.exists() {
                project_issues.push(format!(
                    "{}: status/path drift; expected {}",
                    ticket.id,
                    expected.display()
                ));
            }
        }
        projects.push(ProjectDoctorReport {
            key: project.key,
            tickets: tickets.len(),
            issues: project_issues,
        });
    }

    let ok = issues.is_empty() && projects.iter().all(|project| project.issues.is_empty());
    Ok(DoctorReport {
        root,
        ok,
        projects,
        issues,
    })
}

fn project_root(key: &str) -> LiraResult<PathBuf> {
    Ok(project_dir()?.join(key))
}

fn ticket_path_for(project: &str, status: &str, id: &str) -> LiraResult<PathBuf> {
    Ok(project_root(project)?
        .join("tickets")
        .join(status)
        .join(format!("{id}.yaml")))
}

fn project_from_ticket_id(id: &str) -> LiraResult<&str> {
    id.split_once('-')
        .map(|(project, _)| project)
        .ok_or_else(|| LiraError::new("E_INVALID_TICKET_ID", format!("Invalid ticket id '{id}'.")))
}

fn project_not_found(key: &str) -> LiraError {
    LiraError::new("E_PROJECT_NOT_FOUND", format!("Project '{key}' not found."))
        .suggestion("lira project list --json", "list available projects")
}

fn workspace_lock(name: &str) -> LiraResult<WorkspaceLock> {
    init_workspace(false)?;
    let lock_path = lira_home()?.join("locks").join(format!("{name}.lock"));
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .read(true)
        .open(lock_path)
        .map_err(filesystem_error)?;
    file.try_lock_exclusive()
        .map_err(|_| LiraError::new("E_LOCK_UNAVAILABLE", "Workspace lock is held."))?;
    Ok(WorkspaceLock { file })
}

fn read_yaml<T: DeserializeOwned>(path: &Path) -> LiraResult<T> {
    let body = std::fs::read_to_string(path).map_err(filesystem_error)?;
    serde_yaml::from_str(&body).map_err(|err| {
        LiraError::new(
            "E_INVALID_YAML",
            format!("Invalid YAML in {}.", path.display()),
        )
        .details(json!({ "error": err.to_string() }))
    })
}

fn write_yaml_atomic<T: Serialize>(path: &Path, value: &T) -> LiraResult<()> {
    let body = serde_yaml::to_string(value).map_err(filesystem_error)?;
    atomic_write(path, body.as_bytes())
}

fn atomic_write(path: &Path, bytes: &[u8]) -> LiraResult<()> {
    if let Some(parent) = path.parent() {
        create_dir_all(parent)?;
    }
    let tmp = path.with_extension("tmp");
    {
        let mut file = File::create(&tmp).map_err(filesystem_error)?;
        file.write_all(bytes).map_err(filesystem_error)?;
        file.sync_all().map_err(filesystem_error)?;
    }
    std::fs::rename(&tmp, path).map_err(filesystem_error)?;
    Ok(())
}

fn create_dir_all(path: &Path) -> LiraResult<()> {
    std::fs::create_dir_all(path).map_err(filesystem_error)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(path)
            .map_err(filesystem_error)?
            .permissions();
        perms.set_mode(0o700);
        std::fs::set_permissions(path, perms).map_err(filesystem_error)?;
    }
    Ok(())
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
