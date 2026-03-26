use bashli_core::GlobalSettings;
use bashli_extract::ExtractorRegistry;
use bashli_runner::CommandRunner;
use bashli_steps::StepRegistry;
use bashli_transforms::TransformRegistry;
use std::time::Duration;

use crate::Engine;

/// Builder pattern for configuring the engine.
pub struct EngineBuilder {
    step_registry: StepRegistry,
    transform_registry: TransformRegistry,
    extractor_registry: ExtractorRegistry,
    settings: GlobalSettings,
    shell: Vec<String>,
    default_timeout: Duration,
}

impl EngineBuilder {
    pub fn new() -> Self {
        Self {
            step_registry: StepRegistry::new(),
            transform_registry: TransformRegistry::default_registry(),
            extractor_registry: ExtractorRegistry::default_registry(),
            settings: GlobalSettings::default(),
            shell: vec!["/bin/sh".into(), "-c".into()],
            default_timeout: Duration::from_secs(300),
        }
    }

    pub fn settings(mut self, settings: GlobalSettings) -> Self {
        if let Some(ref shell) = settings.shell {
            self.shell = shell.clone();
        }
        self.default_timeout = Duration::from_millis(settings.timeout_ms);
        self.settings = settings;
        self
    }

    pub fn shell(mut self, shell: Vec<String>) -> Self {
        self.shell = shell;
        self
    }

    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.default_timeout = timeout;
        self
    }

    pub fn read_only(mut self, read_only: bool) -> Self {
        self.settings.read_only = read_only;
        self
    }

    pub fn allowed_paths(mut self, paths: Vec<String>) -> Self {
        self.settings.allowed_paths = Some(paths);
        self
    }

    pub fn build(self) -> Engine {
        let runner = CommandRunner::new(self.shell, self.default_timeout);
        Engine {
            step_registry: self.step_registry,
            transform_registry: self.transform_registry,
            extractor_registry: self.extractor_registry,
            runner,
            settings: self.settings,
        }
    }
}

impl Default for EngineBuilder {
    fn default() -> Self {
        Self::new()
    }
}
