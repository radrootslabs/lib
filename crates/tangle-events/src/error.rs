#[cfg(not(feature = "std"))]
use alloc::string::{String, ToString};

use core::fmt;

use radroots_events_codec::error::{EventEncodeError, EventParseError};
use radroots_sql_core::error::SqlError;
use radroots_types::types::IError;

#[derive(Debug)]
pub enum RadrootsTangleEventsError {
    Sql(IError<SqlError>),
    Encode(EventEncodeError),
    Parse(EventParseError),
    InvalidSelector(String),
    InvalidData(String),
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

#[cfg(test)]
mod tests {
    use super::RadrootsTangleEventsError;
    use radroots_events_codec::error::{EventEncodeError, EventParseError};
    use radroots_sql_core::error::SqlError;
    use radroots_types::types::IError;

    #[test]
    fn display_formats_all_error_variants() {
        let sql_err = RadrootsTangleEventsError::Sql(IError::from(SqlError::Internal));
        assert!(sql_err.to_string().contains("tangle_events.sql"));

        let encode_err = RadrootsTangleEventsError::Encode(EventEncodeError::InvalidField("name"));
        assert!(encode_err.to_string().contains("tangle_events.encode"));

        let parse_err = RadrootsTangleEventsError::Parse(EventParseError::InvalidTag("d"));
        assert!(parse_err.to_string().contains("tangle_events.parse"));

        let selector_err =
            RadrootsTangleEventsError::InvalidSelector("selector missing".to_string());
        assert!(selector_err.to_string().contains("tangle_events.selector"));

        let data_err = RadrootsTangleEventsError::InvalidData("bad data".to_string());
        assert!(data_err.to_string().contains("tangle_events.data"));
    }

    #[test]
    fn from_impls_map_into_expected_variants() {
        let sql_from: RadrootsTangleEventsError = IError::from(SqlError::Internal).into();
        assert!(matches!(sql_from, RadrootsTangleEventsError::Sql(_)));

        let encode_from: RadrootsTangleEventsError = EventEncodeError::Json.into();
        assert!(matches!(encode_from, RadrootsTangleEventsError::Encode(_)));

        let parse_number_err = "invalid".parse::<u32>().expect_err("parse int should fail");
        let parse_from: RadrootsTangleEventsError =
            EventParseError::InvalidNumber("k", parse_number_err).into();
        assert!(matches!(parse_from, RadrootsTangleEventsError::Parse(_)));
    }
}
