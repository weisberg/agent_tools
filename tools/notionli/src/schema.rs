use clap::{Arg, Command, CommandFactory};
use serde_json::{json, Map, Value};

use crate::cli::*;

pub(crate) fn command_catalog() -> Value {
    let command = Cli::command();
    Value::Array(flatten_commands(&command, Vec::new()))
}

pub(crate) fn command_tree() -> Value {
    command_to_json(&Cli::command())
}

pub(crate) fn tool_schema(
    command_filter: Option<String>,
    format: &str,
    profile: Option<String>,
) -> Value {
    let profile = profile.unwrap_or_else(|| "admin".into());
    let tools = flatten_tools(&Cli::command(), Vec::new())
        .into_iter()
        .filter(|tool| tool_allowed_for_profile(tool, &profile))
        .filter(|tool| {
            command_filter
                .as_ref()
                .map(|filter| tool.name == *filter || tool.name.starts_with(&format!("{filter}.")))
                .unwrap_or(true)
        })
        .map(|tool| tool_for_format(&tool, format))
        .collect::<Vec<_>>();

    json!({
        "format": format,
        "profile": profile,
        "count": tools.len(),
        "tools": tools,
    })
}

pub(crate) fn error_catalog() -> Value {
    json!([
        {"exit_code": 0, "code": "success"},
        {"exit_code": 1, "code": "usage_error"},
        {"exit_code": 2, "code": "auth_error"},
        {"exit_code": 3, "code": "permission_denied"},
        {"exit_code": 4, "code": "object_not_found"},
        {"exit_code": 5, "code": "ambiguous_object"},
        {"exit_code": 6, "code": "validation_error"},
        {"exit_code": 7, "code": "edit_conflict"},
        {"exit_code": 8, "code": "rate_limited"},
        {"exit_code": 9, "code": "network_or_api_error"},
        {"exit_code": 10, "code": "partial_failure"},
        {"exit_code": 11, "code": "truncated"}
    ])
}

fn command_to_json(command: &Command) -> Value {
    json!({
        "name": command.get_name(),
        "about": command.get_about().map(ToString::to_string),
        "long_about": command.get_long_about().map(ToString::to_string),
        "args": command.get_arguments().map(arg_to_json).collect::<Vec<_>>(),
        "subcommands": command.get_subcommands().map(command_to_json).collect::<Vec<_>>(),
    })
}

fn arg_to_json(arg: &Arg) -> Value {
    let action = format!("{:?}", arg.get_action());
    let value_names = arg
        .get_value_names()
        .map(|names| names.iter().map(ToString::to_string).collect::<Vec<_>>())
        .unwrap_or_default();
    json!({
        "id": arg.get_id().to_string(),
        "long": arg.get_long(),
        "short": arg.get_short().map(|short| short.to_string()),
        "help": arg.get_help().map(ToString::to_string),
        "required": arg.is_required_set(),
        "global": arg.is_global_set(),
        "action": action,
        "value_names": value_names,
        "defaults": arg.get_default_values().iter().map(|value| value.to_string_lossy().to_string()).collect::<Vec<_>>(),
    })
}

fn flatten_commands(command: &Command, path: Vec<String>) -> Vec<Value> {
    let mut items = Vec::new();
    for subcommand in command.get_subcommands() {
        let mut subpath = path.clone();
        subpath.push(subcommand.get_name().to_string());
        let children = flatten_commands(subcommand, subpath.clone());
        items.push(json!({
            "command": subpath.join("."),
            "about": subcommand.get_about().map(ToString::to_string),
            "writes": command_is_write(&subpath),
            "dry_run_default": command_is_write(&subpath),
            "leaf": children.is_empty(),
        }));
        items.extend(children);
    }
    items
}

#[derive(Debug)]
struct ToolShape {
    name: String,
    description: String,
    args: Vec<Value>,
    writes: bool,
}

fn flatten_tools(command: &Command, path: Vec<String>) -> Vec<ToolShape> {
    let subcommands = command.get_subcommands().collect::<Vec<_>>();
    if subcommands.is_empty() && !path.is_empty() {
        return vec![ToolShape {
            name: path.join("."),
            description: command
                .get_about()
                .map(ToString::to_string)
                .unwrap_or_else(|| format!("Run `notionli {}`.", path.join(" "))),
            args: command.get_arguments().map(arg_to_json).collect(),
            writes: command_is_write(&path),
        }];
    }

    let mut tools = Vec::new();
    for subcommand in subcommands {
        let mut subpath = path.clone();
        subpath.push(subcommand.get_name().to_string());
        tools.extend(flatten_tools(subcommand, subpath));
    }
    tools
}

