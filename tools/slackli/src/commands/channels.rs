use crate::envelope::ErrorEnvelope;

pub fn run(dry_run: bool) -> ErrorEnvelope {
    // TODO(slackli-mvp): implement 'channels' command behavior per PLAN.md MVP sequencing.
    // - wire command args + request model
    // - enforce policy gate before writes (if mutating)
    // - call Slack transport adapter(s)
    // - persist structured audit event
    ErrorEnvelope::not_implemented("channels", dry_run)
}
