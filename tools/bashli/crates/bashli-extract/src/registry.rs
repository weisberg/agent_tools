use bashli_core::extraction::Extraction;
use serde_json::Value;
use std::collections::HashMap;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ExtractionError {
    #[error("invalid config for extractor '{0}': {1}")]
    InvalidConfig(String, String),
    #[error("regex error: {0}")]
    Regex(#[from] regex::Error),
    #[error("jq error: {0}")]
    Jq(String),
    #[error("unknown extractor: {0}")]
    UnknownExtractor(String),
}

/// A named extraction function.
pub trait ExtractorFn: Send + Sync {
    fn name(&self) -> &str;
    fn extract(&self, input: &str, config: &Value) -> Result<Value, ExtractionError>;
}

/// Registry mapping Extraction variants to implementations.
pub struct ExtractorRegistry {
    builtins: HashMap<&'static str, Box<dyn ExtractorFn>>,
    extensions: HashMap<String, Box<dyn ExtractorFn>>,
}

impl ExtractorRegistry {
    pub fn new() -> Self {
        Self {
            builtins: HashMap::new(),
            extensions: HashMap::new(),
        }
    }

    pub fn register(&mut self, name: &'static str, extractor: Box<dyn ExtractorFn>) {
        self.builtins.insert(name, extractor);
    }

    pub fn register_extension(&mut self, name: &str, extractor: Box<dyn ExtractorFn>) {
        self.extensions.insert(name.to_string(), extractor);
    }

    pub fn default_registry() -> Self {
        let mut reg = Self::new();
        reg.register("jq", Box::new(crate::jq::JqExtractor));
        reg.register("regex", Box::new(crate::regex_extract::RegexExtractor));
        reg.register("count_matching", Box::new(crate::pattern::CountMatchingExtractor));
        reg.register("first_matching", Box::new(crate::pattern::FirstMatchingExtractor));
        reg.register("all_matching", Box::new(crate::pattern::AllMatchingExtractor));
        reg.register("line", Box::new(crate::lines::LineExtractor));
        reg.register("line_range", Box::new(crate::lines::LineRangeExtractor));
        reg
    }

    /// Apply an Extraction to input text.
    pub fn apply(&self, input: &str, extraction: &Extraction) -> Result<Value, ExtractionError> {
        match extraction {
            Extraction::Jq(expr) => self.dispatch("jq", input, &Value::String(expr.clone())),
            Extraction::Regex(pattern) => self.dispatch("regex", input, &Value::String(pattern.clone())),
            Extraction::CountMatching(pattern) => self.dispatch("count_matching", input, &Value::String(pattern.clone())),
            Extraction::FirstMatching(pattern) => self.dispatch("first_matching", input, &Value::String(pattern.clone())),
            Extraction::AllMatching(pattern) => self.dispatch("all_matching", input, &Value::String(pattern.clone())),
            Extraction::Line(n) => self.dispatch("line", input, &Value::Number((*n as u64).into())),
            Extraction::LineRange(start, end) => {
                let config = serde_json::json!({"start": start, "end": end});
                self.dispatch("line_range", input, &config)
            }
            Extraction::Extension { name, config } => {
                if let Some(e) = self.extensions.get(name.as_str()) {
                    e.extract(input, config)
                } else {
                    Err(ExtractionError::UnknownExtractor(name.clone()))
                }
            }
        }
    }

    fn dispatch(&self, name: &str, input: &str, config: &Value) -> Result<Value, ExtractionError> {
        if let Some(e) = self.builtins.get(name) {
            e.extract(input, config)
        } else if let Some(e) = self.extensions.get(name) {
            e.extract(input, config)
        } else {
            Err(ExtractionError::UnknownExtractor(name.to_string()))
        }
    }
}

impl Default for ExtractorRegistry {
    fn default() -> Self {
        Self::default_registry()
    }
}