fn tool_for_format(tool: &ToolShape, format: &str) -> Value {
    let input_schema = input_schema_for_tool(tool);
    match format {
        "openai" => json!({
            "type": "function",
            "function": {
                "name": tool.name.replace(['.', '-'], "_"),
                "description": tool.description,
                "parameters": input_schema,
            }
        }),
        "anthropic" => json!({
            "name": tool.name.replace(['.', '-'], "_"),
            "description": tool.description,
            "input_schema": input_schema,
        }),
        "mcp" => json!({
            "name": tool.name,
            "description": tool.description,
            "inputSchema": input_schema,
            "annotations": {
                "readOnlyHint": !tool.writes,
                "destructiveHint": command_is_destructive(&tool.name),
            }
        }),
        _ => json!({
            "name": tool.name,
            "description": tool.description,
            "writes": tool.writes,
            "input_schema": input_schema,
        }),
    }
}

fn input_schema_for_tool(tool: &ToolShape) -> Value {
    let mut properties = Map::new();
    let mut required = Vec::new();

    for arg in &tool.args {
        let Some(id) = arg.get("id").and_then(Value::as_str) else {
            continue;
        };
        let action = arg
            .get("action")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let mut property = json!({
            "type": json_schema_type(action),
            "description": arg.get("help").cloned().unwrap_or(Value::Null),
        });
        if let Some(long) = arg.get("long").and_then(Value::as_str) {
            property["cli_flag"] = json!(format!("--{long}"));
        }
        if let Some(short) = arg.get("short").and_then(Value::as_str) {
            property["cli_short"] = json!(format!("-{short}"));
        }
        if action == "Append" {
            property["items"] = json!({ "type": "string" });
        }
        if arg
            .get("required")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            required.push(Value::String(id.to_string()));
        }
        properties.insert(id.to_string(), property);
    }

    json!({
        "type": "object",
        "additionalProperties": false,
        "properties": properties,
        "required": required,
    })
}

fn json_schema_type(action: &str) -> Value {
    match action {
        "SetTrue" | "SetFalse" => json!("boolean"),
        "Append" => json!("array"),
        _ => json!("string"),
    }
}

fn command_is_write(path: &[String]) -> bool {
    matches!(
        path,
        [noun, verb, ..]
            if matches!(
                (noun.as_str(), verb.as_str()),
                ("alias", "set")
                    | ("alias", "remove")
                    | ("auth", "token")
                    | ("profile", "create")
                    | ("profile", "use")
                    | ("config", "set")
                    | ("config", "use-profile")
                    | ("page", "create")
                    | ("page", "update")
                    | ("page", "append")
                    | ("page", "patch")
                    | ("page", "rename")
                    | ("page", "move")
                    | ("page", "duplicate")
                    | ("page", "trash")
                    | ("page", "restore")
                    | ("page", "worktree")
                    | ("block", "append")
                    | ("block", "insert")
                    | ("block", "replace")
                    | ("block", "update")
                    | ("block", "move")
                    | ("block", "trash")
                    | ("ds", "bulk-update")
                    | ("ds", "bulk-archive")
                    | ("ds", "deduplicate")
                    | ("ds", "import")
                    | ("ds", "move")
                    | ("row", "create")
                    | ("row", "update")
                    | ("row", "upsert")
                    | ("row", "set")
                    | ("row", "relate")
                    | ("row", "trash")
                    | ("row", "restore")
                    | ("comment", "add")
                    | ("comment", "reply")
                    | ("comment", "resolve")
                    | ("file", "upload")
                    | ("file", "attach")
                    | ("webhook", "create")
                    | ("webhook", "delete")
                    | ("batch", "apply")
                    | ("bulk", "rename")
                    | ("template", "register")
                    | ("template", "apply")
                    | ("query", "save")
                    | ("workflow", "run")
                    | ("snapshot", "create")
                    | ("snapshot", "restore-page")
                    | ("snapshot", "restore-row")
                    | ("fixture", "record")
            )
    ) || matches!(path, [single] if matches!(single.as_str(), "select"))
}

fn command_is_destructive(name: &str) -> bool {
    name.ends_with(".trash")
        || name.ends_with(".bulk-archive")
        || name.ends_with(".deduplicate")
        || name.ends_with(".move")
        || name.ends_with(".restore-page")
        || name.ends_with(".restore-row")
}

