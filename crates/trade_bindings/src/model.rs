use radroots_sdk_binding_model as ts;

pub fn types_module() -> ts::TsModule {
    ts::module(vec![
        ts::type_alias(
            "RadrootsFarmRef",
            ts::object(vec![
                ts::field("pubkey", ts::string()),
                ts::field("d_tag", ts::string()),
            ]),
        ),
        ts::type_alias(
            "RadrootsListing",
            ts::object(vec![
                ts::field("d_tag", ts::string()),
                ts::field("farm", ts::reference("RadrootsFarmRef")),
                ts::field("product", ts::reference("RadrootsListingProduct")),
                ts::field("primary_bin_id", ts::string()),
                ts::field("bins", ts::array(ts::reference("RadrootsListingBin"))),
                ts::optional_field(
                    "resource_area",
                    ts::union(vec![ts::reference("RadrootsResourceAreaRef"), ts::null()]),
                ),
                ts::optional_field(
                    "plot",
                    ts::union(vec![ts::reference("RadrootsPlotRef"), ts::null()]),
                ),
                ts::optional_field(
                    "discounts",
                    ts::union(vec![
                        ts::array(ts::reference("RadrootsCoreDiscount")),
                        ts::null(),
                    ]),
                ),
                ts::optional_field(
                    "inventory_available",
                    ts::union(vec![ts::reference("RadrootsCoreDecimal"), ts::null()]),
                ),
                ts::optional_field(
                    "availability",
                    ts::union(vec![
                        ts::reference("RadrootsListingAvailability"),
                        ts::null(),
                    ]),
                ),
                ts::optional_field(
                    "delivery_method",
                    ts::union(vec![
                        ts::reference("RadrootsListingDeliveryMethod"),
                        ts::null(),
                    ]),
                ),
                ts::optional_field(
                    "location",
                    ts::union(vec![ts::reference("RadrootsListingLocation"), ts::null()]),
                ),
                ts::optional_field(
                    "images",
                    ts::union(vec![
                        ts::array(ts::reference("RadrootsListingImage")),
                        ts::null(),
                    ]),
                ),
            ]),
        ),
        ts::type_alias(
            "RadrootsListingAvailability",
            ts::union(vec![
                ts::object(vec![
                    ts::field("kind", ts::string_literal("window")),
                    ts::field(
                        "amount",
                        ts::object(vec![
                            ts::optional_field("start", ts::union(vec![ts::number(), ts::null()])),
                            ts::optional_field("end", ts::union(vec![ts::number(), ts::null()])),
                        ]),
                    ),
                ]),
                ts::object(vec![
                    ts::field("kind", ts::string_literal("status")),
                    ts::field(
                        "amount",
                        ts::object(vec![ts::field(
                            "status",
                            ts::reference("RadrootsListingStatus"),
                        )]),
                    ),
                ]),
            ]),
        ),
        ts::type_alias(
            "RadrootsListingBin",
            ts::object(vec![
                ts::field("bin_id", ts::string()),
                ts::field("quantity", ts::reference("RadrootsCoreQuantity")),
                ts::field(
                    "price_per_canonical_unit",
                    ts::reference("RadrootsCoreQuantityPrice"),
                ),
                ts::optional_field(
                    "display_amount",
                    ts::union(vec![ts::reference("RadrootsCoreDecimal"), ts::null()]),
                ),
                ts::optional_field(
                    "display_unit",
                    ts::union(vec![ts::reference("RadrootsCoreUnit"), ts::null()]),
                ),
                ts::optional_field("display_label", ts::union(vec![ts::string(), ts::null()])),
                ts::optional_field(
                    "display_price",
                    ts::union(vec![ts::reference("RadrootsCoreMoney"), ts::null()]),
                ),
                ts::optional_field(
                    "display_price_unit",
                    ts::union(vec![ts::reference("RadrootsCoreUnit"), ts::null()]),
                ),
            ]),
        ),
        ts::type_alias(
            "RadrootsListingDeliveryMethod",
            ts::union(vec![
                ts::object(vec![ts::field("kind", ts::string_literal("pickup"))]),
                ts::object(vec![ts::field(
                    "kind",
                    ts::string_literal("local_delivery"),
                )]),
                ts::object(vec![ts::field("kind", ts::string_literal("shipping"))]),
                ts::object(vec![
                    ts::field("kind", ts::string_literal("other")),
                    ts::field(
                        "amount",
                        ts::object(vec![ts::field("method", ts::string())]),
                    ),
                ]),
            ]),
        ),
        ts::type_alias(
            "RadrootsListingLocation",
            ts::object(vec![
                ts::field("primary", ts::string()),
                ts::optional_field("city", ts::union(vec![ts::string(), ts::null()])),
                ts::optional_field("region", ts::union(vec![ts::string(), ts::null()])),
                ts::optional_field("country", ts::union(vec![ts::string(), ts::null()])),
                ts::optional_field("lat", ts::union(vec![ts::number(), ts::null()])),
                ts::optional_field("lng", ts::union(vec![ts::number(), ts::null()])),
                ts::optional_field("geohash", ts::union(vec![ts::string(), ts::null()])),
            ]),
        ),
        ts::type_alias(
            "RadrootsListingProduct",
            ts::object(vec![
                ts::field("key", ts::string()),
                ts::field("title", ts::string()),
                ts::field("category", ts::string()),
                ts::optional_field("summary", ts::union(vec![ts::string(), ts::null()])),
                ts::optional_field("process", ts::union(vec![ts::string(), ts::null()])),
                ts::optional_field("lot", ts::union(vec![ts::string(), ts::null()])),
                ts::optional_field("location", ts::union(vec![ts::string(), ts::null()])),
                ts::optional_field("profile", ts::union(vec![ts::string(), ts::null()])),
                ts::optional_field("year", ts::union(vec![ts::string(), ts::null()])),
            ]),
        ),
        ts::type_alias(
            "RadrootsListingStatus",
            ts::union(vec![
                ts::object(vec![ts::field("kind", ts::string_literal("active"))]),
                ts::object(vec![ts::field("kind", ts::string_literal("sold"))]),
                ts::object(vec![
                    ts::field("kind", ts::string_literal("other")),
                    ts::field("amount", ts::object(vec![ts::field("value", ts::string())])),
                ]),
            ]),
        ),
        ts::type_alias(
            "RadrootsTradeFacetCount",
            ts::object(vec![
                ts::field("key", ts::string()),
                ts::field("count", ts::number()),
            ]),
        ),
        ts::type_alias(
            "RadrootsTradeFulfillmentException",
            ts::object(vec![
                ts::field("code", ts::string()),
                ts::field(
                    "severity",
                    ts::reference("RadrootsTradeFulfillmentExceptionSeverity"),
                ),
                ts::field(
                    "status",
                    ts::reference("RadrootsTradeFulfillmentExceptionStatus"),
                ),
                ts::optional_field("source", ts::union(vec![ts::string(), ts::null()])),
                ts::optional_field("notes", ts::union(vec![ts::string(), ts::null()])),
            ]),
        ),
        ts::type_alias(
            "RadrootsTradeFulfillmentExceptionSeverity",
            ts::union(vec![
                ts::string_literal("notice"),
                ts::string_literal("warning"),
                ts::string_literal("blocking"),
            ]),
        ),
        ts::type_alias(
            "RadrootsTradeFulfillmentExceptionStatus",
            ts::union(vec![
                ts::string_literal("open"),
                ts::string_literal("monitoring"),
                ts::string_literal("resolved"),
            ]),
        ),
        ts::type_alias(
            "RadrootsTradeListing",
            ts::object(vec![
                ts::field("listing_id", ts::string()),
                ts::field("listing_addr", ts::string()),
                ts::field("seller_pubkey", ts::string()),
                ts::field("title", ts::string()),
                ts::field("description", ts::string()),
                ts::field("product_type", ts::string()),
                ts::field("primary_bin_id", ts::string()),
                ts::field("bin_quantity", ts::reference("RadrootsCoreQuantity")),
                ts::field("unit", ts::reference("RadrootsCoreUnit")),
                ts::field("unit_price", ts::reference("RadrootsCoreMoney")),
                ts::field("inventory_available", ts::reference("RadrootsCoreDecimal")),
                ts::field("availability", ts::reference("RadrootsListingAvailability")),
                ts::field("location", ts::reference("RadrootsListingLocation")),
                ts::field(
                    "delivery_method",
                    ts::reference("RadrootsListingDeliveryMethod"),
                ),
                ts::field("listing", ts::reference("RadrootsListing")),
            ]),
        ),
        ts::type_alias(
            "RadrootsTradeListingBackofficeOverlay",
            ts::object(vec![
                ts::field("listing_addr", ts::string()),
                ts::optional_field(
                    "review_queue",
                    ts::union(vec![
                        ts::reference("RadrootsTradeReviewQueueEntry"),
                        ts::null(),
                    ]),
                ),
                ts::field(
                    "moderation_flags",
                    ts::array(ts::reference("RadrootsTradeModerationFlag")),
                ),
            ]),
        ),
        ts::type_alias(
            "RadrootsTradeListingBackofficeQuery",
            ts::object(vec![
                ts::field("listing", ts::reference("RadrootsTradeListingQuery")),
                ts::optional_field(
                    "requires_review",
                    ts::union(vec![ts::boolean(), ts::null()]),
                ),
                ts::optional_field(
                    "has_open_moderation_flags",
                    ts::union(vec![ts::boolean(), ts::null()]),
                ),
            ]),
        ),
        ts::type_alias(
            "RadrootsTradeListingBackofficeView",
            ts::object(vec![
                ts::field("listing", ts::reference("RadrootsTradeListingProjection")),
                ts::optional_field(
                    "marketplace",
                    ts::union(vec![
                        ts::reference("RadrootsTradeMarketplaceListingSummary"),
                        ts::null(),
                    ]),
                ),
                ts::optional_field(
                    "overlay",
                    ts::union(vec![
                        ts::reference("RadrootsTradeListingBackofficeOverlay"),
                        ts::null(),
                    ]),
                ),
                ts::field("requires_review", ts::boolean()),
                ts::field("open_moderation_flag_count", ts::number()),
            ]),
        ),
        ts::type_alias(
            "RadrootsTradeListingBinProjection",
            ts::object(vec![
                ts::field("bin", ts::reference("RadrootsListingBin")),
                ts::field("one_bin_total", ts::reference("RadrootsTradeListingTotal")),
            ]),
        ),
        ts::type_alias(
            "RadrootsTradeListingFacets",
            ts::object(vec![
                ts::field(
                    "seller_pubkeys",
                    ts::array(ts::reference("RadrootsTradeFacetCount")),
                ),
                ts::field(
                    "farm_pubkeys",
                    ts::array(ts::reference("RadrootsTradeFacetCount")),
                ),
                ts::field(
                    "farm_ids",
                    ts::array(ts::reference("RadrootsTradeFacetCount")),
                ),
                ts::field(
                    "product_keys",
                    ts::array(ts::reference("RadrootsTradeFacetCount")),
                ),
                ts::field(
                    "product_categories",
                    ts::array(ts::reference("RadrootsTradeFacetCount")),
                ),
                ts::field(
                    "listing_statuses",
                    ts::array(ts::reference("RadrootsTradeFacetCount")),
                ),
            ]),
        ),
        ts::type_alias(
            "RadrootsTradeListingMarketStatus",
            ts::union(vec![
                ts::string_literal("unknown"),
                ts::string_literal("window"),
                ts::string_literal("active"),
                ts::string_literal("sold"),
                ts::object(vec![ts::field(
                    "other",
                    ts::object(vec![ts::field("value", ts::string())]),
                )]),
            ]),
        ),
        ts::type_alias(
            "RadrootsTradeListingProjection",
            ts::object(vec![
                ts::field("listing_addr", ts::string()),
                ts::field("seller_pubkey", ts::string()),
                ts::field("listing_id", ts::string()),
                ts::field("farm", ts::reference("RadrootsFarmRef")),
                ts::field("product", ts::reference("RadrootsListingProduct")),
                ts::field("primary_bin_id", ts::string()),
                ts::field(
                    "bins",
                    ts::array(ts::reference("RadrootsTradeListingBinProjection")),
                ),
                ts::optional_field(
                    "resource_area",
                    ts::union(vec![ts::reference("RadrootsResourceAreaRef"), ts::null()]),
                ),
                ts::optional_field(
                    "plot",
                    ts::union(vec![ts::reference("RadrootsPlotRef"), ts::null()]),
                ),
                ts::optional_field(
                    "discounts",
                    ts::union(vec![
                        ts::array(ts::reference("RadrootsCoreDiscount")),
                        ts::null(),
                    ]),
                ),
                ts::optional_field(
                    "inventory_available",
                    ts::union(vec![ts::reference("RadrootsCoreDecimal"), ts::null()]),
                ),
                ts::optional_field(
                    "availability",
                    ts::union(vec![
                        ts::reference("RadrootsListingAvailability"),
                        ts::null(),
                    ]),
                ),
                ts::optional_field(
                    "delivery_method",
                    ts::union(vec![
                        ts::reference("RadrootsListingDeliveryMethod"),
                        ts::null(),
                    ]),
                ),
                ts::optional_field(
                    "location",
                    ts::union(vec![ts::reference("RadrootsListingLocation"), ts::null()]),
                ),
                ts::optional_field(
                    "images",
                    ts::union(vec![
                        ts::array(ts::reference("RadrootsListingImage")),
                        ts::null(),
                    ]),
                ),
                ts::field("order_count", ts::number()),
                ts::field("open_order_count", ts::number()),
                ts::field("terminal_order_count", ts::number()),
            ]),
        ),
        ts::type_alias(
            "RadrootsTradeListingQuery",
            ts::object(vec![
                ts::optional_field("seller_pubkey", ts::union(vec![ts::string(), ts::null()])),
                ts::optional_field("farm_pubkey", ts::union(vec![ts::string(), ts::null()])),
                ts::optional_field("farm_id", ts::union(vec![ts::string(), ts::null()])),
                ts::optional_field("product_key", ts::union(vec![ts::string(), ts::null()])),
                ts::optional_field(
                    "product_category",
                    ts::union(vec![ts::string(), ts::null()]),
                ),
                ts::optional_field(
                    "listing_status",
                    ts::union(vec![
                        ts::reference("RadrootsTradeListingMarketStatus"),
                        ts::null(),
                    ]),
                ),
            ]),
        ),
        ts::type_alias(
            "RadrootsTradeListingSort",
            ts::object(vec![
                ts::field("field", ts::reference("RadrootsTradeListingSortField")),
                ts::field("direction", ts::reference("RadrootsTradeSortDirection")),
            ]),
        ),
        ts::type_alias(
            "RadrootsTradeListingSortField",
            ts::union(vec![
                ts::string_literal("listing_addr"),
                ts::string_literal("product_title"),
                ts::string_literal("product_category"),
                ts::string_literal("seller_pubkey"),
                ts::string_literal("inventory_available"),
                ts::string_literal("open_order_count"),
                ts::string_literal("total_order_count"),
            ]),
        ),
        ts::type_alias(
            "RadrootsTradeListingSubtotal",
            ts::object(vec![
                ts::field("price_amount", ts::reference("RadrootsCoreMoney")),
                ts::field("price_currency", ts::reference("RadrootsCoreCurrency")),
                ts::field("quantity_amount", ts::reference("RadrootsCoreDecimal")),
                ts::field("quantity_unit", ts::reference("RadrootsCoreUnit")),
            ]),
        ),
        ts::type_alias(
            "RadrootsTradeListingTotal",
            ts::object(vec![
                ts::field("price_amount", ts::reference("RadrootsCoreMoney")),
                ts::field("price_currency", ts::reference("RadrootsCoreCurrency")),
                ts::field("quantity_amount", ts::reference("RadrootsCoreDecimal")),
                ts::field("quantity_unit", ts::reference("RadrootsCoreUnit")),
            ]),
        ),
        ts::type_alias(
            "RadrootsTradeMarketplaceListingSummary",
            ts::object(vec![
                ts::field("listing_addr", ts::string()),
                ts::field("seller_pubkey", ts::string()),
                ts::field("farm_pubkey", ts::string()),
                ts::field("farm_id", ts::string()),
                ts::field("product_key", ts::string()),
                ts::field("product_title", ts::string()),
                ts::field("product_category", ts::string()),
                ts::optional_field("product_summary", ts::union(vec![ts::string(), ts::null()])),
                ts::field(
                    "listing_status",
                    ts::reference("RadrootsTradeListingMarketStatus"),
                ),
                ts::optional_field(
                    "location_primary",
                    ts::union(vec![ts::string(), ts::null()]),
                ),
                ts::optional_field(
                    "inventory_available",
                    ts::union(vec![ts::reference("RadrootsCoreDecimal"), ts::null()]),
                ),
                ts::field("primary_bin_id", ts::string()),
                ts::optional_field(
                    "primary_bin_label",
                    ts::union(vec![ts::string(), ts::null()]),
                ),
                ts::field(
                    "primary_bin_total",
                    ts::reference("RadrootsTradeListingTotal"),
                ),
                ts::field("order_count", ts::number()),
                ts::field("open_order_count", ts::number()),
                ts::field("terminal_order_count", ts::number()),
            ]),
        ),
        ts::type_alias(
            "RadrootsTradeMarketplaceOrderSummary",
            ts::object(vec![
                ts::field("order_id", ts::string()),
                ts::field("listing_addr", ts::string()),
                ts::field("buyer_pubkey", ts::string()),
                ts::field("seller_pubkey", ts::string()),
                ts::field("status", ts::reference("RadrootsTradeOrderStatus")),
                ts::field(
                    "last_message_type",
                    ts::reference("RadrootsTradeMessageType"),
                ),
                ts::field("item_count", ts::number()),
                ts::field("total_bin_count", ts::number()),
                ts::field("has_requested_discounts", ts::boolean()),
                ts::optional_field("last_reason", ts::union(vec![ts::string(), ts::null()])),
            ]),
        ),
        ts::type_alias(
            "RadrootsTradeMessageType",
            ts::union(vec![
                ts::string_literal("listing_validate_request"),
                ts::string_literal("listing_validate_result"),
                ts::string_literal("order_request"),
                ts::string_literal("order_response"),
                ts::string_literal("order_revision"),
                ts::string_literal("order_revision_accept"),
                ts::string_literal("order_revision_decline"),
                ts::string_literal("question"),
                ts::string_literal("answer"),
                ts::string_literal("discount_request"),
                ts::string_literal("discount_offer"),
                ts::string_literal("discount_accept"),
                ts::string_literal("discount_decline"),
                ts::string_literal("cancel"),
                ts::string_literal("fulfillment_update"),
                ts::string_literal("receipt"),
            ]),
        ),
        ts::type_alias(
            "RadrootsTradeModerationFlag",
            ts::object(vec![
                ts::field("code", ts::string()),
                ts::field("severity", ts::reference("RadrootsTradeModerationSeverity")),
                ts::field("status", ts::reference("RadrootsTradeModerationStatus")),
                ts::optional_field("source", ts::union(vec![ts::string(), ts::null()])),
                ts::optional_field("reason", ts::union(vec![ts::string(), ts::null()])),
            ]),
        ),
        ts::type_alias(
            "RadrootsTradeModerationSeverity",
            ts::union(vec![
                ts::string_literal("notice"),
                ts::string_literal("warning"),
                ts::string_literal("block"),
            ]),
        ),
        ts::type_alias(
            "RadrootsTradeModerationStatus",
            ts::union(vec![
                ts::string_literal("open"),
                ts::string_literal("snoozed"),
                ts::string_literal("resolved"),
            ]),
        ),
        ts::type_alias(
            "RadrootsTradeOrderBackofficeOverlay",
            ts::object(vec![
                ts::field("order_id", ts::string()),
                ts::optional_field(
                    "review_queue",
                    ts::union(vec![
                        ts::reference("RadrootsTradeReviewQueueEntry"),
                        ts::null(),
                    ]),
                ),
                ts::field(
                    "moderation_flags",
                    ts::array(ts::reference("RadrootsTradeModerationFlag")),
                ),
                ts::field(
                    "fulfillment_exceptions",
                    ts::array(ts::reference("RadrootsTradeFulfillmentException")),
                ),
            ]),
        ),
        ts::type_alias(
            "RadrootsTradeOrderBackofficeQuery",
            ts::object(vec![
                ts::field("order", ts::reference("RadrootsTradeOrderQuery")),
                ts::optional_field(
                    "requires_review",
                    ts::union(vec![ts::boolean(), ts::null()]),
                ),
                ts::optional_field(
                    "has_open_moderation_flags",
                    ts::union(vec![ts::boolean(), ts::null()]),
                ),
                ts::optional_field(
                    "has_open_fulfillment_exceptions",
                    ts::union(vec![ts::boolean(), ts::null()]),
                ),
            ]),
        ),
        ts::type_alias(
            "RadrootsTradeOrderBackofficeView",
            ts::object(vec![
                ts::field(
                    "order",
                    ts::reference("RadrootsTradeOrderWorkflowProjection"),
                ),
                ts::field(
                    "marketplace",
                    ts::reference("RadrootsTradeMarketplaceOrderSummary"),
                ),
                ts::optional_field(
                    "overlay",
                    ts::union(vec![
                        ts::reference("RadrootsTradeOrderBackofficeOverlay"),
                        ts::null(),
                    ]),
                ),
                ts::field("requires_review", ts::boolean()),
                ts::field("open_moderation_flag_count", ts::number()),
                ts::field("open_fulfillment_exception_count", ts::number()),
            ]),
        ),
        ts::type_alias(
            "RadrootsTradeOrderFacets",
            ts::object(vec![
                ts::field(
                    "buyer_pubkeys",
                    ts::array(ts::reference("RadrootsTradeFacetCount")),
                ),
                ts::field(
                    "seller_pubkeys",
                    ts::array(ts::reference("RadrootsTradeFacetCount")),
                ),
                ts::field(
                    "listing_addrs",
                    ts::array(ts::reference("RadrootsTradeFacetCount")),
                ),
                ts::field(
                    "statuses",
                    ts::array(ts::reference("RadrootsTradeFacetCount")),
                ),
            ]),
        ),
        ts::type_alias(
            "RadrootsTradeOrderQuery",
            ts::object(vec![
                ts::optional_field("listing_addr", ts::union(vec![ts::string(), ts::null()])),
                ts::optional_field("buyer_pubkey", ts::union(vec![ts::string(), ts::null()])),
                ts::optional_field("seller_pubkey", ts::union(vec![ts::string(), ts::null()])),
                ts::optional_field(
                    "status",
                    ts::union(vec![ts::reference("RadrootsTradeOrderStatus"), ts::null()]),
                ),
            ]),
        ),
        ts::type_alias(
            "RadrootsTradeOrderSort",
            ts::object(vec![
                ts::field("field", ts::reference("RadrootsTradeOrderSortField")),
                ts::field("direction", ts::reference("RadrootsTradeSortDirection")),
            ]),
        ),
        ts::type_alias(
            "RadrootsTradeOrderSortField",
            ts::union(vec![
                ts::string_literal("order_id"),
                ts::string_literal("listing_addr"),
                ts::string_literal("buyer_pubkey"),
                ts::string_literal("seller_pubkey"),
                ts::string_literal("status"),
                ts::string_literal("last_message_type"),
                ts::string_literal("total_bin_count"),
            ]),
        ),
        ts::type_alias(
            "RadrootsTradeOrderStatus",
            ts::union(vec![
                ts::string_literal("draft"),
                ts::string_literal("validated"),
                ts::string_literal("requested"),
                ts::string_literal("questioned"),
                ts::string_literal("revised"),
                ts::string_literal("accepted"),
                ts::string_literal("declined"),
                ts::string_literal("cancelled"),
                ts::string_literal("fulfilled"),
                ts::string_literal("completed"),
            ]),
        ),
        ts::type_alias(
            "RadrootsTradeOrderWorkflowMessage",
            ts::object(vec![
                ts::field("event_id", ts::string()),
                ts::field("actor_pubkey", ts::string()),
                ts::field("counterparty_pubkey", ts::string()),
                ts::field("listing_addr", ts::string()),
                ts::optional_field("order_id", ts::union(vec![ts::string(), ts::null()])),
                ts::optional_field(
                    "listing_event",
                    ts::union(vec![ts::reference("RadrootsNostrEventPtr"), ts::null()]),
                ),
                ts::optional_field("root_event_id", ts::union(vec![ts::string(), ts::null()])),
                ts::optional_field("prev_event_id", ts::union(vec![ts::string(), ts::null()])),
                ts::field("payload", ts::reference("RadrootsTradeMessagePayload")),
            ]),
        ),
        ts::type_alias(
            "RadrootsTradeOrderWorkflowProjection",
            ts::object(vec![
                ts::field("order_id", ts::string()),
                ts::field("listing_addr", ts::string()),
                ts::field("buyer_pubkey", ts::string()),
                ts::field("seller_pubkey", ts::string()),
                ts::field("items", ts::array(ts::reference("RadrootsTradeOrderItem"))),
                ts::optional_field(
                    "requested_discounts",
                    ts::union(vec![
                        ts::array(ts::reference("RadrootsTradeOrderEconomicLine")),
                        ts::null(),
                    ]),
                ),
                ts::field("status", ts::reference("RadrootsTradeOrderStatus")),
                ts::optional_field(
                    "listing_snapshot",
                    ts::union(vec![ts::reference("RadrootsNostrEventPtr"), ts::null()]),
                ),
                ts::field("root_event_id", ts::string()),
                ts::field("last_event_id", ts::string()),
                ts::optional_field(
                    "last_discount_request",
                    ts::union(vec![ts::reference("RadrootsCoreDiscountValue"), ts::null()]),
                ),
                ts::optional_field(
                    "last_discount_offer",
                    ts::union(vec![ts::reference("RadrootsCoreDiscountValue"), ts::null()]),
                ),
                ts::optional_field(
                    "accepted_discount",
                    ts::union(vec![ts::reference("RadrootsCoreDiscountValue"), ts::null()]),
                ),
                ts::optional_field(
                    "last_fulfillment_status",
                    ts::union(vec![
                        ts::reference("RadrootsTradeFulfillmentStatus"),
                        ts::null(),
                    ]),
                ),
                ts::optional_field(
                    "receipt_acknowledged",
                    ts::union(vec![ts::boolean(), ts::null()]),
                ),
                ts::optional_field("receipt_at", ts::union(vec![ts::number(), ts::null()])),
                ts::optional_field("last_reason", ts::union(vec![ts::string(), ts::null()])),
                ts::optional_field(
                    "last_discount_decline_reason",
                    ts::union(vec![ts::string(), ts::null()]),
                ),
                ts::field("question_count", ts::number()),
                ts::field("answer_count", ts::number()),
                ts::field("revision_count", ts::number()),
                ts::field("discount_request_count", ts::number()),
                ts::field("discount_offer_count", ts::number()),
                ts::field("discount_accept_count", ts::number()),
                ts::field("discount_decline_count", ts::number()),
                ts::field("cancellation_count", ts::number()),
                ts::field("fulfillment_update_count", ts::number()),
                ts::field("receipt_count", ts::number()),
                ts::field(
                    "last_message_type",
                    ts::reference("RadrootsTradeMessageType"),
                ),
                ts::field("last_actor_pubkey", ts::string()),
            ]),
        ),
        ts::type_alias(
            "RadrootsTradeReviewPriority",
            ts::union(vec![
                ts::string_literal("low"),
                ts::string_literal("normal"),
                ts::string_literal("high"),
                ts::string_literal("critical"),
            ]),
        ),
        ts::type_alias(
            "RadrootsTradeReviewQueueEntry",
            ts::object(vec![
                ts::field("queue", ts::string()),
                ts::field("priority", ts::reference("RadrootsTradeReviewPriority")),
                ts::field("status", ts::reference("RadrootsTradeReviewStatus")),
                ts::optional_field(
                    "assigned_operator",
                    ts::union(vec![ts::string(), ts::null()]),
                ),
                ts::optional_field("reason", ts::union(vec![ts::string(), ts::null()])),
            ]),
        ),
        ts::type_alias(
            "RadrootsTradeReviewStatus",
            ts::union(vec![
                ts::string_literal("queued"),
                ts::string_literal("in_progress"),
                ts::string_literal("blocked"),
                ts::string_literal("resolved"),
            ]),
        ),
        ts::type_alias(
            "RadrootsTradeSortDirection",
            ts::union(vec![ts::string_literal("asc"), ts::string_literal("desc")]),
        ),
    ])
}
