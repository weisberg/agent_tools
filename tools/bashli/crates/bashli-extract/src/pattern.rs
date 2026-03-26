use regex::Regex;
use serde_json::Value;
use crate::registry::{ExtractorFn, ExtractionError};

pub struct CountMatchingExtractor;
impl ExtractorFn for CountMatchingExtractor {
    fn name(&self) -> &str { "count_matching" }
    fn extract(&self, input: &str, config: &Value) -> Result<Value, ExtractionError> {
        let pattern = config.as_str().ok_or_else(|| {
            ExtractionError::InvalidConfig("count_matching".into(), "requires a pattern string".into())
        })?;
        let re = Regex::new(pattern)?;
        let count = input.lines().filter(|line| re.is_match(line)).count();
        Ok(Value::Number(count.into()))
    }
}

pub struct FirstMatchingExtractor;
impl ExtractorFn for FirstMatchingExtractor {
    fn name(&self) -> &str { "first_matching" }
    fn extract(&self, input: &str, config: &Value) -> Result<Value, ExtractionError> {
        let pattern = config.as_str().ok_or_else(|| {
            ExtractionError::InvalidConfig("first_matching".into(), "requires a pattern string".into())
        })?;
        let re = Regex::new(pattern)?;
        match input.lines().find(|line| re.is_match(line)) {
            Some(line) => Ok(Value::String(line.to_string())),
            None => Ok(Value::Null),
        }
    }
}

pub struct AllMatchingExtractor;
impl ExtractorFn for AllMatchingExtractor {
    fn name(&self) -> &str { "all_matching" }
    fn extract(&self, input: &str, config: &Value) -> Result<Value, ExtractionError> {
        let pattern = config.as_str().ok_or_else(|| {
            ExtractionError::InvalidConfig("all_matching".into(), "requires a pattern string".into())
        })?;
        let re = Regex::new(pattern)?;
        let matches: Vec<Value> = input
            .lines()
            .filter(|line| re.is_match(line))
            .map(|line| Value::String(line.to_string()))
            .collect();
        Ok(Value::Array(matches))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_count_matching() {
        let e = CountMatchingExtractor;
        let result = e.extract("error: a\nwarning: b\nerror: c", &Value::String("^error".into())).unwrap();
        assert_eq!(result, Value::Number(2.into()));
    }

    #[test]
    fn test_first_matching() {
        let e = FirstMatchingExtractor;
        let result = e.extract("warning: a\nerror: b\nerror: c", &Value::String("^error".into())).unwrap();
        assert_eq!(result, Value::String("error: b".into()));
    }

    #[test]
    fn test_all_matching() {
        let e = AllMatchingExtractor;
        let result = e.extract("error: a\nwarning: b\nerror: c", &Value::String("^error".into())).unwrap();
        let arr = result.as_array().unwrap();
        assert_eq!(arr.len(), 2);
    }

    #[test]
    fn test_first_matching_none() {
        let e = FirstMatchingExtractor;
        let result = e.extract("warning: a\ninfo: b", &Value::String("^error".into())).unwrap();
        assert_eq!(result, Value::Null);
    }
}
