use bashli_budget::BudgetTracker;
use bashli_extract::ExtractorRegistry;
use bashli_runner::CommandRunner;
use bashli_transforms::TransformRegistry;
use bashli_vars::VarStore;
use bashli_core::GlobalSettings;

/// Shared resources available to every step executor.
pub struct StepContext<'a> {
    pub vars: &'a mut VarStore,
    pub runner: &'a CommandRunner,
    pub budget: &'a mut BudgetTracker,
    pub transforms: &'a TransformRegistry,
    pub extractors: &'a ExtractorRegistry,
    pub settings: &'a GlobalSettings,
}
