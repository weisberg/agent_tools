use serde_json::Value;
use crate::registry::{ExtractorFn, ExtractionError};

pub struct LineExtractor;
impl ExtractorFn for LineExtractor {
    fn name(&self) -> &str { "line" }
    fn extract(&self, input: &str, config: &Value) -> Result<Value, ExtractionError> {
        let n = config.as_u64().ok_or_else(|| {
            ExtractionError::InvalidConfig("line".into(), "requires a line number".into())
        })? as usize;
        match input.lines().nth(n) {
            Some(line) => Ok(Value::String(line.to_string())),
            None => Ok(Value::Null),
        }
    }
}

pub struct LineRangeExtractor;
impl ExtractorFn for LineRangeExtractor {
    fn name(&self) -> &str { "line_range" }
    fn extract(&self, input: &str, config: &Value) -> Result<Value, ExtractionError> {
        let start = config.get("start")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| ExtractionError::InvalidConfig("line_range".into(), "requires 'start' field".into()))?
            as usize;
        let end = config.get("end")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| ExtractionError::InvalidConfig("line_range".into(), "requires 'end' field".into()))?
            as usize;

        let lines: Vec<Value> = input
            .lines()
            .skip(start)
            .take(end.saturating_sub(start))
            .map(|line| Value::String(line.to_string()))
            .collect();
        Ok(Value::Array(lines))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_line_extractor() {
        let e = LineExtractor;
        let result = e.extract("a\nb\nc", &Value::Number(1.into())).unwrap();
        assert_eq!(result, Value::String("b".into()));
    }

    #[test]
    fn test_line_out_of_bounds() {
        let e = LineExtractor;
        let result = e.extract("a\nb", &Value::Number(5.into())).unwrap();
        assert_eq!(result, Value::Null);
    }

    #[test]
    fn test_line_range() {
        let e = LineRangeExtractor;
        let config = serde_json::json!({"start": 1, "end": 3});
        let result = e.extract("a\nb\nc\nd\ne", &config).unwrap();
        let arr = result.as_array().unwrap();
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0], Value::String("b".into()));
        assert_eq!(arr[1], Value::String("c".into()));
    }
}
