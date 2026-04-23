//! Knowledge-base resolution and template rendering.

pub mod resolver;
pub mod rules;
pub mod template;

pub use resolver::{KbResolver, KbValidationIssue};
pub use rules::{load_rules, KbRule, RuleMetadata};
pub use template::render_template;
