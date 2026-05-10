use chrono::{SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const SCHEMA_VERSION: u32 = 3;

pub fn now_string() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Suggestion {
    pub command: String,
    pub reason: String,
}

impl Suggestion {
    pub fn new(command: impl Into<String>, reason: impl Into<String>) -> Self {
        Self {
            command: command.into(),
            reason: reason.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorPayload {
    pub error_code: String,
    pub message: String,
    pub details: serde_json::Value,
    pub suggestions: Vec<Suggestion>,
}

#[derive(Debug, Clone, Serialize)]
pub struct JsonEnvelope<T: Serialize> {
    pub schema_version: u32,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorPayload>,
}

impl<T: Serialize> JsonEnvelope<T> {
    pub fn ok(result: T) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            ok: true,
            result: Some(result),
            error: None,
        }
    }
}

impl JsonEnvelope<()> {
    pub fn err(error: ErrorPayload) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            ok: false,
            result: None,
            error: Some(error),
        }
    }
}

#[derive(Debug, Error, Clone)]
#[error("{message}")]
pub struct LiraError {
    pub error_code: String,
    pub message: String,
    pub details: serde_json::Value,
    pub suggestions: Vec<Suggestion>,
    pub exit_code: i32,
}

impl LiraError {
    pub fn new(error_code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            error_code: error_code.into(),
            message: message.into(),
            details: json!({}),
            suggestions: Vec::new(),
            exit_code: 1,
        }
    }

    pub fn details(mut self, details: serde_json::Value) -> Self {
        self.details = details;
        self
    }

    pub fn suggestion(mut self, command: impl Into<String>, reason: impl Into<String>) -> Self {
        self.suggestions.push(Suggestion::new(command, reason));
        self
    }

    pub fn exit_code(mut self, exit_code: i32) -> Self {
        self.exit_code = exit_code;
        self
    }

    pub fn payload(&self) -> ErrorPayload {
        ErrorPayload {
            error_code: self.error_code.clone(),
            message: self.message.clone(),
            details: self.details.clone(),
            suggestions: self.suggestions.clone(),
        }
    }
}

pub type LiraResult<T> = Result<T, LiraError>;

pub fn filesystem_error(err: impl std::fmt::Display) -> LiraError {
    LiraError::new("E_FILESYSTEM", err.to_string())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub schema_version: u32,
    pub key: String,
    pub name: String,
    pub default_status: String,
}

