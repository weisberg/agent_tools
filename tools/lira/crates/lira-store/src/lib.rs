use fs2::FileExt;
use lira_core::{
    filesystem_error, issue_from_ticket, validate_project_key, BlockerRef, Counters, LiraError,
    LiraResult, NormalizedIssue, Project, TaskSummary, Ticket, TicketSummary, Workflow,
    SCHEMA_VERSION,
};
use rusqlite::{params, params_from_iter, Connection, OpenFlags, OptionalExtension};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeMap;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

const INDEX_SCHEMA_VERSION: u32 = 2;

#[derive(Debug, Serialize)]
pub struct InitReport {
    pub root: PathBuf,
    pub existed: bool,
    pub created: Vec<PathBuf>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReindexReport {
    pub path: PathBuf,
    pub schema_version: u32,
    pub tickets_indexed: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexStaleMarker {
    pub schema_version: u32,
    pub reason: String,
    pub error_code: Option<String>,
    pub message: Option<String>,
    pub marked_at: String,
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
    pub index: IndexDoctorReport,
    pub projects: Vec<ProjectDoctorReport>,
    pub issues: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct IndexDoctorReport {
    pub path: PathBuf,
    pub exists: bool,
    pub stale: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stale_marker: Option<IndexStaleMarker>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schema_version: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tickets_indexed: Option<usize>,
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

pub fn index_path() -> LiraResult<PathBuf> {
    Ok(lira_home()?.join("index").join("tickets.sqlite"))
}

pub fn index_stale_path() -> LiraResult<PathBuf> {
    Ok(lira_home()?.join("index").join("stale.json"))
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
    if let Err(err) = upsert_ticket_index(ticket) {
        let _ = mark_index_stale("ticket index write-through failed", Some(&err));
    }
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
        let tmp_path = path.with_extension("moving");
        write_yaml_atomic(&tmp_path, &ticket)?;
        std::fs::rename(&tmp_path, &new_path).map_err(filesystem_error)?;
        if path.exists() {
            std::fs::remove_file(&path).map_err(filesystem_error)?;
        }
    }
    if let Err(err) = upsert_ticket_index(&ticket) {
        let _ = mark_index_stale("ticket index write-through failed", Some(&err));
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
    let mut tickets: Vec<Ticket> = list_ticket_files(project_filter, status_filter)?
        .into_iter()
        .map(|(_, ticket)| ticket)
        .collect();
    tickets.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(tickets)
}

fn list_ticket_files(
    project_filter: Option<&str>,
    status_filter: Option<&str>,
) -> LiraResult<Vec<(PathBuf, Ticket)>> {
    let mut tickets: Vec<(PathBuf, Ticket)> = Vec::new();
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
                    let path = ticket_entry.path();
                    tickets.push((path.clone(), read_yaml(&path)?));
                }
            }
        }
    }

    tickets.sort_by(|(_, a), (_, b)| a.id.cmp(&b.id));
    Ok(tickets)
}

pub fn reindex() -> LiraResult<ReindexReport> {
    init_workspace(false)?;
    let path = index_path()?;
    let conn = open_index()?;
    reset_index_schema(&conn)?;
    let tickets = list_ticket_files(None, None)?;
    for (source_path, ticket) in &tickets {
        insert_ticket_index(&conn, source_path, ticket)?;
    }
    conn.execute(
        "INSERT OR REPLACE INTO index_meta(key, value) VALUES ('schema_version', ?1)",
        params![INDEX_SCHEMA_VERSION.to_string()],
    )
    .map_err(index_error)?;
    conn.execute(
        "INSERT OR REPLACE INTO index_meta(key, value) VALUES ('rebuilt_at', ?1)",
        params![lira_core::now_string()],
    )
    .map_err(index_error)?;
    clear_index_stale_marker()?;
    Ok(ReindexReport {
        path,
        schema_version: INDEX_SCHEMA_VERSION,
        tickets_indexed: tickets.len(),
    })
}

#[derive(Debug, Clone, Default)]
pub struct TicketQuery<'a> {
    pub project: Option<&'a str>,
    pub status: Option<&'a str>,
    pub label: Option<&'a str>,
    pub assignee: Option<&'a str>,
    pub task_status: Option<&'a str>,
    pub task_tag: Option<&'a str>,
    pub parent_jira: Option<&'a str>,
}

