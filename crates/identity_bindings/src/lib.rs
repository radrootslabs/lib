pub use radroots_identity as upstream;

pub const CONSTANTS_TS: &str =
    include_str!("../../../testdata/baseline/current-radroots-generated/identity/constants.ts");

#[cfg(test)]
mod tests {
    use super::CONSTANTS_TS;

    #[test]
    fn preserves_username_constant_exports() {
        assert!(CONSTANTS_TS.contains("RADROOTS_USERNAME_MIN_LEN"));
        assert!(CONSTANTS_TS.contains("RADROOTS_USERNAME_MAX_LEN"));
        assert!(CONSTANTS_TS.contains("RADROOTS_USERNAME_REGEX"));
    }
}
