pub use radroots_trade as upstream;

mod model;

pub use model::types_module;

#[cfg(test)]
mod tests {
    use super::types_module;

    #[test]
    fn preserves_trade_type_exports() {
        let rendered = types_module().render();
        assert!(rendered.contains("export type RadrootsTradeListingTotal"));
        assert!(rendered.contains("export type RadrootsTradeOrderWorkflowProjection"));
        assert!(rendered.contains("export type RadrootsTradeMarketplaceOrderSummary"));
    }
}