pub fn search_tickets(query: &str, project: Option<&str>) -> LiraResult<Vec<TicketSummary>> {
    if query.trim().is_empty() {
        return Err(LiraError::new(
            "E_QUERY_REQUIRED",
            "Search query is required.",
        ));
    }
    ensure_index_ready()?;
    let conn = open_index()?;
    let fts_query = fts_query(query)?;
    let mut sql = String::from(
        "SELECT t.id FROM ticket_fts JOIN tickets t ON t.id = ticket_fts.id \
         WHERE ticket_fts MATCH ?",
    );
    let mut values = vec![fts_query];
    if let Some(project) = project {
        sql.push_str(" AND t.project = ?");
        values.push(project.to_string());
    }
    sql.push_str(" ORDER BY rank, t.id");
    let ids = query_ids(&conn, &sql, values)?;
    summaries_from_ids(ids)
}

pub fn search_tickets_no_index(
    query: &str,
    project: Option<&str>,
) -> LiraResult<Vec<TicketSummary>> {
    if query.trim().is_empty() {
        return Err(LiraError::new(
            "E_QUERY_REQUIRED",
            "Search query is required.",
        ));
    }
    let needle = query.to_lowercase();
    let mut tickets: Vec<TicketSummary> = list_tickets(project, None)?
        .into_iter()
        .filter(|ticket| ticket_search_text(ticket).to_lowercase().contains(&needle))
        .map(|ticket| TicketSummary::from(&ticket))
        .collect();
    tickets.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(tickets)
}

pub fn query_tickets(filters: TicketQuery<'_>) -> LiraResult<Vec<TicketSummary>> {
    ensure_index_ready()?;
    let conn = open_index()?;
    let mut sql = String::from("SELECT DISTINCT t.id FROM tickets t");
    if filters.label.is_some() {
        sql.push_str(" JOIN ticket_labels l ON l.ticket_id = t.id");
    }
    if filters.task_tag.is_some() {
        sql.push_str(" JOIN ticket_task_tags tt ON tt.ticket_id = t.id");
    }
    if filters.task_status.is_some() {
        sql.push_str(" JOIN ticket_task_statuses ts ON ts.ticket_id = t.id");
    }
    sql.push_str(" WHERE 1 = 1");

    let mut values = Vec::new();
    if let Some(project) = filters.project {
        sql.push_str(" AND t.project = ?");
        values.push(project.to_string());
    }
    if let Some(status) = filters.status {
        sql.push_str(" AND t.status = ?");
        values.push(status.to_string());
    }
    if let Some(label) = filters.label {
        sql.push_str(" AND l.label = ?");
        values.push(label.to_string());
    }
    if let Some(assignee) = filters.assignee {
        sql.push_str(" AND t.assignee = ?");
        values.push(assignee.to_string());
    }
    if let Some(status) = filters.task_status {
        sql.push_str(" AND ts.status = ? AND ts.count > 0");
        values.push(status.to_string());
    }
    if let Some(tag) = filters.task_tag {
        sql.push_str(" AND tt.tag = ?");
        values.push(tag.to_string());
    }
    if let Some(parent_jira) = filters.parent_jira {
        sql.push_str(" AND t.parent_type = 'jira' AND t.parent_id = ?");
        values.push(parent_jira.to_string());
    }
    sql.push_str(" ORDER BY t.id");

    let ids = query_ids(&conn, &sql, values)?;
    summaries_from_ids(ids)
}

pub fn query_tickets_no_index(filters: TicketQuery<'_>) -> LiraResult<Vec<TicketSummary>> {
    let mut tickets: Vec<TicketSummary> = list_tickets(filters.project, filters.status)?
        .into_iter()
        .filter(|ticket| ticket_matches_query(ticket, &filters))
        .map(|ticket| TicketSummary::from(&ticket))
        .collect();
    tickets.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(tickets)
}

