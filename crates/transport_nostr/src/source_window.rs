use super::{Candidate, FetchCursor, RelayCursor, candidate_is_after_cursor, parse_cursor};

const UNTIL_PREFIX: &str = "nostr-until-v1";

/// Concrete-owner positions remain opaque to transport-neutral callers.
pub(super) enum Position {
    After(RelayCursor),
    Through(u64),
}

impl Position {
    pub(super) fn parse(
        cursor: &FetchCursor,
        scope: &str,
    ) -> Result<Self, radroots_transport::Error> {
        if !cursor.as_str().starts_with(UNTIL_PREFIX) {
            return parse_cursor(cursor, scope).map(Self::After);
        }
        let mut parts = cursor.as_str().split(':');
        let prefix = parts.next();
        let time = parts.next();
        let actual_scope = parts.next();
        let until = time.and_then(|value| value.parse::<u64>().ok());
        match (prefix, time, actual_scope, until, parts.next()) {
            (Some(UNTIL_PREFIX), Some(time), Some(actual), Some(until), None)
                if actual == scope && until.to_string() == time =>
            {
                Ok(Self::Through(until))
            }
            _ => Err(radroots_transport::Error::InvalidFetchCursor),
        }
    }

    pub(super) fn until(&self) -> u64 {
        match self {
            Self::After(cursor) => cursor.created_at_unix_s(),
            Self::Through(until) => *until,
        }
    }

    pub(super) fn includes(&self, candidate: &Candidate) -> bool {
        match self {
            Self::After(cursor) => candidate_is_after_cursor(candidate, cursor),
            Self::Through(until) => candidate.created_at <= *until,
        }
    }
}

pub(super) fn effective_until(selector: Option<u64>, position: Option<&Position>) -> Option<u64> {
    match (selector, position) {
        (Some(until), Some(position)) => Some(until.min(position.until())),
        (Some(until), None) => Some(until),
        (None, Some(position)) => Some(position.until()),
        (None, None) => None,
    }
}

pub(super) fn before_boundary(boundary: u64, scope: &str) -> Option<FetchCursor> {
    let until = boundary.checked_sub(1)?;
    FetchCursor::parse(format!("{UNTIL_PREFIX}:{until}:{scope}")).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn older_positions_are_canonical_bounded_and_never_underflow() {
        let scope = "a".repeat(64);
        assert!(before_boundary(0, &scope).is_none());
        for boundary in [1, 100, u64::MAX] {
            let cursor = before_boundary(boundary, &scope).unwrap();
            let position = Position::parse(&cursor, &scope).unwrap();
            assert_eq!(position.until(), boundary - 1);
            assert_eq!(effective_until(None, Some(&position)), Some(boundary - 1));
            assert_eq!(effective_until(Some(0), Some(&position)), Some(0));
        }
        assert_eq!(effective_until(Some(10), None), Some(10));
        assert_eq!(effective_until(None, None), None);
        for value in ["", "-1", "+1", "01", "18446744073709551616"] {
            let cursor = FetchCursor::parse(format!("{UNTIL_PREFIX}:{value}:{scope}")).unwrap();
            assert!(Position::parse(&cursor, &scope).is_err());
        }
        for value in [
            format!("{UNTIL_PREFIX}:1:{}", "b".repeat(64)),
            format!("{UNTIL_PREFIX}:1:{scope}:extra"),
            format!("{UNTIL_PREFIX}:1"),
            format!("{UNTIL_PREFIX}x:1:{scope}"),
        ] {
            assert!(Position::parse(&FetchCursor::parse(value).unwrap(), &scope).is_err());
        }
    }
}
