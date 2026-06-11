pub use radroots_core as upstream;

pub const TYPES_TS: &str =
    include_str!("../../../testdata/baseline/current-radroots-generated/core/types.ts");

#[cfg(test)]
mod tests {
    use super::TYPES_TS;

    #[test]
    fn preserves_core_type_exports() {
        assert!(TYPES_TS.contains("export type RadrootsCoreMoney"));
        assert!(TYPES_TS.contains("export type RadrootsCoreQuantityPrice"));
        assert!(TYPES_TS.contains("\"each\""));
    }
}
