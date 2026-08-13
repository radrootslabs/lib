//! Native filesystem metadata normalization behind portable generic boundaries.

pub(crate) fn mode<T>(raw: T) -> u32
where
    T: Into<u32>,
{
    raw.into()
}

pub(crate) fn link_count<T>(raw: T) -> u64
where
    T: Into<u64>,
{
    raw.into()
}

pub(crate) fn device<T>(raw: T) -> Result<u64, T::Error>
where
    T: TryInto<u64>,
{
    raw.try_into()
}

pub(crate) fn secure_directory(
    is_directory: bool,
    actual_uid: u32,
    expected_uid: u32,
    raw_mode: u32,
) -> bool {
    crate::all_constraints([
        is_directory,
        actual_uid == expected_uid,
        raw_mode & 0o022 == 0,
    ])
}

pub(crate) fn exact_directory(
    is_directory: bool,
    actual_uid: u32,
    expected_uid: u32,
    raw_mode: u32,
) -> bool {
    crate::all_constraints([
        is_directory,
        actual_uid == expected_uid,
        raw_mode & 0o777 == 0o700,
    ])
}

pub(crate) fn restrictive_directory(
    is_directory: bool,
    actual_uid: u32,
    expected_uid: u32,
    raw_mode: u32,
) -> bool {
    crate::all_constraints([
        is_directory,
        actual_uid == expected_uid,
        matches!(raw_mode & 0o777, 0o500 | 0o700),
    ])
}

pub(crate) fn exact_regular_file(
    is_regular_file: bool,
    link_count: u64,
    actual_uid: u32,
    expected_uid: u32,
    raw_mode: u32,
) -> bool {
    crate::all_constraints([
        is_regular_file,
        link_count == 1,
        actual_uid == expected_uid,
        raw_mode & 0o777 == 0o600,
    ])
}

pub(crate) fn regular_owner_single_link(
    is_regular_file: bool,
    link_count: u64,
    actual_uid: u32,
    expected_uid: u32,
) -> bool {
    crate::all_constraints([is_regular_file, link_count == 1, actual_uid == expected_uid])
}

pub(crate) fn restrictive_regular_file(
    is_regular_file: bool,
    link_count: u64,
    actual_uid: u32,
    expected_uid: u32,
    raw_mode: u32,
) -> bool {
    crate::all_constraints([
        is_regular_file,
        link_count == 1,
        actual_uid == expected_uid,
        matches!(raw_mode & 0o777, 0o400 | 0o600),
    ])
}

pub(crate) fn valid_artifact_length(length: u64, expected: Option<u64>) -> bool {
    let expected_matches = expected.is_none_or(|expected| length == expected);
    crate::all_constraints([length != 0, length <= i64::MAX as u64, expected_matches])
}

pub(crate) fn identity_pair_matches(
    held_device: u64,
    held_inode: u64,
    current_device: u64,
    current_inode: u64,
    expected_device: u64,
    expected_inode: u64,
) -> bool {
    crate::all_constraints([
        (held_device, held_inode) == (expected_device, expected_inode),
        (current_device, current_inode) == (expected_device, expected_inode),
    ])
}

pub(crate) fn sqlite_wal_header(header: &[u8; 20]) -> bool {
    crate::all_constraints([
        &header[..16] == b"SQLite format 3\0",
        header[18] == 2,
        header[19] == 2,
    ])
}