fn tool_allowed_for_profile(tool: &ToolShape, profile: &str) -> bool {
    match profile {
        "readonly" => !tool.writes,
        "editor" => {
            !tool.writes
                || tool.name.starts_with("page.create")
                || tool.name.starts_with("page.patch")
                || tool.name.starts_with("row.create")
                || tool.name.starts_with("row.update")
                || tool.name.starts_with("comment.add")
        }
        "database-writer" => {
            tool_allowed_for_profile(tool, "editor")
                || tool.name.starts_with("ds.bulk-update")
                || tool.name.starts_with("ds.import")
        }
        _ => true,
    }
}

pub(crate) fn command_name(command: &Commands) -> &'static str {
    match command {
        Commands::Auth(_) => "auth",
        Commands::Profile(_) => "profile",
        Commands::Config(_) => "config",
        Commands::Doctor(_) => "doctor",
        Commands::Resolve(_) => "resolve",
        Commands::Alias(_) => "alias",
        Commands::Select { .. } => "select",
        Commands::Selected => "selected",
        Commands::Search(_) => "search",
        Commands::Ls(_) => "ls",
        Commands::Tree(_) => "tree",
        Commands::Open { .. } => "open",
        Commands::Page(_) => "page",
        Commands::Block(_) => "block",
        Commands::Db(_) => "db",
        Commands::Ds(_) => "ds",
        Commands::Row(_) => "row",
        Commands::Comment(_) => "comment",
        Commands::User(_) => "user",
        Commands::Team(_) => "team",
        Commands::File(_) => "file",
        Commands::Meeting(_) => "meeting",
        Commands::Webhook(_) => "webhook",
        Commands::Watch(_) => "watch",
        Commands::Sync(_) => "sync",
        Commands::Op(_) => "op",
        Commands::Audit(_) => "audit",
        Commands::Policy(_) => "policy",
        Commands::Batch(_) => "batch",
        Commands::Bulk(_) => "bulk",
        Commands::Template(_) => "template",
        Commands::Query(_) => "query",
        Commands::Workflow(_) => "workflow",
        Commands::Snapshot(_) => "snapshot",
        Commands::Mock(_) => "mock",
        Commands::Fixture(_) => "fixture",
        Commands::Tools(_) => "tools",
        Commands::Mcp(_) => "mcp",
        Commands::Schema(_) => "schema",
        Commands::Completion { .. } => "completion",
        Commands::Tui => "tui",
    }
}

pub(crate) fn command_path(command: &Commands) -> String {
    match command {
        Commands::Auth(command) => format!("auth.{}", auth_path(command)),
        Commands::Profile(command) => format!("profile.{}", profile_path(command)),
        Commands::Config(command) => format!("config.{}", config_path(command)),
        Commands::Doctor(command) => format!("doctor.{}", doctor_path(command)),
        Commands::Resolve(_) => "resolve".into(),
        Commands::Alias(command) => format!("alias.{}", alias_path(command)),
        Commands::Select { .. } => "select".into(),
        Commands::Selected => "selected".into(),
        Commands::Search(_) => "search".into(),
        Commands::Ls(_) => "ls".into(),
        Commands::Tree(_) => "tree".into(),
        Commands::Open { .. } => "open".into(),
        Commands::Page(command) => format!("page.{}", page_path(command)),
        Commands::Block(command) => format!("block.{}", block_path(command)),
        Commands::Db(command) => format!("db.{}", db_path(command)),
        Commands::Ds(command) => format!("ds.{}", ds_path(command)),
        Commands::Row(command) => format!("row.{}", row_path(command)),
        Commands::Comment(command) => format!("comment.{}", comment_path(command)),
        Commands::User(command) => format!("user.{}", user_path(command)),
        Commands::Team(command) => format!("team.{}", team_path(command)),
        Commands::File(command) => format!("file.{}", file_path(command)),
        Commands::Meeting(command) => format!("meeting.{}", meeting_path(command)),
        Commands::Webhook(command) => format!("webhook.{}", webhook_path(command)),
        Commands::Watch(_) => "watch".into(),
        Commands::Sync(command) => format!("sync.{}", sync_path(command)),
        Commands::Op(command) => format!("op.{}", op_path(command)),
        Commands::Audit(command) => format!("audit.{}", audit_path(command)),
        Commands::Policy(command) => format!("policy.{}", policy_path(command)),
        Commands::Batch(command) => format!("batch.{}", batch_path(command)),
        Commands::Bulk(command) => format!("bulk.{}", bulk_path(command)),
        Commands::Template(command) => format!("template.{}", template_path(command)),
        Commands::Query(command) => format!("query.{}", query_path(command)),
        Commands::Workflow(command) => format!("workflow.{}", workflow_path(command)),
        Commands::Snapshot(command) => format!("snapshot.{}", snapshot_path(command)),
        Commands::Mock(_) => "mock.serve".into(),
        Commands::Fixture(command) => format!("fixture.{}", fixture_path(command)),
        Commands::Tools(command) => format!("tools.{}", tools_path(command)),
        Commands::Mcp(_) => "mcp.serve".into(),
        Commands::Schema(command) => format!("schema.{}", schema_path(command)),
        Commands::Completion { .. } => "completion".into(),
        Commands::Tui => "tui".into(),
    }
}

