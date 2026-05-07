use clap::{Parser, Subcommand};
use lira_core::{
    normalize_nonempty, now_string, validate_completion_policy, validate_project_key,
    validate_transition, Comment, HistoryEvent, JsonEnvelope, LiraError, LiraResult, ParentRef,
    Task, Ticket, TicketSummary, SCHEMA_VERSION,
};
use lira_store::JsonlEvent;
use serde::Serialize;
use serde_json::json;
use std::io::Read;
use std::path::Path;

#[derive(Parser, Debug)]
#[command(
    name = "lira",
    version,
    about = "Local-first issue tracking for agents"
)]
struct Cli {
    #[arg(long, global = true)]
    json: bool,
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    Init {
        #[arg(long)]
        dry_run: bool,
    },
    Doctor,
    Validate,
    Project {
        #[command(subcommand)]
        command: ProjectCommands,
    },
    #[command(name = "new")]
    New(NewArgs),
    Show {
        id: String,
    },
    Ls {
        #[arg(long)]
        project: Option<String>,
        #[arg(long)]
        status: Option<String>,
    },
    Search {
        query: String,
        #[arg(long)]
        project: Option<String>,
    },
    Query(QueryArgs),
    Count {
        #[arg(long = "group-by", default_value = "status")]
        group_by: String,
        #[arg(long)]
        project: Option<String>,
    },
    Board {
        #[arg(long)]
        project: Option<String>,
    },
    Mv {
        id: String,
        status: String,
        #[arg(long)]
        force: bool,
    },
    Task {
        #[command(subcommand)]
        command: TaskCommands,
    },
    Comment {
        id: String,
        body: Option<String>,
        #[arg(long)]
        stdin: bool,
        #[arg(long)]
        author: Option<String>,
    },
    History {
        #[command(subcommand)]
        command: HistoryCommands,
    },
    Claim {
        id: String,
        #[arg(long)]
        agent: String,
        #[arg(long)]
        force: bool,
        #[arg(long)]
        reason: Option<String>,
    },
    Release {
        id: String,
        #[arg(long)]
        agent: Option<String>,
    },
    Active {
        #[arg(long)]
        agent: String,
    },
    Next {
        #[arg(long)]
        project: Option<String>,
        #[arg(long)]
        agent: Option<String>,
    },
    Candidates {
        #[arg(long)]
        project: Option<String>,
        #[arg(long = "state")]
        state: Option<String>,
        #[arg(long)]
        limit: Option<usize>,
    },
    Issue {
        #[command(subcommand)]
        command: IssueCommands,
    },
    Workflow {
        #[command(subcommand)]
        command: WorkflowCommands,
    },
    Label {
        #[command(subcommand)]
        command: LabelCommands,
    },
    Link {
        id: String,
        #[arg(long)]
        jira: Option<String>,
        #[arg(long)]
        blocks: Vec<String>,
        #[arg(long = "blocked-by")]
        blocked_by: Vec<String>,
        #[arg(long = "relates-to")]
        relates_to: Vec<String>,
        #[arg(long)]
        duplicates: Vec<String>,
        #[arg(long = "child")]
        child_tickets: Vec<String>,
    },
}

#[derive(Parser, Debug)]
struct NewArgs {
    title: String,
    #[arg(long)]
    project: String,
    #[arg(long, default_value = "")]
    description: String,
    #[arg(long = "description-stdin")]
    description_stdin: bool,
    #[arg(long = "acceptance-criterion", required = true)]
    acceptance_criteria: Vec<String>,
    #[arg(long, required = true)]
    task: Vec<String>,
    #[arg(long = "parent-jira")]
    parent_jira: Option<String>,
    #[arg(long = "type", default_value = "task")]
    ticket_type: String,
    #[arg(long, default_value = "medium")]
    priority: String,
    #[arg(long)]
    assignee: Option<String>,
    #[arg(long)]
    reporter: Option<String>,
    #[arg(long)]
    actor: Option<String>,
}

