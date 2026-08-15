//! Bounded projections for untrusted SQLite TEXT and BLOB values.

use sqlx::{Row, sqlite::SqliteRow};

pub(crate) const MAX_IDENTIFIER_UTF8_BYTES: usize = 128;
pub(crate) const MAX_INTEGRITY_RESULT_BYTES: usize = 64;
pub(crate) const INTEGRITY_CHECK_SQL: &str = "PRAGMA integrity_check(1)";

pub(crate) fn bounded_bytes<'row>(
    row: &'row SqliteRow,
    type_ok_column: &str,
    length_column: &str,
    prefix_column: &str,
    minimum: usize,
    maximum: usize,
) -> Option<&'row [u8]> {
    let type_ok = row.try_get::<i64, _>(type_ok_column).ok()? == 1;
    let length = row.try_get::<Option<i64>, _>(length_column).ok()??;
    let length = usize::try_from(length).ok()?;
    let prefix = row.try_get::<Option<&'row [u8]>, _>(prefix_column).ok()??;
    bounded_projection(type_ok, length, prefix, minimum, maximum)
}

pub(crate) fn bounded_utf8<'row>(
    row: &'row SqliteRow,
    type_ok_column: &str,
    length_column: &str,
    prefix_column: &str,
    minimum: usize,
    maximum: usize,
) -> Option<&'row str> {
    core::str::from_utf8(bounded_bytes(
        row,
        type_ok_column,
        length_column,
        prefix_column,
        minimum,
        maximum,
    )?)
    .ok()
}

pub(crate) fn bounded_integrity_bytes(row: &SqliteRow) -> Option<&[u8]> {
    let value = row.try_get::<&[u8], _>(0).ok()?;
    crate::all_constraints([!value.is_empty(), value.len() <= MAX_INTEGRITY_RESULT_BYTES])
        .then_some(value)
}

pub(crate) fn integrity_result_failed(row: &SqliteRow) -> Option<bool> {
    let value = row.try_get::<&[u8], _>(0).ok()?;
    (!value.is_empty()).then_some(value != b"ok")
}

fn bounded_projection(
    type_ok: bool,
    length: usize,
    prefix: &[u8],
    minimum: usize,
    maximum: usize,
) -> Option<&[u8]> {
    crate::all_constraints([
        type_ok,
        minimum <= maximum,
        length >= minimum,
        length <= maximum,
        prefix.len() == length,
    ])
    .then_some(prefix)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounded_projection_rejects_every_independent_drift() {
        assert_eq!(
            bounded_projection(true, 2, b"ok", 1, 64),
            Some(b"ok".as_slice())
        );
        assert!(bounded_projection(false, 2, b"ok", 1, 64).is_none());
        assert!(bounded_projection(true, 2, b"ok", 65, 64).is_none());
        assert!(bounded_projection(true, 0, b"", 1, 64).is_none());
        assert!(bounded_projection(true, 65, &[b'x'; 65], 1, 64).is_none());
        assert!(bounded_projection(true, 2, b"x", 1, 64).is_none());
    }

    #[test]
    fn integrity_query_and_borrowed_byte_cap_are_exact() {
        assert_eq!(INTEGRITY_CHECK_SQL, "PRAGMA integrity_check(1)");
        assert_eq!(MAX_INTEGRITY_RESULT_BYTES, 64);
    }
}
