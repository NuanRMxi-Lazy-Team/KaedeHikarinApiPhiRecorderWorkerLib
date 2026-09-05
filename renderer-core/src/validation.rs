use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationError {
    MissingPath(&'static str),
    NonPositive(&'static str),
    NonNegative(&'static str),
    Positive(&'static str),
    EvenResolution,
    GreaterThan {
        field: &'static str,
        other: &'static str,
    },
    InvalidEnum {
        field: &'static str,
        value: String,
    },
}

impl fmt::Display for ValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingPath(field) => write!(formatter, "missing path: {field}"),
            Self::NonPositive(field) => write!(formatter, "value must be positive: {field}"),
            Self::NonNegative(field) => write!(formatter, "value must be non-negative: {field}"),
            Self::Positive(field) => write!(formatter, "value must be greater than zero: {field}"),
            Self::EvenResolution => write!(formatter, "resolution must have even width and height"),
            Self::GreaterThan { field, other } => {
                write!(formatter, "{field} must be greater than {other}")
            }
            Self::InvalidEnum { field, value } => write!(formatter, "invalid {field}: {value}"),
        }
    }
}

impl std::error::Error for ValidationError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{RenderConfig, RenderRequest};
    use std::path::PathBuf;

    #[test]
    fn default_config_is_valid() {
        assert!(RenderConfig::default().validate().is_ok());
    }

    #[test]
    fn odd_resolution_is_rejected_for_yuv420() {
        let mut config = RenderConfig::default();
        config.resolution.width = 1919;

        assert_eq!(config.validate(), Err(ValidationError::EvenResolution));
    }

    #[test]
    fn request_requires_explicit_input_and_output_paths() {
        let request = RenderRequest {
            chart_path: PathBuf::new(),
            output_path: PathBuf::new(),
            config: RenderConfig::default(),
        };

        assert_eq!(
            request.validate(),
            Err(ValidationError::MissingPath("chart_path"))
        );
    }
}