#[derive(Parser, Debug)]
struct QueryArgs {
    #[arg(long)]
    project: Option<String>,
    #[arg(long)]
    status: Option<String>,
    #[arg(long)]
    label: Option<String>,
    #[arg(long)]
    assignee: Option<String>,
    #[arg(long = "task-status")]
    task_status: Option<String>,
    #[arg(long = "task-tag")]
    task_tag: Option<String>,
    #[arg(long = "parent-jira")]
    parent_jira: Option<String>,
}

#[derive(Subcommand, Debug)]
enum ProjectCommands {
    List,
    Create { key: String, name: String },
    Show { key: String },
}

#[derive(Subcommand, Debug)]
enum TaskCommands {
    List {
        id: String,
    },
    Add {
        id: String,
        title: String,
        #[arg(long)]
        tag: Vec<String>,
    },
    Status {
        id: String,
        task: String,
        status: String,
    },
    Done {
        id: String,
        task: String,
    },
    Cancel {
        id: String,
        task: String,
    },
}

#[derive(Subcommand, Debug)]
enum HistoryCommands {
    List {
        id: String,
    },
    Add {
        id: String,
        #[arg(long)]
        action: String,
        #[arg(long)]
        message: String,
        #[arg(long)]
        actor: Option<String>,
    },
}

#[derive(Subcommand, Debug)]
enum IssueCommands {
    Show {
        id: String,
    },
    Current {
        #[arg(long = "ids", required = true)]
        ids: Vec<String>,
    },
}

#[derive(Subcommand, Debug)]
enum WorkflowCommands {
    Symphony {
        #[command(subcommand)]
        command: SymphonyWorkflowCommands,
    },
}

#[derive(Subcommand, Debug)]
enum SymphonyWorkflowCommands {
    Export {
        #[arg(long)]
        project: String,
    },
    Validate {
        path: String,
        #[arg(long)]
        project: String,
    },
}

#[derive(Subcommand, Debug)]
enum LabelCommands {
    Add { id: String, label: String },
    Remove { id: String, label: String },
}

fn main() {
    let cli = Cli::parse();
    let json = cli.json;
    if let Err(err) = run(cli) {
        if json {
            let _ = print_json(JsonEnvelope::err(err.payload()));
        } else {
            eprintln!("{err}");
            for suggestion in &err.suggestions {
                eprintln!("suggestion: {} ({})", suggestion.command, suggestion.reason);
            }
        }
        std::process::exit(err.exit_code);
    }
}

fn run(cli: Cli) -> LiraResult<()> {
    match cli.command {
        Some(Commands::Init { dry_run }) => cmd_init(cli.json, dry_run),
        Some(Commands::Doctor) | Some(Commands::Validate) => cmd_doctor(cli.json),
        Some(Commands::Project { command }) => match command {
            ProjectCommands::List => cmd_project_list(cli.json),
            ProjectCommands::Create { key, name } => cmd_project_create(cli.json, &key, &name),
            ProjectCommands::Show { key } => cmd_project_show(cli.json, &key),
        },
        Some(Commands::New(args)) => cmd_new(cli.json, args),
        Some(Commands::Show { id }) => cmd_show(cli.json, &id),
        Some(Commands::Ls { project, status }) => {
            cmd_ls(cli.json, project.as_deref(), status.as_deref())
        }
        Some(Commands::Search { query, project }) => {
            cmd_search(cli.json, &query, project.as_deref())
        }
        Some(Commands::Query(args)) => cmd_query(cli.json, args),
        Some(Commands::Count { group_by, project }) => {
            cmd_count(cli.json, &group_by, project.as_deref())
        }
        Some(Commands::Board { project }) => cmd_board(cli.json, project.as_deref()),
        Some(Commands::Mv { id, status, force }) => cmd_mv(cli.json, &id, &status, force),
        Some(Commands::Task { command }) => cmd_task(cli.json, command),
        Some(Commands::Comment {
            id,
            body,
            stdin,
            author,
        }) => cmd_comment(cli.json, &id, body, stdin, author),
        Some(Commands::History { command }) => cmd_history(cli.json, command),
        Some(Commands::Claim {
            id,
            agent,
            force,
            reason,
        }) => cmd_claim(cli.json, &id, &agent, force, reason),
        Some(Commands::Release { id, agent }) => cmd_release(cli.json, &id, agent),
        Some(Commands::Active { agent }) => cmd_active(cli.json, &agent),
        Some(Commands::Next { project, agent }) => {
            cmd_next(cli.json, project.as_deref(), agent.as_deref())
        }
        Some(Commands::Candidates {
            project,
            state,
            limit,
        }) => cmd_candidates(cli.json, project.as_deref(), state.as_deref(), limit),
        Some(Commands::Issue { command }) => cmd_issue(cli.json, command),
        Some(Commands::Workflow { command }) => cmd_workflow(cli.json, command),
        Some(Commands::Label { command }) => cmd_label(cli.json, command),
        Some(Commands::Link {
            id,
            jira,
            blocks,
            blocked_by,
            relates_to,
            duplicates,
            child_tickets,
        }) => cmd_link(
            cli.json,
            &id,
            jira,
            blocks,
            blocked_by,
            relates_to,
            duplicates,
            child_tickets,
        ),
        None => cmd_version(cli.json),
    }
}

