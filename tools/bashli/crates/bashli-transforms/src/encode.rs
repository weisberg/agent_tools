use base64::{Engine as _, engine::general_purpose::STANDARD};
use sha2::{Sha256, Digest};
use serde_json::Value;
use crate::registry::{TransformFn, TransformError};

pub struct Base64EncodeTransform;
impl TransformFn for Base64EncodeTransform {
    fn name(&self) -> &str { "base64_encode" }
    fn apply(&self, input: &str, _config: &Value) -> Result<Value, TransformError> {
        Ok(Value::String(STANDARD.encode(input.as_bytes())))
    }
}

pub struct Base64DecodeTransform;
impl TransformFn for Base64DecodeTransform {
    fn name(&self) -> &str { "base64_decode" }
    fn apply(&self, input: &str, _config: &Value) -> Result<Value, TransformError> {
        let bytes = STANDARD.decode(input.trim())
            .map_err(|e| TransformError::Base64Decode(e.to_string()))?;
        let s = String::from_utf8(bytes)
            .map_err(|e| TransformError::Base64Decode(e.to_string()))?;
        Ok(Value::String(s))
    }
}

pub struct Sha256Transform;
impl TransformFn for Sha256Transform {
    fn name(&self) -> &str { "sha256" }
    fn apply(&self, input: &str, _config: &Value) -> Result<Value, TransformError> {
        let mut hasher = Sha256::new();
        hasher.update(input.as_bytes());
        let result = hasher.finalize();
        let hex = result.iter().map(|b| format!("{b:02x}")).collect::<String>();
        Ok(Value::String(hex))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base64_roundtrip() {
        let enc = Base64EncodeTransform;
        let dec = Base64DecodeTransform;
        let encoded = enc.apply("hello world", &Value::Null).unwrap();
        let decoded = dec.apply(encoded.as_str().unwrap(), &Value::Null).unwrap();
        assert_eq!(decoded, Value::String("hello world".into()));
    }

    #[test]
    fn test_sha256() {
        let t = Sha256Transform;
        let result = t.apply("hello", &Value::Null).unwrap();
        let hash = result.as_str().unwrap();
        assert_eq!(hash, "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824");
    }
}