pub fn count_tickets(group_by: &str, project: Option<&str>) -> LiraResult<BTreeMap<String, usize>> {
    ensure_index_ready()?;
    let column = match group_by {
        "status" => "status",
        "priority" => "priority",
        other => {
            return Err(LiraError::new(
                "E_INVALID_GROUP_BY",
                format!("Unsupported group-by '{other}'."),
            ));
        }
    };
    let conn = open_index()?;
    let mut sql = format!("SELECT {column}, COUNT(*) FROM tickets");
    let mut values = Vec::new();
    if let Some(project) = project {
        sql.push_str(" WHERE project = ?");
        values.push(project.to_string());
    }
    sql.push_str(&format!(" GROUP BY {column} ORDER BY {column}"));
    let mut stmt = conn.prepare(&sql).map_err(index_error)?;
    let rows = stmt
        .query_map(params_from_iter(values.iter()), |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, usize>(1)?))
        })
        .map_err(index_error)?;
    let mut counts = BTreeMap::new();
    for row in rows {
        let (key, count) = row.map_err(index_error)?;
        counts.insert(key, count);
    }
    Ok(counts)
}

pub fn count_tickets_no_index(
    group_by: &str,
    project: Option<&str>,
) -> LiraResult<BTreeMap<String, usize>> {
    let mut counts = BTreeMap::new();
    for ticket in list_tickets(project, None)? {
        let key = match group_by {
            "status" => ticket.status,
            "priority" => ticket.priority,
            other => {
                return Err(LiraError::new(
                    "E_INVALID_GROUP_BY",
                    format!("Unsupported group-by '{other}'."),
                ));
            }
        };
        *counts.entry(key).or_insert(0) += 1;
    }
    Ok(counts)
}

pub fn board_tickets(project: Option<&str>) -> LiraResult<BTreeMap<String, Vec<TicketSummary>>> {
    let mut board: BTreeMap<String, Vec<TicketSummary>> = BTreeMap::new();
    for ticket in query_tickets(TicketQuery {
        project,
        ..TicketQuery::default()
    })? {
        board.entry(ticket.status.clone()).or_default().push(ticket);
    }
    Ok(board)
}

pub fn board_tickets_no_index(
    project: Option<&str>,
) -> LiraResult<BTreeMap<String, Vec<TicketSummary>>> {
    let mut board: BTreeMap<String, Vec<TicketSummary>> = BTreeMap::new();
    for ticket in query_tickets_no_index(TicketQuery {
        project,
        ..TicketQuery::default()
    })? {
        board.entry(ticket.status.clone()).or_default().push(ticket);
    }
    Ok(board)
}

pub fn normalized_issue(id: &str) -> LiraResult<NormalizedIssue> {
    let ticket = read_ticket(id)?;
    normalized_issue_for_ticket(&ticket)
}

pub fn normalized_issues(ids: &[String]) -> LiraResult<Vec<serde_json::Value>> {
    ids.iter()
        .map(|id| match normalized_issue(id) {
            Ok(issue) => Ok(json!({ "id": id, "ok": true, "issue": issue })),
            Err(err) if err.error_code == "E_TICKET_NOT_FOUND" => {
                Ok(json!({ "id": id, "ok": false, "error": err.payload() }))
            }
            Err(err) => Err(err),
        })
        .collect()
}

pub fn candidate_issues(
    project: Option<&str>,
    state: Option<&str>,
) -> LiraResult<Vec<NormalizedIssue>> {
    let mut issues = Vec::new();
    let tickets = list_tickets(project, state)?;
    for ticket in tickets {
        let workflow = read_workflow(&ticket.project)?;
        if !is_candidate(&ticket, &workflow)? {
            continue;
        }
        issues.push(normalized_issue_for_ticket(&ticket)?);
    }
    issues.sort_by(|a, b| {
        a.priority
            .unwrap_or(u8::MAX)
            .cmp(&b.priority.unwrap_or(u8::MAX))
            .then_with(|| a.created_at.cmp(&b.created_at))
            .then_with(|| a.identifier.cmp(&b.identifier))
    });
    Ok(issues)
}