fn cmd_version(json: bool) -> LiraResult<()> {
    output(
        json,
        json!({ "version": env!("CARGO_PKG_VERSION") }),
        env!("CARGO_PKG_VERSION").to_string(),
    )
}

fn cmd_init(json: bool, dry_run: bool) -> LiraResult<()> {
    let report = lira_store::init_workspace(dry_run)?;
    output(json, report, "initialized workspace".to_string())
}

fn cmd_doctor(json: bool) -> LiraResult<()> {
    let report = lira_store::doctor()?;
    output(json, report.clone(), format!("doctor ok: {}", report.ok))
}

fn cmd_project_list(json: bool) -> LiraResult<()> {
    let projects = lira_store::list_projects()?;
    output(
        json,
        json!({ "projects": projects }),
        "listed projects".to_string(),
    )
}

fn cmd_project_create(json: bool, key: &str, name: &str) -> LiraResult<()> {
    let project = lira_store::create_project(key, name)?;
    log_event(
        "project_created",
        None,
        Some(key),
        "ok",
        json!({ "name": name }),
    )?;
    output(json, project, format!("created project {key}"))
}

fn cmd_project_show(json: bool, key: &str) -> LiraResult<()> {
    let project = lira_store::read_project(key)?;
    let workflow = lira_store::read_workflow(key)?;
    output(
        json,
        json!({ "project": project, "workflow": workflow }),
        format!("project {key}"),
    )
}

fn cmd_new(json: bool, args: NewArgs) -> LiraResult<()> {
    validate_project_key(&args.project)?;
    lira_store::read_project(&args.project)?;
    let id = lira_store::allocate_ticket_id(&args.project)?;
    let description = if args.description_stdin {
        read_stdin()?
    } else {
        args.description
    };
    let ticket = Ticket::new(
        id.clone(),
        args.project.clone(),
        args.title,
        description,
        args.ticket_type,
        args.priority,
        args.assignee,
        args.reporter,
        args.parent_jira.map(ParentRef::jira),
        normalize_nonempty(args.acceptance_criteria),
        normalize_nonempty(args.task),
        args.actor,
    );
    lira_store::write_ticket(&ticket)?;
    log_event(
        "ticket_created",
        Some(&id),
        Some(&args.project),
        "ok",
        json!({}),
    )?;
    output(json, ticket, format!("created ticket {id}"))
}

fn cmd_show(json: bool, id: &str) -> LiraResult<()> {
    let ticket = lira_store::read_ticket(id)?;
    output(json, ticket, format!("ticket {id}"))
}

