#[cfg(not(feature = "std"))]
use alloc::string::{String, ToString};

use core::fmt;

use radroots_events_codec::error::{EventEncodeError, EventParseError};
use radroots_sql_core::error::SqlError;
use radroots_types::types::IError;

pub enum RadrootsTangleEventsError {
    Sql(IError<SqlError>),
    Encode(EventEncodeError),
    Parse(EventParseError),
    InvalidSelector(String),
    InvalidData(String),
}

impl fmt::Debug for RadrootsTangleEventsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl fmt::Display for RadrootsTangleEventsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sql(err) => write!(f, "tangle_events.sql: {}", err.err.to_string()),
            Self::Encode(err) => write!(f, "tangle_events.encode: {err}"),
            Self::Parse(err) => write!(f, "tangle_events.parse: {err}"),
            Self::InvalidSelector(msg) => write!(f, "tangle_events.selector: {msg}"),
            Self::InvalidData(msg) => write!(f, "tangle_events.data: {msg}"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for RadrootsTangleEventsError {}

impl From<IError<SqlError>> for RadrootsTangleEventsError {
    fn from(err: IError<SqlError>) -> Self {
        Self::Sql(err)
    }
}

impl From<EventEncodeError> for RadrootsTangleEventsError {
    fn from(err: EventEncodeError) -> Self {
        Self::Encode(err)
    }
}

impl From<EventParseError> for RadrootsTangleEventsError {
    fn from(err: EventParseError) -> Self {
        Self::Parse(err)
    }
}