fn is_candidate(ticket: &Ticket, workflow: &Workflow) -> LiraResult<bool> {
    if workflow
        .orchestration
        .terminal_statuses
        .iter()
        .any(|status| status == &ticket.status)
        || workflow.status_terminal(&ticket.status)
    {
        return Ok(false);
    }
    if !workflow
        .orchestration
        .active_statuses
        .iter()
        .any(|status| status == &ticket.status)
    {
        return Ok(false);
    }
    if workflow.orchestration.exclude_claimed && ticket.agent.claimed_by.is_some() {
        return Ok(false);
    }
    if ticket
        .orchestration
        .as_ref()
        .and_then(|metadata| metadata.active_for_dispatch)
        == Some(false)
    {
        return Ok(false);
    }
    if workflow.orchestration.exclude_blocked && ticket.status == "todo" {
        for blocker in &ticket.links.blocked_by {
            if let Ok(blocking_ticket) = read_ticket(blocker) {
                let blocker_workflow = read_workflow(&blocking_ticket.project)?;
                if !blocker_workflow.status_terminal(&blocking_ticket.status) {
                    return Ok(false);
                }
            }
        }
    }
    Ok(true)
}

fn normalized_issue_for_ticket(ticket: &Ticket) -> LiraResult<NormalizedIssue> {
    let blockers = ticket
        .links
        .blocked_by
        .iter()
        .map(|id| match read_ticket(id) {
            Ok(blocker) => BlockerRef {
                id: Some(blocker.id.clone()),
                identifier: Some(blocker.id.clone()),
                state: Some(blocker.status.clone()),
                created_at: Some(blocker.timestamps.created.clone()),
                updated_at: Some(blocker.timestamps.updated.clone()),
            },
            Err(_) => BlockerRef {
                id: None,
                identifier: Some(id.clone()),
                state: None,
                created_at: None,
                updated_at: None,
            },
        })
        .collect();
    Ok(issue_from_ticket(ticket, blockers))
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
    let index = index_doctor_report()?;
    for issue in &index.issues {
        issues.push(format!("index: {issue}"));
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
        index,
        projects,
        issues,
    })
}

fn ensure_index_ready() -> LiraResult<()> {
    if let Some(marker) = read_index_stale_marker()? {
        return Err(index_stale_error(
            marker.reason,
            marker
                .message
                .unwrap_or_else(|| "Index is marked stale.".to_string()),
        ));
    }
    if !index_path()?.exists() {
        reindex()?;
    } else {
        let conn = open_index()?;
        init_index_schema(&conn)?;
        if read_index_schema_version(&conn)? != Some(INDEX_SCHEMA_VERSION)
            || !index_schema_compatible(&conn)?
        {
            drop(conn);
            reindex()?;
            return Ok(());
        }
        let drift = index_drift_issues(&conn)?;
        if !drift.is_empty() {
            let message = drift.join("; ");
            let err = index_stale_error("source drift detected", message.clone());
            let _ = mark_index_stale("source drift detected", Some(&err));
            return Err(err);
        }
    }
    Ok(())
}

fn open_index() -> LiraResult<Connection> {
    init_workspace(false)?;
    let path = index_path()?;
    if let Some(parent) = path.parent() {
        create_dir_all(parent)?;
    }
    Connection::open(path).map_err(index_error)
}