fn auth_path(command: &AuthCommand) -> &'static str {
    match command {
        AuthCommand::Login(_) => "login",
        AuthCommand::Token(_) => "token.set",
        AuthCommand::Whoami => "whoami",
        AuthCommand::Doctor => "doctor",
    }
}

fn profile_path(command: &ProfileCommand) -> &'static str {
    match command {
        ProfileCommand::List => "list",
        ProfileCommand::Create { .. } => "create",
        ProfileCommand::Use { .. } => "use",
        ProfileCommand::Show { .. } => "show",
    }
}

fn config_path(command: &ConfigCommand) -> &'static str {
    match command {
        ConfigCommand::Get { .. } => "get",
        ConfigCommand::Set { .. } => "set",
        ConfigCommand::UseProfile { .. } => "use-profile",
    }
}

fn doctor_path(command: &DoctorCommand) -> &'static str {
    match command {
        DoctorCommand::RoundTrip { .. } => "round-trip",
        DoctorCommand::Cache => "cache",
        DoctorCommand::Api => "api",
    }
}

fn alias_path(command: &AliasCommand) -> &'static str {
    match command {
        AliasCommand::Set { .. } => "set",
        AliasCommand::List => "list",
        AliasCommand::Remove { .. } => "remove",
    }
}

fn page_path(command: &PageCommand) -> &'static str {
    match command {
        PageCommand::Get { .. } => "get",
        PageCommand::Fetch(_) => "fetch",
        PageCommand::Section(_) => "section",
        PageCommand::Outline(_) => "outline",
        PageCommand::Create(_) => "create",
        PageCommand::Update(_) => "update",
        PageCommand::Append(_) => "append",
        PageCommand::Patch(_) => "patch",
        PageCommand::Rename(_) => "rename",
        PageCommand::Move(_) => "move",
        PageCommand::Duplicate(_) => "duplicate",
        PageCommand::Trash(_) => "trash",
        PageCommand::Restore { .. } => "restore",
        PageCommand::Edit { .. } => "edit",
        PageCommand::Worktree(command) => page_worktree_path(command),
        PageCommand::Todos { .. } => "todos",
        PageCommand::Headings { .. } => "headings",
        PageCommand::Links { .. } => "links",
        PageCommand::Mentions { .. } => "mentions",
        PageCommand::Files { .. } => "files",
        PageCommand::Comments { .. } => "comments",
        PageCommand::CheckStale { .. } => "check-stale",
    }
}

fn page_worktree_path(command: &PageWorktreeCommand) -> &'static str {
    match command {
        PageWorktreeCommand::Checkout { .. } => "worktree.checkout",
        PageWorktreeCommand::Push { .. } => "worktree.push",
    }
}

fn block_path(command: &BlockCommand) -> &'static str {
    match command {
        BlockCommand::Get { .. } => "get",
        BlockCommand::Children { .. } => "children",
        BlockCommand::Find { .. } => "find",
        BlockCommand::Append(_) => "append",
        BlockCommand::Insert(_) => "insert",
        BlockCommand::Replace(_) => "replace",
        BlockCommand::Update(_) => "update",
        BlockCommand::Move { .. } => "move",
        BlockCommand::Trash { .. } => "trash",
    }
}

fn db_path(command: &DbCommand) -> &'static str {
    match command {
        DbCommand::List => "list",
        DbCommand::Get { .. } => "get",
    }
}

fn ds_path(command: &DsCommand) -> &'static str {
    match command {
        DsCommand::List { .. } => "list",
        DsCommand::Get { .. } => "get",
        DsCommand::Schema(_) => "schema",
        DsCommand::Query(_) => "query",
        DsCommand::BulkUpdate(_) => "bulk-update",
        DsCommand::BulkArchive(_) => "bulk-archive",
        DsCommand::Deduplicate(_) => "deduplicate",
        DsCommand::Import(_) => "import",
        DsCommand::Export(_) => "export",
        DsCommand::Move { .. } => "move",
        DsCommand::Lint { .. } => "lint",
    }
}

