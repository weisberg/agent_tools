use serde::{Deserialize, Serialize};

/// Conditions for assertions and if-steps.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssertCondition {
    Equals(String),
    NotEquals(String),
    Contains(String),
    NotContains(String),
    Matches(String),
    IsEmpty,
    IsNotEmpty,
    GreaterThan(f64),
    LessThan(f64),
    IsJson,
    InRange(f64, f64),
}

impl AssertCondition {
    /// Evaluate the condition against a string value.
    pub fn evaluate(&self, value: &str) -> bool {
        match self {
            Self::Equals(expected) => value == expected,
            Self::NotEquals(expected) => value != expected,
            Self::Contains(substr) => value.contains(substr.as_str()),
            Self::NotContains(substr) => !value.contains(substr.as_str()),
            Self::Matches(pattern) => {
                regex::Regex::new(pattern)
                    .map(|re| re.is_match(value))
                    .unwrap_or(false)
            }
            Self::IsEmpty => value.is_empty(),
            Self::IsNotEmpty => !value.is_empty(),
            Self::GreaterThan(threshold) => {
                value.trim().parse::<f64>().map(|n| n > *threshold).unwrap_or(false)
            }
            Self::LessThan(threshold) => {
                value.trim().parse::<f64>().map(|n| n < *threshold).unwrap_or(false)
            }
            Self::IsJson => serde_json::from_str::<serde_json::Value>(value).is_ok(),
            Self::InRange(low, high) => {
                value.trim().parse::<f64>().map(|n| n >= *low && n <= *high).unwrap_or(false)
            }
        }
    }
}

/// What to do on assertion failure.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssertFailAction {
    Abort,
    SkipRest,
    Warn,
    Fallback(Box<crate::spec::Step>),
}