fn open_index_readonly() -> LiraResult<Connection> {
    Connection::open_with_flags(index_path()?, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(index_error)
}

fn reset_index_schema(conn: &Connection) -> LiraResult<()> {
    conn.execute_batch(
        "
        DROP TABLE IF EXISTS ticket_labels;
        DROP TABLE IF EXISTS ticket_task_tags;
        DROP TABLE IF EXISTS ticket_task_statuses;
        DROP TABLE IF EXISTS tickets;
        DROP TABLE IF EXISTS index_meta;
        DROP TABLE IF EXISTS ticket_fts;
        ",
    )
    .map_err(index_error)?;
    init_index_schema(conn)
}

fn init_index_schema(conn: &Connection) -> LiraResult<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS tickets (
            id TEXT PRIMARY KEY,
            project TEXT NOT NULL,
            status TEXT NOT NULL,
            title TEXT NOT NULL,
            description TEXT NOT NULL,
            priority TEXT NOT NULL,
            assignee TEXT,
            claimed_by TEXT,
            parent_type TEXT,
            parent_id TEXT,
            created TEXT NOT NULL,
            updated TEXT NOT NULL,
            source_path TEXT NOT NULL,
            source_mtime INTEGER NOT NULL,
            task_total INTEGER NOT NULL,
            task_todo INTEGER NOT NULL,
            task_in_progress INTEGER NOT NULL,
            task_blocked INTEGER NOT NULL,
            task_done INTEGER NOT NULL,
            task_cancelled INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_tickets_project_status ON tickets(project, status);
        CREATE INDEX IF NOT EXISTS idx_tickets_parent ON tickets(parent_type, parent_id);
        CREATE TABLE IF NOT EXISTS ticket_labels (
            ticket_id TEXT NOT NULL,
            label TEXT NOT NULL,
            PRIMARY KEY(ticket_id, label)
        );
        CREATE INDEX IF NOT EXISTS idx_ticket_labels_label ON ticket_labels(label);
        CREATE TABLE IF NOT EXISTS ticket_task_tags (
            ticket_id TEXT NOT NULL,
            tag TEXT NOT NULL,
            PRIMARY KEY(ticket_id, tag)
        );
        CREATE INDEX IF NOT EXISTS idx_ticket_task_tags_tag ON ticket_task_tags(tag);
        CREATE TABLE IF NOT EXISTS ticket_task_statuses (
            ticket_id TEXT NOT NULL,
            status TEXT NOT NULL,
            count INTEGER NOT NULL,
            PRIMARY KEY(ticket_id, status)
        );
        CREATE VIRTUAL TABLE IF NOT EXISTS ticket_fts USING fts5(
            id UNINDEXED,
            title,
            description,
            acceptance_criteria,
            tasks,
            labels,
            task_tags
        );
        CREATE TABLE IF NOT EXISTS index_meta (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );
        ",
    )
    .map_err(index_error)
}

fn upsert_ticket_index(ticket: &Ticket) -> LiraResult<()> {
    let conn = open_index()?;
    init_index_schema(&conn)?;
    if !index_schema_compatible(&conn)? {
        return Err(index_stale_error(
            "schema incompatible",
            "SQLite index schema is missing source metadata columns.",
        ));
    }
    match read_index_schema_version(&conn)? {
        Some(INDEX_SCHEMA_VERSION) => {}
        Some(version) => {
            return Err(index_stale_error(
                "schema version mismatch",
                format!("SQLite index schema is v{version}, expected v{INDEX_SCHEMA_VERSION}."),
            ));
        }
        None => {
            write_index_schema_version(&conn)?;
        }
    }
    delete_ticket_index(&conn, &ticket.id)?;
    let source_path = ticket_path_for(&ticket.project, &ticket.status, &ticket.id)?;
    insert_ticket_index(&conn, &source_path, ticket)
}

fn delete_ticket_index(conn: &Connection, id: &str) -> LiraResult<()> {
    conn.execute(
        "DELETE FROM ticket_labels WHERE ticket_id = ?1",
        params![id],
    )
    .map_err(index_error)?;
    conn.execute(
        "DELETE FROM ticket_task_tags WHERE ticket_id = ?1",
        params![id],
    )
    .map_err(index_error)?;
    conn.execute(
        "DELETE FROM ticket_task_statuses WHERE ticket_id = ?1",
        params![id],
    )
    .map_err(index_error)?;
    conn.execute("DELETE FROM ticket_fts WHERE id = ?1", params![id])
        .map_err(index_error)?;
    conn.execute("DELETE FROM tickets WHERE id = ?1", params![id])
        .map_err(index_error)?;
    Ok(())
}