fn row_path(command: &RowCommand) -> &'static str {
    match command {
        RowCommand::Get { .. } => "get",
        RowCommand::Create(_) => "create",
        RowCommand::Update(_) => "update",
        RowCommand::Upsert(_) => "upsert",
        RowCommand::Set(_) => "set",
        RowCommand::Relate(_) => "relate",
        RowCommand::Trash { .. } => "trash",
        RowCommand::Restore { .. } => "restore",
    }
}

fn comment_path(command: &CommentCommand) -> &'static str {
    match command {
        CommentCommand::List { .. } => "list",
        CommentCommand::Add(_) => "add",
        CommentCommand::Reply { .. } => "reply",
        CommentCommand::Resolve { .. } => "resolve",
    }
}

fn user_path(command: &UserCommand) -> &'static str {
    match command {
        UserCommand::Me => "me",
        UserCommand::List => "list",
        UserCommand::Find { .. } => "find",
    }
}

fn team_path(command: &TeamCommand) -> &'static str {
    match command {
        TeamCommand::List => "list",
    }
}

fn file_path(command: &FileCommand) -> &'static str {
    match command {
        FileCommand::Upload { .. } => "upload",
        FileCommand::Attach { .. } => "attach",
        FileCommand::List => "list",
        FileCommand::Status { .. } => "status",
    }
}

fn meeting_path(command: &MeetingCommand) -> &'static str {
    match command {
        MeetingCommand::List { .. } => "list",
        MeetingCommand::Get { .. } => "get",
    }
}

fn webhook_path(command: &WebhookCommand) -> &'static str {
    match command {
        WebhookCommand::List => "list",
        WebhookCommand::Create(_) => "create",
        WebhookCommand::Delete { .. } => "delete",
        WebhookCommand::Serve(_) => "serve",
    }
}

fn sync_path(command: &SyncCommand) -> &'static str {
    match command {
        SyncCommand::Run { .. } => "run",
        SyncCommand::Status => "status",
        SyncCommand::Diff => "diff",
        SyncCommand::Pull { .. } => "pull",
    }
}

fn op_path(command: &OpCommand) -> &'static str {
    match command {
        OpCommand::List { .. } => "list",
        OpCommand::Show { .. } => "show",
        OpCommand::Undo { .. } => "undo",
        OpCommand::Status { .. } => "status",
        OpCommand::Resume { .. } => "resume",
        OpCommand::Cancel { .. } => "cancel",
    }
}

fn audit_path(command: &AuditCommand) -> &'static str {
    match command {
        AuditCommand::List => "list",
        AuditCommand::Show { .. } => "show",
    }
}

fn policy_path(command: &PolicyCommand) -> &'static str {
    match command {
        PolicyCommand::Show => "show",
        PolicyCommand::Check { .. } => "check",
    }
}

fn batch_path(command: &BatchCommand) -> &'static str {
    match command {
        BatchCommand::Apply { .. } => "apply",
    }
}

fn bulk_path(command: &BulkCommand) -> &'static str {
    match command {
        BulkCommand::Rename(_) => "rename",
    }
}

fn template_path(command: &TemplateCommand) -> &'static str {
    match command {
        TemplateCommand::List => "list",
        TemplateCommand::Register { .. } => "register",
        TemplateCommand::Apply { .. } => "apply",
    }
}

fn query_path(command: &QueryCommand) -> &'static str {
    match command {
        QueryCommand::Save { .. } => "save",
        QueryCommand::List => "list",
        QueryCommand::Run { .. } => "run",
        QueryCommand::Show { .. } => "show",
    }
}

fn workflow_path(command: &WorkflowCommand) -> &'static str {
    match command {
        WorkflowCommand::List => "list",
        WorkflowCommand::Run { .. } => "run",
        WorkflowCommand::Show { .. } => "show",
    }
}

fn snapshot_path(command: &SnapshotCommand) -> &'static str {
    match command {
        SnapshotCommand::Create { .. } => "create",
        SnapshotCommand::Diff { .. } => "diff",
        SnapshotCommand::RestorePage { .. } => "restore-page",
        SnapshotCommand::RestoreRow { .. } => "restore-row",
    }
}

fn fixture_path(command: &FixtureCommand) -> &'static str {
    match command {
        FixtureCommand::Record { .. } => "record",
        FixtureCommand::Replay { .. } => "replay",
    }
}

fn tools_path(command: &ToolsCommand) -> &'static str {
    match command {
        ToolsCommand::List => "list",
        ToolsCommand::Schema { .. } => "schema",
    }
}

fn schema_path(command: &SchemaCommand) -> &'static str {
    match command {
        SchemaCommand::Commands => "commands",
        SchemaCommand::Errors => "errors",
    }
}