fn cmd_ls(json: bool, project: Option<&str>, status: Option<&str>) -> LiraResult<()> {
    let tickets = lira_store::list_tickets(project, status)?;
    let summaries: Vec<TicketSummary> = tickets.iter().map(TicketSummary::from).collect();
    output(json, json!({ "tickets": summaries }), "tickets".to_string())
}

fn cmd_search(json: bool, query: &str, project: Option<&str>) -> LiraResult<()> {
    let needle = query.to_ascii_lowercase();
    let tickets: Vec<TicketSummary> = lira_store::list_tickets(project, None)?
        .iter()
        .filter(|ticket| ticket_matches_search(ticket, &needle))
        .map(TicketSummary::from)
        .collect();
    output(
        json,
        json!({ "tickets": tickets }),
        format!("search results for {query}"),
    )
}

fn cmd_query(json: bool, args: QueryArgs) -> LiraResult<()> {
    let tickets: Vec<TicketSummary> =
        lira_store::list_tickets(args.project.as_deref(), args.status.as_deref())?
            .iter()
            .filter(|ticket| {
                args.label
                    .as_ref()
                    .is_none_or(|label| ticket.labels.iter().any(|value| value == label))
            })
            .filter(|ticket| {
                args.assignee
                    .as_ref()
                    .is_none_or(|assignee| ticket.assignee.as_ref() == Some(assignee))
            })
            .filter(|ticket| {
                args.task_status
                    .as_ref()
                    .is_none_or(|status| ticket.tasks.iter().any(|task| &task.status == status))
            })
            .filter(|ticket| {
                args.task_tag.as_ref().is_none_or(|tag| {
                    ticket
                        .tasks
                        .iter()
                        .any(|task| task.tags.iter().any(|value| value == tag))
                })
            })
            .filter(|ticket| {
                args.parent_jira.as_ref().is_none_or(|jira| {
                    ticket
                        .parent
                        .as_ref()
                        .is_some_and(|parent| parent.parent_type == "jira" && parent.id == *jira)
                })
            })
            .map(TicketSummary::from)
            .collect();
    output(
        json,
        json!({ "tickets": tickets }),
        "query results".to_string(),
    )
}

fn cmd_count(json: bool, group_by: &str, project: Option<&str>) -> LiraResult<()> {
    let mut counts = std::collections::BTreeMap::new();
    for ticket in lira_store::list_tickets(project, None)? {
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
        *counts.entry(key).or_insert(0usize) += 1;
    }
    output(
        json,
        json!({ "group_by": group_by, "counts": counts }),
        "counts".to_string(),
    )
}

fn cmd_board(json: bool, project: Option<&str>) -> LiraResult<()> {
    let mut board: std::collections::BTreeMap<String, Vec<TicketSummary>> =
        std::collections::BTreeMap::new();
    for ticket in lira_store::list_tickets(project, None)? {
        board
            .entry(ticket.status.clone())
            .or_default()
            .push(TicketSummary::from(&ticket));
    }
    output(json, json!({ "board": board }), "board".to_string())
}

fn cmd_mv(json: bool, id: &str, status: &str, force: bool) -> LiraResult<()> {
    let moved = lira_store::update_ticket(id, |ticket| {
        let workflow = lira_store::read_workflow(&ticket.project)?;
        validate_transition(&workflow, &ticket.status, status)?;
        if status == "done" && !force {
            validate_completion_policy(ticket, &workflow)?;
        }
        let previous = ticket.status.clone();
        ticket.status = status.to_string();
        ticket.touch();
        ticket.add_history(
            "ticket_moved",
            format!("Moved from {previous} to {status}"),
            None,
        );
        Ok(())
    })?;
    log_event(
        "ticket_moved",
        Some(id),
        Some(&moved.project),
        "ok",
        json!({ "status": status }),
    )?;
    output(json, moved, format!("moved {id} to {status}"))
}