fn insert_ticket_index(conn: &Connection, source_path: &Path, ticket: &Ticket) -> LiraResult<()> {
    let summary = TaskSummary::from(ticket);
    let (parent_type, parent_id) = ticket
        .parent
        .as_ref()
        .map(|parent| (Some(parent.parent_type.as_str()), Some(parent.id.as_str())))
        .unwrap_or((None, None));
    let source_path_string = source_path_string(source_path);
    let source_mtime = source_mtime(source_path)?;
    conn.execute(
        "
        INSERT INTO tickets (
            id, project, status, title, description, priority, assignee, claimed_by,
            parent_type, parent_id, created, updated, source_path, source_mtime,
            task_total, task_todo, task_in_progress, task_blocked, task_done, task_cancelled
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20)
        ",
        params![
            ticket.id,
            ticket.project,
            ticket.status,
            ticket.title,
            ticket.description,
            ticket.priority,
            ticket.assignee,
            ticket.agent.claimed_by,
            parent_type,
            parent_id,
            ticket.timestamps.created,
            ticket.timestamps.updated,
            source_path_string,
            source_mtime,
            summary.total,
            summary.todo,
            summary.in_progress,
            summary.blocked,
            summary.done,
            summary.cancelled
        ],
    )
    .map_err(index_error)?;

    for label in &ticket.labels {
        conn.execute(
            "INSERT OR IGNORE INTO ticket_labels(ticket_id, label) VALUES (?1, ?2)",
            params![ticket.id, label],
        )
        .map_err(index_error)?;
    }

    let mut task_tags = Vec::new();
    let mut task_statuses: BTreeMap<&str, usize> = BTreeMap::new();
    for task in &ticket.tasks {
        *task_statuses.entry(task.status.as_str()).or_insert(0) += 1;
        for tag in &task.tags {
            if !task_tags.contains(tag) {
                task_tags.push(tag.clone());
                conn.execute(
                    "INSERT OR IGNORE INTO ticket_task_tags(ticket_id, tag) VALUES (?1, ?2)",
                    params![ticket.id, tag],
                )
                .map_err(index_error)?;
            }
        }
    }
    for (status, count) in task_statuses {
        conn.execute(
            "INSERT INTO ticket_task_statuses(ticket_id, status, count) VALUES (?1, ?2, ?3)",
            params![ticket.id, status, count],
        )
        .map_err(index_error)?;
    }

    conn.execute(
        "
        INSERT INTO ticket_fts(id, title, description, acceptance_criteria, tasks, labels, task_tags)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
        ",
        params![
            ticket.id,
            ticket.title,
            ticket.description,
            ticket.acceptance_criteria.join("\n"),
            ticket
                .tasks
                .iter()
                .map(|task| task.title.as_str())
                .collect::<Vec<_>>()
                .join("\n"),
            ticket.labels.join(" "),
            task_tags.join(" ")
        ],
    )
    .map_err(index_error)?;
    Ok(())
}

fn ticket_search_text(ticket: &Ticket) -> String {
    let mut text = String::new();
    text.push_str(&ticket.id);
    text.push('\n');
    text.push_str(&ticket.title);
    text.push('\n');
    text.push_str(&ticket.description);
    text.push('\n');
    text.push_str(&ticket.acceptance_criteria.join("\n"));
    text.push('\n');
    for task in &ticket.tasks {
        text.push_str(&task.title);
        text.push('\n');
        text.push_str(&task.tags.join(" "));
        text.push('\n');
    }
    text.push_str(&ticket.labels.join(" "));
    text
}

fn ticket_matches_query(ticket: &Ticket, filters: &TicketQuery<'_>) -> bool {
    filters
        .label
        .is_none_or(|label| ticket.labels.iter().any(|value| value == label))
        && filters
            .assignee
            .is_none_or(|assignee| ticket.assignee.as_deref() == Some(assignee))
        && filters
            .task_status
            .is_none_or(|status| ticket.tasks.iter().any(|task| task.status == status))
        && filters.task_tag.is_none_or(|tag| {
            ticket
                .tasks
                .iter()
                .any(|task| task.tags.iter().any(|value| value == tag))
        })
        && filters.parent_jira.is_none_or(|parent_jira| {
            ticket
                .parent
                .as_ref()
                .is_some_and(|parent| parent.parent_type == "jira" && parent.id == parent_jira)
        })
}

