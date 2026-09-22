//! Redacted SQL failure classification shared by the existing storage adapters.

use radroots_storage::Error;

pub(crate) fn map_backend(source: sqlx::Error) -> Error {
    let full = match &source {
        sqlx::Error::Database(error) => error
            .code()
            .and_then(|code| code.parse::<u32>().ok())
            .is_some_and(|code| code & 0xff == 13),
        sqlx::Error::Io(error) => matches!(
            error.kind(),
            std::io::ErrorKind::StorageFull | std::io::ErrorKind::QuotaExceeded
        ),
        _ => false,
    };
    if full {
        Error::SpaceInsufficient
    } else {
        Error::BackendUnavailable
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::error::{DatabaseError, ErrorKind};
    use std::{borrow::Cow, error::Error as StdError, fmt};

    #[derive(Debug)]
    struct Coded(Option<&'static str>);
    impl fmt::Display for Coded {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("private path and raw database detail")
        }
    }
    impl StdError for Coded {}
    impl DatabaseError for Coded {
        fn message(&self) -> &str {
            "private path and raw database detail"
        }
        fn code(&self) -> Option<Cow<'_, str>> {
            self.0.map(Cow::Borrowed)
        }
        fn as_error(&self) -> &(dyn StdError + Send + Sync + 'static) {
            self
        }
        fn as_error_mut(&mut self) -> &mut (dyn StdError + Send + Sync + 'static) {
            self
        }
        fn into_error(self: Box<Self>) -> Box<dyn StdError + Send + Sync + 'static> {
            self
        }
        fn kind(&self) -> ErrorKind {
            ErrorKind::Other
        }
    }

    #[test]
    fn numeric_capacity_codes_are_redacted_and_other_database_errors_stay_generic() {
        for code in [Some("13"), Some("269"), Some("525")] {
            let mapped = map_backend(sqlx::Error::Database(Box::new(Coded(code))));
            assert_eq!(mapped, Error::SpaceInsufficient);
            assert_eq!(mapped.to_string(), "storage space is insufficient");
            assert_eq!(format!("{mapped:?}"), "SpaceInsufficient");
        }
        for code in [
            None,
            Some(""),
            Some("full"),
            Some("-13"),
            Some("4294967296"),
            Some("11"),
            Some("5"),
            Some("10"),
        ] {
            assert_eq!(
                map_backend(sqlx::Error::Database(Box::new(Coded(code)))),
                Error::BackendUnavailable
            );
        }
    }

    #[test]
    fn typed_io_capacity_is_distinct_without_inventing_a_no_effect_receipt() {
        use std::io::ErrorKind as Io;
        for kind in [Io::StorageFull, Io::QuotaExceeded] {
            let source = std::io::Error::new(kind, "private path and raw file detail");
            assert_eq!(
                map_backend(sqlx::Error::Io(source)),
                Error::SpaceInsufficient
            );
        }
        for kind in [Io::PermissionDenied, Io::Other] {
            assert_eq!(
                map_backend(sqlx::Error::Io(std::io::Error::from(kind))),
                Error::BackendUnavailable
            );
        }
        assert_eq!(
            map_backend(sqlx::Error::PoolClosed),
            Error::BackendUnavailable
        );
    }
}