fn cmd_task(json: bool, command: TaskCommands) -> LiraResult<()> {
    match command {
        TaskCommands::List { id } => {
            let ticket = lira_store::read_ticket(&id)?;
            output(
                json,
                json!({ "tasks": ticket.tasks }),
                format!("tasks for {id}"),
            )
        }
        TaskCommands::Add { id, title, tag } => {
            let updated = lira_store::update_ticket(&id, |ticket| {
                let task = Task {
                    id: ticket.next_task_id(),
                    title: title.clone(),
                    status: "todo".to_string(),
                    tags: tag.clone(),
                    created_on: now_string(),
                    last_modified: now_string(),
                };
                ticket.tasks.push(task.clone());
                ticket.touch();
                ticket.add_history("task_added", format!("Added task {}", task.id), None);
                Ok(())
            })?;
            log_event(
                "task_added",
                Some(&id),
                Some(&updated.project),
                "ok",
                json!({}),
            )?;
            output(json, updated, format!("added task to {id}"))
        }
        TaskCommands::Status { id, task, status } => update_task_status(json, &id, &task, &status),
        TaskCommands::Done { id, task } => update_task_status(json, &id, &task, "done"),
        TaskCommands::Cancel { id, task } => update_task_status(json, &id, &task, "cancelled"),
    }
}

fn update_task_status(json: bool, id: &str, task_id: &str, status: &str) -> LiraResult<()> {
    let updated = lira_store::update_ticket(id, |ticket| {
        let workflow = lira_store::read_workflow(&ticket.project)?;
        if !workflow.has_task_status(status) {
            return Err(LiraError::new(
                "E_INVALID_TASK_STATUS",
                format!("Invalid task status '{status}'."),
            ));
        }
        let task = ticket
            .tasks
            .iter_mut()
            .find(|task| task.id == task_id)
            .ok_or_else(|| {
                LiraError::new("E_TASK_NOT_FOUND", format!("Task '{task_id}' not found."))
            })?;
        let previous = task.status.clone();
        task.status = status.to_string();
        task.last_modified = now_string();
        ticket.touch();
        ticket.add_history(
            "task_status_changed",
            format!("Moved {task_id} from {previous} to {status}"),
            None,
        );
        Ok(())
    })?;
    log_event(
        "task_status_changed",
        Some(id),
        Some(&updated.project),
        "ok",
        json!({ "task": task_id, "status": status }),
    )?;
    output(json, updated, format!("moved {id}:{task_id} to {status}"))
}

fn cmd_comment(
    json: bool,
    id: &str,
    body: Option<String>,
    stdin: bool,
    author: Option<String>,
) -> LiraResult<()> {
    let body = if stdin {
        read_stdin()?
    } else {
        body.ok_or_else(|| LiraError::new("E_COMMENT_REQUIRED", "Comment body is required."))?
    };
    let updated = lira_store::update_ticket(id, |ticket| {
        let comment = Comment {
            id: ticket.next_comment_id(),
            body: body.clone(),
            author: author.clone(),
            created_on: now_string(),
            sync_github: false,
            github_id: None,
        };
        ticket.comments.push(comment.clone());
        ticket.touch();
        ticket.add_history(
            "comment_added",
            format!("Added comment {}", comment.id),
            author.clone(),
        );
        Ok(())
    })?;
    log_event(
        "comment_added",
        Some(id),
        Some(&updated.project),
        "ok",
        json!({}),
    )?;
    output(json, updated, format!("added comment to {id}"))
}

fn cmd_history(json: bool, command: HistoryCommands) -> LiraResult<()> {
    match command {
        HistoryCommands::List { id } => {
            let ticket = lira_store::read_ticket(&id)?;
            output(
                json,
                json!({ "history": ticket.history }),
                format!("history for {id}"),
            )
        }
        HistoryCommands::Add {
            id,
            action,
            message,
            actor,
        } => {
            let updated = lira_store::update_ticket(&id, |ticket| {
                let seq = ticket.history.len() + 1;
                ticket.history.push(HistoryEvent {
                    id: format!("h{seq}"),
                    action: action.clone(),
                    message: message.clone(),
                    actor: actor.clone(),
                    timestamp: now_string(),
                });
                ticket.touch();
                Ok(())
            })?;
            log_event(
                "history_added",
                Some(&id),
                Some(&updated.project),
                "ok",
                json!({}),
            )?;
            output(json, updated, format!("added history to {id}"))
        }
    }
}

