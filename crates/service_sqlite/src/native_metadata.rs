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

#[cfg(test)]
mod tests {
    use super::{device, link_count, mode};

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
}
