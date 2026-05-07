use crate::envelope::ErrorEnvelope;

pub fn run(dry_run: bool) -> ErrorEnvelope {
    // TODO(slackli-post-mvp): implement 'upload' command behavior.
    ErrorEnvelope::not_implemented("upload", dry_run)
}
