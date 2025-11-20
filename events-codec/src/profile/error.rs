use core::fmt;

#[derive(Debug)]
pub enum ProfileEncodeError {
    InvalidUrl(&'static str, String),
    Json,
}

impl fmt::Display for ProfileEncodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidUrl(field, val) => write!(f, "invalid URL for {}: {}", field, val),
            Self::Json => write!(f, "failed to serialize metadata JSON"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for ProfileEncodeError {}