impl Project {
    pub fn new(key: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            key: key.into(),
            name: name.into(),
            default_status: "backlog".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Counters {
    pub schema_version: u32,
    pub project: String,
    pub next_ticket: u64,
}

impl Counters {
    pub fn new(project: impl Into<String>) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            project: project.into(),
            next_ticket: 1,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workflow {
    pub schema_version: u32,
    pub project: String,
    pub default_status: String,
    pub statuses: Vec<StatusDef>,
    pub task_statuses: Vec<StatusDef>,
    pub allowed_transitions: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    pub orchestration: OrchestrationPolicy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusDef {
    pub id: String,
    pub name: String,
    pub terminal: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrchestrationPolicy {
    pub active_statuses: Vec<String>,
    pub terminal_statuses: Vec<String>,
    pub handoff_statuses: Vec<String>,
    pub exclude_claimed: bool,
    pub exclude_blocked: bool,
}

impl Default for OrchestrationPolicy {
    fn default() -> Self {
        Self {
            active_statuses: vec!["todo".to_string(), "in-progress".to_string()],
            terminal_statuses: vec![
                "done".to_string(),
                "cancelled".to_string(),
                "archived".to_string(),
            ],
            handoff_statuses: vec!["in-review".to_string()],
            exclude_claimed: true,
            exclude_blocked: true,
        }
    }
}

impl Workflow {
    pub fn default_for(project: impl Into<String>) -> Self {
        let project = project.into();
        let statuses = vec![
            StatusDef::new("backlog", "Backlog", false),
            StatusDef::new("todo", "To Do", false),
            StatusDef::new("in-progress", "In Progress", false),
            StatusDef::new("blocked", "Blocked", false),
            StatusDef::new("in-review", "In Review", false),
            StatusDef::new("done", "Done", true),
            StatusDef::new("cancelled", "Cancelled", true),
            StatusDef::new("archived", "Archived", true),
        ];
        let task_statuses = vec![
            StatusDef::new("todo", "To Do", false),
            StatusDef::new("in-progress", "In Progress", false),
            StatusDef::new("blocked", "Blocked", false),
            StatusDef::new("done", "Done", true),
            StatusDef::new("cancelled", "Cancelled", true),
        ];
        let allowed_transitions = BTreeMap::from([
            (
                "backlog".to_string(),
                vec![
                    "todo".to_string(),
                    "cancelled".to_string(),
                    "archived".to_string(),
                ],
            ),
            (
                "todo".to_string(),
                vec![
                    "in-progress".to_string(),
                    "blocked".to_string(),
                    "cancelled".to_string(),
                    "archived".to_string(),
                ],
            ),
            (
                "in-progress".to_string(),
                vec![
                    "in-review".to_string(),
                    "blocked".to_string(),
                    "todo".to_string(),
                    "done".to_string(),
                ],
            ),
            (
                "blocked".to_string(),
                vec![
                    "todo".to_string(),
                    "in-progress".to_string(),
                    "cancelled".to_string(),
                ],
            ),
            (
                "in-review".to_string(),
                vec!["done".to_string(), "in-progress".to_string()],
            ),
            ("done".to_string(), vec!["archived".to_string()]),
            ("cancelled".to_string(), vec!["archived".to_string()]),
            ("archived".to_string(), vec![]),
        ]);

        Self {
            schema_version: SCHEMA_VERSION,
            project,
            default_status: "backlog".to_string(),
            statuses,
            task_statuses,
            allowed_transitions,
            orchestration: OrchestrationPolicy::default(),
        }
    }

    pub fn has_status(&self, status: &str) -> bool {
        self.statuses.iter().any(|s| s.id == status)
    }

    pub fn has_task_status(&self, status: &str) -> bool {
        self.task_statuses.iter().any(|s| s.id == status)
    }

    pub fn status_terminal(&self, status: &str) -> bool {
        self.statuses
            .iter()
            .find(|s| s.id == status)
            .map(|s| s.terminal)
            .unwrap_or(false)
    }

    pub fn task_status_terminal(&self, status: &str) -> bool {
        self.task_statuses
            .iter()
            .find(|s| s.id == status)
            .map(|s| s.terminal)
            .unwrap_or(false)
    }

    pub fn can_transition(&self, from: &str, to: &str) -> bool {
        from == to
            || self
                .allowed_transitions
                .get(from)
                .map(|targets| targets.iter().any(|target| target == to))
                .unwrap_or(false)
    }
}

impl StatusDef {
    pub fn new(id: impl Into<String>, name: impl Into<String>, terminal: bool) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            terminal,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ticket {
    pub schema_version: u32,
    pub id: String,
    pub project: String,
    pub title: String,
    pub description: String,
    #[serde(rename = "type")]
    pub ticket_type: String,
    pub status: String,
    pub priority: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assignee: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reporter: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<ParentRef>,
    pub labels: Vec<String>,
    pub acceptance_criteria: Vec<String>,
    pub tasks: Vec<Task>,
    pub links: Links,
    pub comments: Vec<Comment>,
    pub history: Vec<HistoryEvent>,
    pub timestamps: Timestamps,
    pub agent: AgentMetadata,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub orchestration: Option<OrchestrationMetadata>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub github: Option<GithubBinding>,
}

impl Ticket {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: impl Into<String>,
        project: impl Into<String>,
        title: impl Into<String>,
        description: impl Into<String>,
        ticket_type: impl Into<String>,
        priority: impl Into<String>,
        assignee: Option<String>,
        reporter: Option<String>,
        parent: Option<ParentRef>,
        acceptance_criteria: Vec<String>,
        task_titles: Vec<String>,
        actor: Option<String>,
    ) -> Self {
        let now = now_string();
        let id = id.into();
        let project = project.into();
        let actor = actor.unwrap_or_else(|| "lira".to_string());
        let tasks = task_titles
            .into_iter()
            .enumerate()
            .map(|(idx, title)| Task {
                id: format!("T{}", idx + 1),
                title,
                status: "todo".to_string(),
                tags: Vec::new(),
                created_on: now.clone(),
                last_modified: now.clone(),
            })
            .collect();
        let mut ticket = Self {
            schema_version: SCHEMA_VERSION,
            id: id.clone(),
            project,
            title: title.into(),
            description: description.into(),
            ticket_type: ticket_type.into(),
            status: "backlog".to_string(),
            priority: priority.into(),
            assignee,
            reporter,
            parent,
            labels: Vec::new(),
            acceptance_criteria,
            tasks,
            links: Links::default(),
            comments: Vec::new(),
            history: Vec::new(),
            timestamps: Timestamps {
                created: now.clone(),
                updated: now,
            },
            agent: AgentMetadata::default(),
            orchestration: None,
            github: None,
        };
        ticket.add_history("created", format!("Created {id}"), Some(actor));
        ticket
    }

    pub fn touch(&mut self) {
        self.timestamps.updated = now_string();
    }

    pub fn add_history(
        &mut self,
        action: impl Into<String>,
        message: impl Into<String>,
        actor: Option<String>,
    ) {
        let seq = self.history.len() + 1;
        self.history.push(HistoryEvent {
            id: format!("h{seq}"),
            action: action.into(),
            message: message.into(),
            actor,
            timestamp: now_string(),
        });
    }

    pub fn next_comment_id(&self) -> String {
        format!("local-c{}", self.comments.len() + 1)
    }

    pub fn next_task_id(&self) -> String {
        let max = self
            .tasks
            .iter()
            .filter_map(|task| task.id.strip_prefix('T')?.parse::<u64>().ok())
            .max()
            .unwrap_or(0);
        format!("T{}", max + 1)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Task {
    pub id: String,
    pub title: String,
    pub status: String,
    pub tags: Vec<String>,
    pub created_on: String,
    pub last_modified: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParentRef {
    #[serde(rename = "type")]
    pub parent_type: String,
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

impl ParentRef {
    pub fn jira(key: impl Into<String>) -> Self {
        Self {
            parent_type: "jira".to_string(),
            id: key.into(),
            url: None,
            title: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Links {
    pub blocks: Vec<String>,
    pub blocked_by: Vec<String>,
    pub relates_to: Vec<String>,
    pub duplicates: Vec<String>,
    pub child_tickets: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Comment {
    pub id: String,
    pub body: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    pub created_on: String,
    pub sync_github: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub github_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEvent {
    pub id: String,
    pub action: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actor: Option<String>,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Timestamps {
    pub created: String,
    pub updated: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AgentMetadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub claimed_by: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub claimed_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OrchestrationMetadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace_hint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_for_dispatch: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_claimed_by: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_claimed_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_released_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GithubBinding {
    pub repo: String,
    pub issue: u64,
    pub url: String,
    pub sync_state: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TicketSummary {
    pub id: String,
    pub project: String,
    pub title: String,
    pub status: String,
    pub priority: String,
    pub assignee: Option<String>,
    pub claimed_by: Option<String>,
    pub task_summary: TaskSummary,
}

impl From<&Ticket> for TicketSummary {
    fn from(ticket: &Ticket) -> Self {
        Self {
            id: ticket.id.clone(),
            project: ticket.project.clone(),
            title: ticket.title.clone(),
            status: ticket.status.clone(),
            priority: ticket.priority.clone(),
            assignee: ticket.assignee.clone(),
            claimed_by: ticket.agent.claimed_by.clone(),
            task_summary: TaskSummary::from(ticket),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskSummary {
    pub total: usize,
    pub todo: usize,
    pub in_progress: usize,
    pub blocked: usize,
    pub done: usize,
    pub cancelled: usize,
}

impl From<&Ticket> for TaskSummary {
    fn from(ticket: &Ticket) -> Self {
        Self {
            total: ticket.tasks.len(),
            todo: ticket.tasks.iter().filter(|t| t.status == "todo").count(),
            in_progress: ticket
                .tasks
                .iter()
                .filter(|t| t.status == "in-progress")
                .count(),
            blocked: ticket
                .tasks
                .iter()
                .filter(|t| t.status == "blocked")
                .count(),
            done: ticket.tasks.iter().filter(|t| t.status == "done").count(),
            cancelled: ticket
                .tasks
                .iter()
                .filter(|t| t.status == "cancelled")
                .count(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockerRef {
    pub id: Option<String>,
    pub identifier: Option<String>,
    pub state: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalizedIssue {
    pub id: String,
    pub identifier: String,
    pub title: String,
    pub description: Option<String>,
    pub priority: Option<u8>,
    pub state: String,
    pub branch_name: Option<String>,
    pub url: Option<String>,
    pub labels: Vec<String>,
    pub blocked_by: Vec<BlockerRef>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

pub fn priority_value(priority: &str) -> Option<u8> {
    match priority {
        "highest" => Some(1),
        "high" => Some(2),
        "medium" => Some(3),
        "low" => Some(4),
        "lowest" => Some(5),
        _ => None,
    }
}

pub fn issue_from_ticket(ticket: &Ticket, blockers: Vec<BlockerRef>) -> NormalizedIssue {
    let mut labels = ticket
        .labels
        .iter()
        .map(|label| label.to_ascii_lowercase())
        .collect::<Vec<_>>();
    labels.sort();
    labels.dedup();
    NormalizedIssue {
        id: ticket.id.clone(),
        identifier: ticket.id.clone(),
        title: ticket.title.clone(),
        description: if ticket.description.trim().is_empty() {
            None
        } else {
            Some(ticket.description.clone())
        },
        priority: priority_value(&ticket.priority),
        state: ticket.status.clone(),
        branch_name: ticket
            .orchestration
            .as_ref()
            .and_then(|metadata| metadata.branch_name.clone()),
        url: ticket.github.as_ref().map(|github| github.url.clone()),
        labels,
        blocked_by: blockers,
        created_at: Some(ticket.timestamps.created.clone()),
        updated_at: Some(ticket.timestamps.updated.clone()),
    }
}

pub fn validate_project_key(key: &str) -> LiraResult<()> {
    let valid = !key.is_empty()
        && key.len() <= 16
        && key
            .chars()
            .all(|ch| ch.is_ascii_uppercase() || ch.is_ascii_digit() || ch == '-')
        && key.chars().next().is_some_and(|ch| ch.is_ascii_uppercase());
    if valid {
        Ok(())
    } else {
        Err(LiraError::new(
            "E_INVALID_PROJECT_KEY",
            "Project keys must start with an uppercase letter and contain only A-Z, 0-9, or '-'.",
        ))
    }
}

pub fn validate_acceptance_criteria(criteria: &[String]) -> LiraResult<()> {
    if criteria
        .iter()
        .any(|criterion| !criterion.trim().is_empty())
    {
        Ok(())
    } else {
        Err(LiraError::new(
            "E_ACCEPTANCE_CRITERIA_REQUIRED",
            "At least one non-empty acceptance criterion is required.",
        ))
    }
}

pub fn validate_tasks(tasks: &[Task], workflow: &Workflow) -> LiraResult<()> {
    if tasks.is_empty() {
        return Err(LiraError::new(
            "E_TASK_REQUIRED",
            "At least one task is required.",
        ));
    }

    let mut seen = BTreeSet::new();
    for task in tasks {
        if task.title.trim().is_empty() {
            return Err(LiraError::new(
                "E_INVALID_TASK_SCHEMA",
                "Task titles must be non-empty.",
            ));
        }
        if !seen.insert(task.id.clone()) {
            return Err(LiraError::new(
                "E_INVALID_TASK_SCHEMA",
                format!("Duplicate task id '{}'.", task.id),
            ));
        }
        if !workflow.has_task_status(&task.status) {
            return Err(LiraError::new(
                "E_INVALID_TASK_STATUS",
                format!("Invalid task status '{}'.", task.status),
            ));
        }
    }
    Ok(())
}

pub fn validate_ticket(ticket: &Ticket, workflow: &Workflow) -> LiraResult<()> {
    validate_acceptance_criteria(&ticket.acceptance_criteria)?;
    validate_tasks(&ticket.tasks, workflow)?;
    if !workflow.has_status(&ticket.status) {
        return Err(LiraError::new(
            "E_INVALID_STATUS",
            format!("Invalid ticket status '{}'.", ticket.status),
        ));
    }
    Ok(())
}

pub fn validate_transition(workflow: &Workflow, from: &str, to: &str) -> LiraResult<()> {
    if !workflow.has_status(to) {
        return Err(LiraError::new(
            "E_INVALID_STATUS",
            format!("Unknown target status '{to}'."),
        ));
    }
    if workflow.can_transition(from, to) {
        Ok(())
    } else {
        Err(LiraError::new(
            "E_INVALID_TRANSITION",
            format!("Cannot move from '{from}' to '{to}'."),
        ))
    }
}

pub fn validate_completion_policy(ticket: &Ticket, workflow: &Workflow) -> LiraResult<()> {
    validate_acceptance_criteria(&ticket.acceptance_criteria)?;
    let open: Vec<String> = ticket
        .tasks
        .iter()
        .filter(|task| !workflow.task_status_terminal(&task.status))
        .map(|task| task.id.clone())
        .collect();
    if open.is_empty() {
        Ok(())
    } else {
        Err(LiraError::new(
            "E_COMPLETION_POLICY",
            "All tasks must be done or cancelled before moving a ticket to done.",
        )
        .details(json!({ "open_tasks": open }))
        .suggestion(
            format!("lira task status {} <TASK_ID> done --json", ticket.id),
            "complete remaining tasks first",
        ))
    }
}

pub fn normalize_nonempty(values: Vec<String>) -> Vec<String> {
    values
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completion_policy_rejects_open_tasks() {
        let workflow = Workflow::default_for("ORION");
        let ticket = Ticket::new(
            "ORION-1",
            "ORION",
            "Title",
            "",
            "task",
            "medium",
            None,
            None,
            None,
            vec!["It works".to_string()],
            vec!["Do it".to_string()],
            None,
        );
        let err = validate_completion_policy(&ticket, &workflow).unwrap_err();
        assert_eq!(err.error_code, "E_COMPLETION_POLICY");
    }

    #[test]
    fn issue_projection_maps_priority() {
        let ticket = Ticket::new(
            "ORION-1",
            "ORION",
            "Title",
            "Body",
            "task",
            "high",
            None,
            None,
            None,
            vec!["It works".to_string()],
            vec!["Do it".to_string()],
            None,
        );
        let issue = issue_from_ticket(&ticket, Vec::new());
        assert_eq!(issue.priority, Some(2));
        assert_eq!(issue.identifier, "ORION-1");
    }
}
