use serde::Serialize;

use crate::envelope::SuccessEnvelope;

#[derive(Debug, Serialize)]
pub struct StatusResult {
    pub message: &'static str,
    pub version: &'static str,
    pub defaults: Defaults,
}

#[derive(Debug, Serialize)]
pub struct Defaults {
    pub output: &'static str,
    pub stream_output: &'static str,
    pub receive_mode: &'static str,
    pub reply_mode: &'static str,
}

pub fn run(dry_run: bool) -> SuccessEnvelope<StatusResult> {
    SuccessEnvelope::new(
        StatusResult {
            message: "slackli foundation ready",
            version: env!("CARGO_PKG_VERSION"),
            defaults: Defaults {
                output: "json",
                stream_output: "ndjson",
                receive_mode: "socket_mode",
                reply_mode: "thread",
            },
        },
        "status",
        dry_run,
    )
}
