/// Convert human-readable dimension strings to Office.js points.
///
/// Supported formats:
///   1in   → 72.0
///   72pt  → 72.0
///   2.54cm → 72.0
///   100px → 75.0
///   914400emu → 72.0
///   72    → 72.0 (bare number = points)

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Points(pub f64);

impl Points {
    /// Parse a dimension string into points.
    pub fn parse(s: &str) -> Result<Self, UnitError> {
        let s = s.trim();
        if s.is_empty() {
            return Err(UnitError::Empty);
        }

        // Try suffixed formats
        if let Some(val) = s.strip_suffix("emu") {
            let v: f64 = val.parse().map_err(|_| UnitError::Invalid(s.to_string()))?;
            return Ok(Points(v / 12700.0));
        }
        if let Some(val) = s.strip_suffix("in") {
            let v: f64 = val.parse().map_err(|_| UnitError::Invalid(s.to_string()))?;
            return Ok(Points(v * 72.0));
        }
        if let Some(val) = s.strip_suffix("cm") {
            let v: f64 = val.parse().map_err(|_| UnitError::Invalid(s.to_string()))?;
            return Ok(Points(v * 72.0 / 2.54));
        }
        if let Some(val) = s.strip_suffix("px") {
            let v: f64 = val.parse().map_err(|_| UnitError::Invalid(s.to_string()))?;
            return Ok(Points(v * 0.75));
        }
        if let Some(val) = s.strip_suffix("pt") {
            let v: f64 = val.parse().map_err(|_| UnitError::Invalid(s.to_string()))?;
            return Ok(Points(v));
        }

        // Bare number = points
        let v: f64 = s.parse().map_err(|_| UnitError::Invalid(s.to_string()))?;
        Ok(Points(v))
    }

    /// Format as inches string for display.
    pub fn to_inches_str(self) -> String {
        format!("{:.2}in", self.0 / 72.0)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum UnitError {
    #[error("empty dimension string")]
    Empty,
    #[error("invalid dimension: {0} (expected e.g. 1in, 72pt, 2.54cm, 100px, 914400emu, or bare number)")]
    Invalid(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_inches() {
        assert_eq!(Points::parse("1in").unwrap().0, 72.0);
        assert_eq!(Points::parse("0.5in").unwrap().0, 36.0);
    }

    #[test]
    fn parse_points() {
        assert_eq!(Points::parse("72pt").unwrap().0, 72.0);
        assert_eq!(Points::parse("72").unwrap().0, 72.0);
    }

    #[test]
    fn parse_cm() {
        let pts = Points::parse("2.54cm").unwrap().0;
        assert!((pts - 72.0).abs() < 0.01);
    }

    #[test]
    fn parse_px() {
        assert_eq!(Points::parse("100px").unwrap().0, 75.0);
    }

    #[test]
    fn parse_emu() {
        let pts = Points::parse("914400emu").unwrap().0;
        assert!((pts - 72.0).abs() < 0.01);
    }

    #[test]
    fn to_inches() {
        assert_eq!(Points(72.0).to_inches_str(), "1.00in");
    }
}