fn cmd_claim(
    json: bool,
    id: &str,
    agent: &str,
    force: bool,
    reason: Option<String>,
) -> LiraResult<()> {
    let updated = lira_store::update_ticket(id, |ticket| {
        if ticket
            .agent
            .claimed_by
            .as_deref()
            .is_some_and(|owner| owner != agent)
            && !force
        {
            return Err(LiraError::new(
                "E_CLAIM_HELD",
                format!(
                    "Ticket is already claimed by {}.",
                    ticket.agent.claimed_by.clone().unwrap()
                ),
            )
            .details(json!({
                "owner": ticket.agent.claimed_by,
                "claimed_at": ticket.agent.claimed_at,
            }))
            .suggestion(
                format!(
                    "lira active --agent {} --json",
                    ticket.agent.claimed_by.clone().unwrap()
                ),
                "inspect current owner",
            ));
        }
        let now = now_string();
        ticket.agent.claimed_by = Some(agent.to_string());
        ticket.agent.claimed_at = Some(now.clone());
        let metadata = ticket.orchestration.get_or_insert_with(Default::default);
        metadata.last_claimed_by = Some(agent.to_string());
        metadata.last_claimed_at = Some(now);
        ticket.touch();
        let reason_suffix = reason
            .as_ref()
            .map(|reason| format!(": {reason}"))
            .unwrap_or_default();
        ticket.add_history(
            "claimed",
            format!("Claimed by {agent}{reason_suffix}"),
            Some(agent.to_string()),
        );
        Ok(())
    })?;
    let issue = lira_store::normalized_issue(id)?;
    log_event(
        "claimed",
        Some(id),
        Some(&updated.project),
        "ok",
        json!({ "agent": agent }),
    )?;
    output(
        json,
        json!({ "claimed": true, "previous_owner": null, "ticket": updated, "issue": issue }),
        format!("claimed {id}"),
    )
}

fn cmd_release(json: bool, id: &str, agent: Option<String>) -> LiraResult<()> {
    let updated = lira_store::update_ticket(id, |ticket| {
        if let Some(expected) = &agent {
            if ticket.agent.claimed_by.as_deref() != Some(expected.as_str()) {
                return Err(LiraError::new(
                    "E_CLAIM_HELD",
                    "Ticket is not claimed by the requested agent.",
                ));
            }
        }
        let previous = ticket.agent.claimed_by.take();
        ticket.agent.claimed_at = None;
        if let Some(metadata) = &mut ticket.orchestration {
            metadata.last_released_at = Some(now_string());
        }
        ticket.touch();
        ticket.add_history(
            "released",
            format!("Released claim from {previous:?}"),
            agent.clone(),
        );
        Ok(())
    })?;
    log_event(
        "released",
        Some(id),
        Some(&updated.project),
        "ok",
        json!({}),
    )?;
    output(json, updated, format!("released {id}"))
}

fn cmd_active(json: bool, agent: &str) -> LiraResult<()> {
    let tickets: Vec<TicketSummary> = lira_store::list_tickets(None, None)?
        .iter()
        .filter(|ticket| ticket.agent.claimed_by.as_deref() == Some(agent))
        .map(TicketSummary::from)
        .collect();
    output(
        json,
        json!({ "tickets": tickets }),
        format!("active tickets for {agent}"),
    )
}

fn cmd_next(json: bool, project: Option<&str>, _agent: Option<&str>) -> LiraResult<()> {
    let ticket = lira_store::candidate_issues(project, None)?
        .into_iter()
        .next();
    output(json, json!({ "ticket": ticket }), "next ticket".to_string())
}

