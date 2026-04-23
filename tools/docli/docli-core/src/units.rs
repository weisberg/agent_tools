use crate::error::DocliError;

const DXA_PER_INCH: f64 = 1440.0;
const EMU_PER_INCH: f64 = 914_400.0;
const PX_PER_INCH: f64 = 96.0;

pub fn parse_dxa(value: &str) -> Result<i64, DocliError> {
    parse_length(value, DXA_PER_INCH)
}

pub fn parse_emu(value: &str) -> Result<i64, DocliError> {
    parse_length(value, EMU_PER_INCH)
}

pub fn dxa_to_inches(value: i64) -> f64 {
    value as f64 / DXA_PER_INCH
}

fn parse_length(value: &str, scale_per_inch: f64) -> Result<i64, DocliError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(DocliError::InvalidSpec {
            message: "unit value cannot be empty".to_string(),
        });
    }

    let split_at = trimmed
        .find(|ch: char| !(ch.is_ascii_digit() || ch == '.' || ch == '-'))
        .unwrap_or(trimmed.len());
    let (number, unit) = trimmed.split_at(split_at);
    let magnitude: f64 = number.parse().map_err(|_| DocliError::InvalidSpec {
        message: format!("invalid numeric value: {trimmed}"),
    })?;

    let scaled = match unit {
        "" => magnitude,
        "in" => magnitude * scale_per_inch,
        "cm" => magnitude * scale_per_inch / 2.54,
        "mm" => magnitude * scale_per_inch / 25.4,
        "pt" => magnitude * scale_per_inch / 72.0,
        "px" => magnitude * scale_per_inch / PX_PER_INCH,
        _ => {
            return Err(DocliError::InvalidSpec {
                message: format!("unsupported unit: {trimmed}"),
            });
        }
    };

    Ok(scaled.round() as i64)
}

#[cfg(test)]
mod tests {
    use super::{dxa_to_inches, parse_dxa, parse_emu};

    #[test]
    fn parses_inches_to_dxa() {
        assert_eq!(parse_dxa("1in").unwrap(), 1440);
    }

    #[test]
    fn parses_centimeters_to_dxa() {
        assert_eq!(parse_dxa("2.54cm").unwrap(), 1440);
    }

    #[test]
    fn parses_millimeters_to_dxa() {
        assert_eq!(parse_dxa("25.4mm").unwrap(), 1440);
    }

    #[test]
    fn parses_points_to_dxa() {
        assert_eq!(parse_dxa("12pt").unwrap(), 240);
    }

    #[test]
    fn parses_pixels_to_dxa() {
        assert_eq!(parse_dxa("96px").unwrap(), 1440);
    }

    #[test]
    fn parses_inches_to_emu() {
        assert_eq!(parse_emu("1in").unwrap(), 914_400);
    }

    #[test]
    fn converts_dxa_to_inches() {
        assert_eq!(dxa_to_inches(2160), 1.5);
    }
}
