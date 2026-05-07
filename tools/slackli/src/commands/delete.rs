use crate::envelope::ErrorEnvelope;

pub fn run(dry_run: bool) -> ErrorEnvelope {
    // TODO(slackli-post-mvp): implement 'delete' command behavior.
    ErrorEnvelope::not_implemented("delete", dry_run)
}