fn cmd_candidates(
    json: bool,
    project: Option<&str>,
    state: Option<&str>,
    limit: Option<usize>,
) -> LiraResult<()> {
    let mut issues = lira_store::candidate_issues(project, state)?;
    if let Some(limit) = limit {
        issues.truncate(limit);
    }
    output(
        json,
        json!({ "issues": issues }),
        "candidate issues".to_string(),
    )
}

fn cmd_issue(json: bool, command: IssueCommands) -> LiraResult<()> {
    match command {
        IssueCommands::Show { id } => {
            let issue = lira_store::normalized_issue(&id)?;
            output(json, json!({ "issue": issue }), format!("issue {id}"))
        }
        IssueCommands::Current { ids } => {
            let issues = lira_store::normalized_issues(&ids)?;
            output(
                json,
                json!({ "issues": issues }),
                "current issues".to_string(),
            )
        }
    }
}

fn cmd_workflow(json: bool, command: WorkflowCommands) -> LiraResult<()> {
    match command {
        WorkflowCommands::Symphony { command } => match command {
            SymphonyWorkflowCommands::Export { project } => {
                lira_store::read_project(&project)?;
                let workflow = lira_store::read_workflow(&project)?;
                let front_matter = json!({
                    "tracker": {
                        "kind": "lira",
                        "project": project,
                        "active_states": workflow.orchestration.active_statuses,
                        "terminal_states": workflow.orchestration.terminal_statuses,
                    }
                });
                output(json, front_matter, "workflow tracker export".to_string())
            }
            SymphonyWorkflowCommands::Validate { path, project } => {
                lira_store::read_project(&project)?;
                let body = std::fs::read_to_string(&path).map_err(|err| {
                    LiraError::new(
                        "E_WORKFLOW_FILE_NOT_FOUND",
                        format!("Could not read {}.", Path::new(&path).display()),
                    )
                    .details(json!({ "error": err.to_string() }))
                })?;
                validate_workflow_front_matter(&body, &project)?;
                output(
                    json,
                    json!({ "valid": true, "project": project, "path": path }),
                    "workflow valid".to_string(),
                )
            }
        },
    }
}

fn validate_workflow_front_matter(body: &str, project: &str) -> LiraResult<()> {
    let Some(rest) = body.strip_prefix("---") else {
        return Err(LiraError::new(
            "E_WORKFLOW_PARSE",
            "WORKFLOW.md must start with YAML front matter.",
        ));
    };
    let Some((front_matter, _markdown)) = rest.split_once("---") else {
        return Err(LiraError::new(
            "E_WORKFLOW_PARSE",
            "WORKFLOW.md front matter is not closed.",
        ));
    };
    let value: serde_yaml::Value = serde_yaml::from_str(front_matter).map_err(|err| {
        LiraError::new(
            "E_WORKFLOW_PARSE",
            "Could not parse WORKFLOW.md front matter.",
        )
        .details(json!({ "error": err.to_string() }))
    })?;
    let tracker = value
        .get("tracker")
        .ok_or_else(|| LiraError::new("E_WORKFLOW_PARSE", "Missing tracker block."))?;
    let kind = tracker.get("kind").and_then(|value| value.as_str());
    if kind != Some("lira") {
        return Err(LiraError::new(
            "E_UNSUPPORTED_TRACKER_KIND",
            "Only tracker.kind: lira is supported.",
        ));
    }
    if let Some(configured_project) = tracker.get("project").and_then(|value| value.as_str()) {
        if configured_project != project {
            return Err(LiraError::new(
                "E_WORKFLOW_PARSE",
                format!("WORKFLOW.md project '{configured_project}' does not match '{project}'."),
            ));
        }
    }
    Ok(())
}