fn fts_query(query: &str) -> LiraResult<String> {
    let terms: Vec<String> = query
        .split_whitespace()
        .filter_map(|term| {
            let normalized = term
                .chars()
                .map(|ch| {
                    if ch.is_alphanumeric() || ch == '_' {
                        ch
                    } else {
                        ' '
                    }
                })
                .collect::<String>()
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ");
            if normalized.is_empty() {
                None
            } else {
                Some(format!("\"{}\"", normalized.replace('"', " ")))
            }
        })
        .collect();
    if terms.is_empty() {
        return Err(LiraError::new(
            "E_QUERY_REQUIRED",
            "Search query must contain at least one searchable term.",
        ));
    }
    Ok(terms.join(" "))
}

fn read_index_schema_version(conn: &Connection) -> LiraResult<Option<u32>> {
    let value = conn
        .query_row(
            "SELECT value FROM index_meta WHERE key = 'schema_version'",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(index_error)?;
    value
        .map(|value| {
            value.parse::<u32>().map_err(|err| {
                LiraError::new(
                    "E_INDEX",
                    format!("Invalid index schema version '{value}'."),
                )
                .details(json!({ "error": err.to_string() }))
                .suggestion("lira reindex --json", "rebuild index")
            })
        })
        .transpose()
}

fn write_index_schema_version(conn: &Connection) -> LiraResult<()> {
    conn.execute(
        "INSERT OR REPLACE INTO index_meta(key, value) VALUES ('schema_version', ?1)",
        params![INDEX_SCHEMA_VERSION.to_string()],
    )
    .map_err(index_error)?;
    Ok(())
}

fn index_schema_compatible(conn: &Connection) -> LiraResult<bool> {
    let mut stmt = conn
        .prepare("PRAGMA table_info(tickets)")
        .map_err(index_error)?;
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(index_error)?;
    let mut columns = Vec::new();
    for row in rows {
        columns.push(row.map_err(index_error)?);
    }
    Ok(columns.iter().any(|column| column == "source_path")
        && columns.iter().any(|column| column == "source_mtime"))
}

fn index_drift_issues(conn: &Connection) -> LiraResult<Vec<String>> {
    let expected_files = list_ticket_files(None, None)?;
    let mut expected = BTreeMap::new();
    for (path, ticket) in expected_files {
        expected.insert(ticket.id, (source_path_string(&path), source_mtime(&path)?));
    }

    let mut stmt = conn
        .prepare("SELECT id, source_path, source_mtime FROM tickets ORDER BY id")
        .map_err(index_error)?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
            ))
        })
        .map_err(index_error)?;
    let mut indexed = BTreeMap::new();
    for row in rows {
        let (id, path, mtime) = row.map_err(index_error)?;
        indexed.insert(id, (path, mtime));
    }

    let mut issues = Vec::new();
    for (id, (indexed_path, indexed_mtime)) in &indexed {
        match expected.get(id) {
            Some((source_path, source_mtime)) => {
                if indexed_path != source_path {
                    issues.push(format!(
                        "{id}: source path changed from {indexed_path} to {source_path}"
                    ));
                } else if indexed_mtime != source_mtime {
                    issues.push(format!("{id}: source mtime changed for {source_path}"));
                }
            }
            None => issues.push(format!(
                "{id}: indexed source {indexed_path} no longer exists"
            )),
        }
    }
    for (id, (source_path, _)) in &expected {
        if !indexed.contains_key(id) {
            issues.push(format!("{id}: missing index row for {source_path}"));
        }
    }
    Ok(issues)
}

