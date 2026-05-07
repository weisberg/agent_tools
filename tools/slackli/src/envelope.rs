use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct Meta {
    pub tool: &'static str,
    pub command: String,
    pub dry_run: bool,
}

#[derive(Debug, Serialize)]
pub struct SuccessEnvelope<T>
where
    T: Serialize,
{
    pub ok: bool,
    pub result: T,
    pub meta: Meta,
}

#[derive(Debug, Serialize)]
pub struct ErrorEnvelope {
    pub ok: bool,
    pub error: ErrorPayload,
    pub meta: Meta,
}

#[derive(Debug, Serialize)]
pub struct ErrorPayload {
    pub code: &'static str,
    pub message: String,
    pub suggestion: &'static str,
}

impl<T> SuccessEnvelope<T>
where
    T: Serialize,
{
    pub fn new(result: T, command: impl Into<String>, dry_run: bool) -> Self {
        Self {
            ok: true,
            result,
            meta: Meta {
                tool: "slackli",
                command: command.into(),
                dry_run,
            },
        }
    }
}

impl ErrorEnvelope {
    pub fn not_implemented(command: impl Into<String>, dry_run: bool) -> Self {
        Self {
            ok: false,
            error: ErrorPayload {
                code: "NOT_IMPLEMENTED",
                message: "Command scaffold exists but behavior is not implemented yet".into(),
                suggestion: "Start with `status`, then implement MVP flow: send/reply/history/thread/listen.",
            },
            meta: Meta {
                tool: "slackli",
                command: command.into(),
                dry_run,
            },
        }
    }
}