pub(crate) fn sqlite_header(header: &[u8; 20]) -> bool {
    crate::all_constraints([
        &header[..16] == b"SQLite format 3\0",
        matches!(header[18], 1 | 2),
        header[19] == header[18],
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsigned_mode_and_link_widths_normalize_without_truncation() {
        assert_eq!(mode(0o600_u16), 0o600);
        assert_eq!(mode(u32::MAX), u32::MAX);
        assert_eq!(link_count(u16::MAX), u64::from(u16::MAX));
        assert_eq!(link_count(u32::MAX), u64::from(u32::MAX));
        assert_eq!(link_count(u64::MAX), u64::MAX);
    }

    #[test]
    fn signed_and_unsigned_device_widths_remain_checked() {
        assert_eq!(device(7_i32), Ok(7));
        assert_eq!(device(u64::MAX), Ok(u64::MAX));
        assert!(device(-1_i32).is_err());
    }

    #[test]
    fn constraint_inventory_requires_every_entry() {
        assert!(crate::all_constraints([true; 16]));
        for index in 0..16 {
            let mut constraints = [true; 16];
            constraints[index] = false;
            assert!(!crate::all_constraints(constraints));
        }
        assert!(crate::all_constraints([]));
    }

    #[test]
    fn directory_predicates_bind_every_independent_fact() {
        assert!(secure_directory(true, 7, 7, 0o755));
        assert!(!secure_directory(false, 7, 7, 0o755));
        assert!(!secure_directory(true, 8, 7, 0o755));
        assert!(!secure_directory(true, 7, 7, 0o775));

        assert!(exact_directory(true, 7, 7, 0o700));
        assert!(!exact_directory(false, 7, 7, 0o700));
        assert!(!exact_directory(true, 8, 7, 0o700));
        assert!(!exact_directory(true, 7, 7, 0o500));

        assert!(restrictive_directory(true, 7, 7, 0o500));
        assert!(restrictive_directory(true, 7, 7, 0o700));
        assert!(!restrictive_directory(false, 7, 7, 0o700));
        assert!(!restrictive_directory(true, 8, 7, 0o700));
        assert!(!restrictive_directory(true, 7, 7, 0o755));
    }

    #[test]
    fn regular_file_predicates_bind_every_independent_fact() {
        assert!(exact_regular_file(true, 1, 7, 7, 0o600));
        assert!(!exact_regular_file(false, 1, 7, 7, 0o600));
        assert!(!exact_regular_file(true, 2, 7, 7, 0o600));
        assert!(!exact_regular_file(true, 1, 8, 7, 0o600));
        assert!(!exact_regular_file(true, 1, 7, 7, 0o400));

        assert!(regular_owner_single_link(true, 1, 7, 7));
        assert!(!regular_owner_single_link(false, 1, 7, 7));
        assert!(!regular_owner_single_link(true, 2, 7, 7));
        assert!(!regular_owner_single_link(true, 1, 8, 7));

        assert!(restrictive_regular_file(true, 1, 7, 7, 0o400));
        assert!(restrictive_regular_file(true, 1, 7, 7, 0o600));
        assert!(!restrictive_regular_file(false, 1, 7, 7, 0o600));
        assert!(!restrictive_regular_file(true, 2, 7, 7, 0o600));
        assert!(!restrictive_regular_file(true, 1, 8, 7, 0o600));
        assert!(!restrictive_regular_file(true, 1, 7, 7, 0o700));
    }

    #[test]
    fn artifact_length_and_identity_predicates_bind_boundaries() {
        assert!(valid_artifact_length(1, None));
        assert!(valid_artifact_length(
            i64::MAX as u64,
            Some(i64::MAX as u64)
        ));
        assert!(!valid_artifact_length(0, None));
        assert!(!valid_artifact_length(i64::MAX as u64 + 1, None));
        assert!(!valid_artifact_length(1, Some(2)));

        assert!(identity_pair_matches(1, 2, 1, 2, 1, 2));
        for values in [(0, 2, 1, 2), (1, 0, 1, 2), (1, 2, 0, 2), (1, 2, 1, 0)] {
            assert!(!identity_pair_matches(
                values.0, values.1, values.2, values.3, 1, 2
            ));
        }
    }

    #[test]
    fn sqlite_header_requires_exact_wal_versions() {
        let mut header = [0_u8; 20];
        header[..16].copy_from_slice(b"SQLite format 3\0");
        header[18] = 2;
        header[19] = 2;
        assert!(sqlite_wal_header(&header));

        let mut bad_magic = header;
        bad_magic[0] = b'X';
        assert!(!sqlite_wal_header(&bad_magic));
        let mut bad_write = header;
        bad_write[18] = 1;
        assert!(!sqlite_wal_header(&bad_write));
        let mut bad_read = header;
        bad_read[19] = 1;
        assert!(!sqlite_wal_header(&bad_read));

        let mut rollback = header;
        rollback[18] = 1;
        rollback[19] = 1;
        assert!(sqlite_header(&rollback));
        assert!(sqlite_header(&header));
        assert!(!sqlite_header(&bad_magic));
        assert!(!sqlite_header(&bad_write));
        assert!(!sqlite_header(&bad_read));
    }
}
