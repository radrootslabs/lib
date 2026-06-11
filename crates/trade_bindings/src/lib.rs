pub use radroots_trade as upstream;

pub const TYPES_TS: &str = include_str!("typescript/types.ts");

#[cfg(test)]
mod tests {
    use super::TYPES_TS;

    #[test]
    fn preserves_trade_type_exports() {
        assert!(TYPES_TS.contains("export type RadrootsTradeListingTotal"));
        assert!(TYPES_TS.contains("export type RadrootsTradeOrderWorkflowProjection"));
        assert!(TYPES_TS.contains("export type RadrootsTradeMarketplaceOrderSummary"));
    }
}
