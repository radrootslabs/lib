mod dto;

pub use dto::dto_roots;
pub use radroots_core as upstream;

#[cfg(test)]
mod tests {
    const GENERATED_TYPES_TS: &str =
        include_str!("../../../packages/core-bindings/src/generated/types.ts");

    #[test]
    fn generated_core_types_are_source_rendered() {
        assert!(GENERATED_TYPES_TS.contains("export type Money"));
        assert!(GENERATED_TYPES_TS.contains("export type QuantityPrice"));
        assert!(GENERATED_TYPES_TS.contains("export type UnitDimension"));
        assert!(GENERATED_TYPES_TS.contains("label?: string | null"));
        assert!(!GENERATED_TYPES_TS.contains("label: string | null"));
        assert!(!GENERATED_TYPES_TS.contains("RadrootsCore"));
    }
}
