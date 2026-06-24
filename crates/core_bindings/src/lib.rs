pub use radroots_core as upstream;

#[cfg(test)]
mod tests {
    const GENERATED_TYPES_TS: &str =
        include_str!("../../../packages/core-bindings/src/generated/types.ts");

    #[test]
    fn generated_core_types_are_source_rendered() {
        assert!(GENERATED_TYPES_TS.contains("export type RadrootsCoreMoney"));
        assert!(GENERATED_TYPES_TS.contains("export type RadrootsCoreQuantityPrice"));
        assert!(GENERATED_TYPES_TS.contains("export type RadrootsCoreUnitDimension"));
        assert!(GENERATED_TYPES_TS.contains("label?: string | null"));
        assert!(!GENERATED_TYPES_TS.contains("label: string | null"));
    }
}
