pub use radroots_core as upstream;

use radroots_sdk_binding_model as ts;

pub fn types_module() -> ts::TsModule {
    ts::module(vec![
        ts::type_alias("RadrootsCoreCurrency", ts::string()),
        ts::type_alias("RadrootsCoreDecimal", ts::string()),
        ts::type_alias(
            "RadrootsCoreDiscount",
            ts::object(vec![
                ts::field("scope", ts::reference("RadrootsCoreDiscountScope")),
                ts::field("threshold", ts::reference("RadrootsCoreDiscountThreshold")),
                ts::field("value", ts::reference("RadrootsCoreDiscountValue")),
            ]),
        ),
        ts::type_alias(
            "RadrootsCoreDiscountScope",
            ts::union(vec![
                ts::string_literal("bin"),
                ts::string_literal("order_total"),
            ]),
        ),
        ts::type_alias(
            "RadrootsCoreDiscountThreshold",
            ts::union(vec![
                ts::object(vec![
                    ts::field("kind", ts::string_literal("bin_count")),
                    ts::field(
                        "amount",
                        ts::object(vec![
                            ts::field("bin_id", ts::string()),
                            ts::field("min", ts::number()),
                        ]),
                    ),
                ]),
                ts::object(vec![
                    ts::field("kind", ts::string_literal("order_quantity")),
                    ts::field(
                        "amount",
                        ts::object(vec![ts::field(
                            "min",
                            ts::reference("RadrootsCoreQuantity"),
                        )]),
                    ),
                ]),
            ]),
        ),
        ts::type_alias(
            "RadrootsCoreDiscountValue",
            ts::union(vec![
                ts::object(vec![
                    ts::field("kind", ts::string_literal("money_per_bin")),
                    ts::field("amount", ts::reference("RadrootsCoreMoney")),
                ]),
                ts::object(vec![
                    ts::field("kind", ts::string_literal("percent")),
                    ts::field("amount", ts::reference("RadrootsCorePercent")),
                ]),
            ]),
        ),
        ts::type_alias(
            "RadrootsCoreMoney",
            ts::object(vec![
                ts::field("amount", ts::string()),
                ts::field("currency", ts::string()),
            ]),
        ),
        ts::type_alias(
            "RadrootsCorePercent",
            ts::object(vec![ts::field("value", ts::string())]),
        ),
        ts::type_alias(
            "RadrootsCoreQuantity",
            ts::object(vec![
                ts::field("amount", ts::string()),
                ts::field("unit", ts::reference("RadrootsCoreUnit")),
                ts::field("label", ts::nullable(ts::string())),
            ]),
        ),
        ts::type_alias(
            "RadrootsCoreQuantityPrice",
            ts::object(vec![
                ts::field("amount", ts::reference("RadrootsCoreMoney")),
                ts::field("quantity", ts::reference("RadrootsCoreQuantity")),
            ]),
        ),
        ts::type_alias(
            "RadrootsCoreUnit",
            ts::union(vec![
                ts::string_literal("each"),
                ts::string_literal("kg"),
                ts::string_literal("g"),
                ts::string_literal("oz"),
                ts::string_literal("lb"),
                ts::string_literal("l"),
                ts::string_literal("ml"),
            ]),
        ),
        ts::type_alias(
            "RadrootsCoreUnitDimension",
            ts::union(vec![
                ts::string_literal("count"),
                ts::string_literal("mass"),
                ts::string_literal("volume"),
            ]),
        ),
    ])
}

#[cfg(test)]
mod tests {
    use super::types_module;

    #[test]
    fn preserves_core_type_exports() {
        let rendered = types_module().render();
        assert!(rendered.contains("export type RadrootsCoreMoney"));
        assert!(rendered.contains("export type RadrootsCoreQuantityPrice"));
        assert!(rendered.contains("\"each\""));
    }
}
