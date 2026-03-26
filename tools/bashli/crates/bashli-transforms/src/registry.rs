use bashli_core::transform::Transform;
use serde_json::Value;
use std::collections::HashMap;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum TransformError {
    #[error("invalid config for transform '{0}': {1}")]
    InvalidConfig(String, String),
    #[error("regex error: {0}")]
    Regex(#[from] regex::Error),
    #[error("jq error: {0}")]
    Jq(String),
    #[error("sed error: {0}")]
    Sed(String),
    #[error("awk error: {0}")]
    Awk(String),
    #[error("json parse error: {0}")]
    JsonParse(String),
    #[error("base64 decode error: {0}")]
    Base64Decode(String),
    #[error("unknown transform: {0}")]
    UnknownTransform(String),
}

/// A named, stateless transform function.
pub trait TransformFn: Send + Sync {
    fn name(&self) -> &str;
    fn apply(&self, input: &str, config: &Value) -> Result<Value, TransformError>;
}

/// Registry mapping Transform variants to implementations.
pub struct TransformRegistry {
    builtins: HashMap<&'static str, Box<dyn TransformFn>>,
    extensions: HashMap<String, Box<dyn TransformFn>>,
}

impl TransformRegistry {
    pub fn new() -> Self {
        Self {
            builtins: HashMap::new(),
            extensions: HashMap::new(),
        }
    }

    pub fn register(&mut self, name: &'static str, transform: Box<dyn TransformFn>) {
        self.builtins.insert(name, transform);
    }

    pub fn register_extension(&mut self, name: &str, transform: Box<dyn TransformFn>) {
        self.extensions.insert(name.to_string(), transform);
    }

    pub fn default_registry() -> Self {
        let mut reg = Self::new();
        reg.register("trim", Box::new(crate::text::TrimTransform));
        reg.register("lines", Box::new(crate::text::LinesTransform));
        reg.register("count_lines", Box::new(crate::text::CountLinesTransform));
        reg.register("count_bytes", Box::new(crate::text::CountBytesTransform));
        reg.register("count_words", Box::new(crate::text::CountWordsTransform));
        reg.register("head", Box::new(crate::slice::HeadTransform));
        reg.register("tail", Box::new(crate::slice::TailTransform));
        reg.register("sort", Box::new(crate::sort::SortTransform));
        reg.register("unique", Box::new(crate::sort::UniqueTransform));
        reg.register("grep", Box::new(crate::grep::GrepTransform));
        reg.register("json", Box::new(crate::json::JsonParseTransform));
        reg.register("split", Box::new(crate::json::SplitTransform));
        reg.register("jq", Box::new(crate::jq::JqTransform));
        reg.register("sed", Box::new(crate::sed::SedTransform));
        reg.register("awk", Box::new(crate::awk::AwkTransform));
        reg.register("base64_encode", Box::new(crate::encode::Base64EncodeTransform));
        reg.register("base64_decode", Box::new(crate::encode::Base64DecodeTransform));
        reg.register("sha256", Box::new(crate::encode::Sha256Transform));
        reg.register("code_block", Box::new(crate::format::CodeBlockTransform));
        reg.register("regex", Box::new(crate::format::RegexTransform));
        reg
    }

    /// Apply a Transform enum to input text.
    pub fn apply(&self, input: &str, transform: &Transform) -> Result<Value, TransformError> {
        match transform {
            Transform::Raw => Ok(Value::String(input.to_string())),
            Transform::Trim => self.dispatch("trim", input, &Value::Null),
            Transform::Lines => self.dispatch("lines", input, &Value::Null),
            Transform::Json => self.dispatch("json", input, &Value::Null),
            Transform::CountLines => self.dispatch("count_lines", input, &Value::Null),
            Transform::CountBytes => self.dispatch("count_bytes", input, &Value::Null),
            Transform::CountWords => self.dispatch("count_words", input, &Value::Null),
            Transform::Head(n) => self.dispatch("head", input, &Value::Number((*n as u64).into())),
            Transform::Tail(n) => self.dispatch("tail", input, &Value::Number((*n as u64).into())),
            Transform::Sort(spec) => {
                let config = serde_json::to_value(spec).unwrap_or(Value::Null);
                self.dispatch("sort", input, &config)
            }
            Transform::Unique => self.dispatch("unique", input, &Value::Null),
            Transform::Jq(expr) => self.dispatch("jq", input, &Value::String(expr.clone())),
            Transform::Sed(spec) => {
                let config = serde_json::to_value(spec).unwrap_or(Value::Null);
                self.dispatch("sed", input, &config)
            }
            Transform::Awk(spec) => {
                let config = serde_json::to_value(spec).unwrap_or(Value::Null);
                self.dispatch("awk", input, &config)
            }
            Transform::Grep(spec) => {
                let config = serde_json::to_value(spec).unwrap_or(Value::Null);
                self.dispatch("grep", input, &config)
            }
            Transform::Split(delim) => self.dispatch("split", input, &Value::String(delim.clone())),
            Transform::Pipe(transforms) => {
                crate::pipe::apply_pipe(self, input, transforms)
            }
            Transform::Base64Encode => self.dispatch("base64_encode", input, &Value::Null),
            Transform::Base64Decode => self.dispatch("base64_decode", input, &Value::Null),
            Transform::CodeBlock(lang) => {
                let config = lang.as_ref()
                    .map(|l| Value::String(l.clone()))
                    .unwrap_or(Value::Null);
                self.dispatch("code_block", input, &config)
            }
            Transform::Regex(pattern) => self.dispatch("regex", input, &Value::String(pattern.clone())),
            Transform::Sha256 => self.dispatch("sha256", input, &Value::Null),
            Transform::Extension { name, config } => {
                if let Some(t) = self.extensions.get(name.as_str()) {
                    t.apply(input, config)
                } else {
                    Err(TransformError::UnknownTransform(name.clone()))
                }
            }
        }
    }

    fn dispatch(&self, name: &str, input: &str, config: &Value) -> Result<Value, TransformError> {
        if let Some(t) = self.builtins.get(name) {
            t.apply(input, config)
        } else if let Some(t) = self.extensions.get(name) {
            t.apply(input, config)
        } else {
            Err(TransformError::UnknownTransform(name.to_string()))
        }
    }
}

impl Default for TransformRegistry {
    fn default() -> Self {
        Self::default_registry()
    }
}