fn cmd_label(json: bool, command: LabelCommands) -> LiraResult<()> {
    match command {
        LabelCommands::Add { id, label } => {
            let updated = lira_store::update_ticket(&id, |ticket| {
                if !ticket.labels.contains(&label) {
                    ticket.labels.push(label.clone());
                    ticket.labels.sort();
                    ticket.touch();
                    ticket.add_history("label_added", format!("Added label {label}"), None);
                }
                Ok(())
            })?;
            log_event(
                "label_added",
                Some(&id),
                Some(&updated.project),
                "ok",
                json!({ "label": label }),
            )?;
            output(json, updated, format!("added label to {id}"))
        }
        LabelCommands::Remove { id, label } => {
            let updated = lira_store::update_ticket(&id, |ticket| {
                ticket.labels.retain(|value| value != &label);
                ticket.touch();
                ticket.add_history("label_removed", format!("Removed label {label}"), None);
                Ok(())
            })?;
            log_event(
                "label_removed",
                Some(&id),
                Some(&updated.project),
                "ok",
                json!({ "label": label }),
            )?;
            output(json, updated, format!("removed label from {id}"))
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn cmd_link(
    json: bool,
    id: &str,
    jira: Option<String>,
    blocks: Vec<String>,
    blocked_by: Vec<String>,
    relates_to: Vec<String>,
    duplicates: Vec<String>,
    child_tickets: Vec<String>,
) -> LiraResult<()> {
    let updated = lira_store::update_ticket(id, |ticket| {
        if let Some(key) = jira.clone() {
            ticket.parent = Some(ParentRef::jira(key));
        }
        extend_unique(&mut ticket.links.blocks, &blocks);
        extend_unique(&mut ticket.links.blocked_by, &blocked_by);
        extend_unique(&mut ticket.links.relates_to, &relates_to);
        extend_unique(&mut ticket.links.duplicates, &duplicates);
        extend_unique(&mut ticket.links.child_tickets, &child_tickets);
        ticket.touch();
        ticket.add_history("links_updated", "Updated ticket links", None);
        Ok(())
    })?;
    log_event(
        "links_updated",
        Some(id),
        Some(&updated.project),
        "ok",
        json!({}),
    )?;
    output(json, updated, format!("updated links for {id}"))
}

fn output<T: Serialize>(json_mode: bool, value: T, human: String) -> LiraResult<()> {
    if json_mode {
        print_json(JsonEnvelope::ok(value))
    } else {
        println!("{human}");
        Ok(())
    }
}

fn print_json<T: Serialize>(value: T) -> LiraResult<()> {
    println!(
        "{}",
        serde_json::to_string_pretty(&value)
            .map_err(|err| LiraError::new("E_JSON_SERIALIZE", err.to_string()))?
    );
    Ok(())
}

fn read_stdin() -> LiraResult<String> {
    let mut body = String::new();
    std::io::stdin()
        .read_to_string(&mut body)
        .map_err(|err| LiraError::new("E_STDIN", err.to_string()))?;
    Ok(body)
}

fn log_event(
    action: &str,
    ticket: Option<&str>,
    project: Option<&str>,
    result: &str,
    details: serde_json::Value,
) -> LiraResult<()> {
    lira_store::append_log(JsonlEvent {
        schema_version: SCHEMA_VERSION,
        timestamp: now_string(),
        action: action.to_string(),
        ticket: ticket.map(ToString::to_string),
        project: project.map(ToString::to_string),
        result: result.to_string(),
        details,
    })
}

fn extend_unique(target: &mut Vec<String>, values: &[String]) {
    for value in values {
        if !target.contains(value) {
            target.push(value.clone());
        }
    }
    target.sort();
}

fn ticket_matches_search(ticket: &Ticket, needle: &str) -> bool {
    let mut haystack = [
        ticket.id.as_str(),
        ticket.title.as_str(),
        ticket.description.as_str(),
        ticket.status.as_str(),
        ticket.priority.as_str(),
    ]
    .join("\n")
    .to_ascii_lowercase();
    for criterion in &ticket.acceptance_criteria {
        haystack.push('\n');
        haystack.push_str(&criterion.to_ascii_lowercase());
    }
    for task in &ticket.tasks {
        haystack.push('\n');
        haystack.push_str(&task.title.to_ascii_lowercase());
        haystack.push('\n');
        haystack.push_str(&task.tags.join(" ").to_ascii_lowercase());
    }
    haystack.contains(needle)
}
