use core::fmt;

#[derive(Debug)]
pub enum JobParseError {
    MissingTag(&'static str),
    InvalidTag(&'static str),
    InvalidNumber(&'static str, core::num::ParseIntError),
    NonWholeSats(&'static str),
    AmountOverflow(&'static str),
    MissingChainTag(&'static str),
}

impl fmt::Display for JobParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            JobParseError::MissingTag(t) => write!(f, "missing tag: {}", t),
            JobParseError::InvalidTag(t) => write!(f, "invalid tag structure for '{}'", t),
            JobParseError::InvalidNumber(t, e) => write!(f, "invalid number in '{}': {}", t, e),
            JobParseError::NonWholeSats(t) => {
                write!(
                    f,
                    "amount in msats is not a whole number of sats for '{}'",
                    t
                )
            }
            JobParseError::AmountOverflow(t) => {
                write!(f, "amount overflow in '{}' (does not fit u32 sat)", t)
            }
            JobParseError::MissingChainTag(t) => write!(f, "missing required chain tag: {}", t),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for JobParseError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            JobParseError::InvalidNumber(_, e) => Some(e),
            _ => None,
        }
    }
}