fn index_doctor_report() -> LiraResult<IndexDoctorReport> {
    let path = index_path()?;
    let exists = path.exists();
    let stale_marker = read_index_stale_marker()?;
    let mut issues = Vec::new();
    if let Some(marker) = &stale_marker {
        issues.push(format!("stale marker: {}", marker.reason));
    }

    let mut schema_version = None;
    let mut tickets_indexed = None;
    if exists {
        match open_index_readonly() {
            Ok(conn) => {
                match read_index_schema_version(&conn) {
                    Ok(version) => {
                        schema_version = version;
                        if version != Some(INDEX_SCHEMA_VERSION) {
                            issues.push(format!(
                                "schema version {:?}, expected {}",
                                version, INDEX_SCHEMA_VERSION
                            ));
                        }
                    }
                    Err(err) => issues.push(format!("schema version error: {}", err.message)),
                }
                match index_schema_compatible(&conn) {
                    Ok(true) => {}
                    Ok(false) => issues.push("schema missing source metadata columns".to_string()),
                    Err(err) => issues.push(format!("schema error: {}", err.message)),
                }
                match conn.query_row("SELECT COUNT(*) FROM tickets", [], |row| {
                    row.get::<_, usize>(0)
                }) {
                    Ok(count) => tickets_indexed = Some(count),
                    Err(err) => issues.push(format!("ticket count error: {err}")),
                }
                match index_drift_issues(&conn) {
                    Ok(drift) => issues.extend(drift),
                    Err(err) => issues.push(format!("drift check error: {}", err.message)),
                }
            }
            Err(err) => issues.push(format!("open error: {}", err.message)),
        }
    }

    Ok(IndexDoctorReport {
        path,
        exists,
        stale: stale_marker.is_some() || !issues.is_empty(),
        stale_marker,
        schema_version,
        tickets_indexed,
        issues,
    })
}

fn mark_index_stale(reason: impl Into<String>, err: Option<&LiraError>) -> LiraResult<()> {
    let marker = IndexStaleMarker {
        schema_version: INDEX_SCHEMA_VERSION,
        reason: reason.into(),
        error_code: err.map(|err| err.error_code.clone()),
        message: err.map(|err| err.message.clone()),
        marked_at: lira_core::now_string(),
    };
    let path = index_stale_path()?;
    let bytes = serde_json::to_vec_pretty(&marker)
        .map_err(|err| LiraError::new("E_JSON_SERIALIZE", err.to_string()))?;
    atomic_write(&path, &bytes)
}

fn clear_index_stale_marker() -> LiraResult<()> {
    let path = index_stale_path()?;
    if path.exists() {
        std::fs::remove_file(path).map_err(filesystem_error)?;
    }
    Ok(())
}

fn read_index_stale_marker() -> LiraResult<Option<IndexStaleMarker>> {
    let path = index_stale_path()?;
    if !path.exists() {
        return Ok(None);
    }
    read_yaml(&path).map(Some)
}

fn source_path_string(path: &Path) -> String {
    path.to_string_lossy().to_string()
}

fn source_mtime(path: &Path) -> LiraResult<i64> {
    let modified = std::fs::metadata(path)
        .map_err(filesystem_error)?
        .modified()
        .map_err(filesystem_error)?;
    let duration = modified.duration_since(UNIX_EPOCH).map_err(|err| {
        LiraError::new(
            "E_FILESYSTEM",
            format!(
                "File mtime for {} is before the Unix epoch.",
                path.display()
            ),
        )
        .details(json!({ "error": err.to_string() }))
    })?;
    Ok(duration.as_millis() as i64)
}

fn query_ids(conn: &Connection, sql: &str, values: Vec<String>) -> LiraResult<Vec<String>> {
    let mut stmt = conn.prepare(sql).map_err(index_error)?;
    let rows = stmt
        .query_map(params_from_iter(values.iter()), |row| {
            row.get::<_, String>(0)
        })
        .map_err(index_error)?;
    let mut ids = Vec::new();
    for row in rows {
        ids.push(row.map_err(index_error)?);
    }
    Ok(ids)
}

fn summaries_from_ids(ids: Vec<String>) -> LiraResult<Vec<TicketSummary>> {
    ids.into_iter()
        .map(|id| read_ticket(&id).map(|ticket| TicketSummary::from(&ticket)))
        .collect()
}

fn index_error(err: impl std::fmt::Display) -> LiraError {
    LiraError::new("E_INDEX", err.to_string()).suggestion("lira reindex --json", "rebuild index")
}

fn index_stale_error(reason: impl Into<String>, message: impl Into<String>) -> LiraError {
    let reason = reason.into();
    let message = message.into();
    LiraError::new("E_INDEX_STALE", format!("SQLite index is stale: {message}"))
        .details(json!({ "reason": reason }))
        .suggestion("lira reindex --json", "rebuild index from canonical YAML")
        .suggestion(
            "lira search <query> --no-index --json",
            "read canonical YAML without using the SQLite cache",
        )
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
