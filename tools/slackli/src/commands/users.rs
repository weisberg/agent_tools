use crate::envelope::ErrorEnvelope;

pub fn run(dry_run: bool) -> ErrorEnvelope {
    // TODO(slackli-post-mvp): implement 'users' command behavior.
    ErrorEnvelope::not_implemented("users", dry_run)
}
