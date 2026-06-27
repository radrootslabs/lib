use super::*;

const SELLER_PUBLIC_KEY_HEX: &str =
    "e0266e3cfb0d2886f91c73f5f868f3b98273713e5fcd97c081663f5518a4b3af";

fn projection_row() -> RadrootsListingProjectionRow {
    RadrootsListingProjectionRow {
        listing_addr: RadrootsListingAddress::parse(format!(
            "30402:{SELLER_PUBLIC_KEY_HEX}:AAAAAAAAAAAAAAAAAAAAAg"
        ))
        .expect("listing address"),
        listing_event_id: "a".repeat(64),
        seller_pubkey: SELLER_PUBLIC_KEY_HEX.to_owned(),
        title: "Blueberries".to_owned(),
        description: "Fresh field berries".to_owned(),
        product_type: "fruit".to_owned(),
        price_amount: "6".to_owned(),
        price_currency: "USD".to_owned(),
        inventory_available: "12".to_owned(),
        delivery_method: "pickup".to_owned(),
        locality_primary: "Fernwood".to_owned(),
        locality_city: Some("Victoria".to_owned()),
        locality_region: Some("BC".to_owned()),
        locality_country: Some("CA".to_owned()),
        geohash5: "c2b2q".to_owned(),
        updated_at_ms: 1_700_000_000_000,
    }
}

fn projection_error_message(error: RadrootsSdkError) -> String {
    match error {
        RadrootsSdkError::Projection { message } => message,
        other => panic!("expected projection error, got {other:?}"),
    }
}

#[test]
fn market_search_request_builders_preserve_refresh_contract() {
    let request = MarketSearchRequest::new("berries")
        .with_limit(7)
        .with_projection_refresh(SyncProjectionRefreshRequest::new().with_limit(3));

    assert_eq!(request.query, "berries");
    assert_eq!(request.limit, 7);
    assert_eq!(request.projection_refresh.limit, 3);
}

#[test]
fn listing_projection_row_conversion_validates_stored_identity_columns() {
    let row = projection_row();
    let search_row =
        MarketListingSearchRow::try_from_projection_row(row.clone()).expect("search row");
    assert_eq!(search_row.listing_addr, row.listing_addr);
    assert_eq!(search_row.listing_event_id.as_str(), row.listing_event_id);
    assert_eq!(search_row.seller_pubkey.as_str(), row.seller_pubkey);
    assert_eq!(search_row.title, "Blueberries");

    let mut invalid_event_id = row.clone();
    invalid_event_id.listing_event_id = "not-an-event-id".to_owned();
    assert!(
        projection_error_message(
            MarketListingSearchRow::try_from_projection_row(invalid_event_id).unwrap_err()
        )
        .contains("projection event id")
    );

    let mut invalid_seller = row;
    invalid_seller.seller_pubkey = "not-a-pubkey".to_owned();
    assert!(
        projection_error_message(
            MarketListingSearchRow::try_from_projection_row(invalid_seller).unwrap_err()
        )
        .contains("projection seller pubkey")
    );
}

#[test]
fn market_search_receipt_serializes_absent_optional_locality_fields() {
    let mut row = projection_row();
    row.locality_city = None;
    row.locality_region = None;
    row.locality_country = None;
    let search_row = MarketListingSearchRow::try_from_projection_row(row).expect("search row");
    let receipt = MarketSearchReceipt {
        source: MarketSearchSource::LocalProjectionFts,
        refresh: SyncProjectionRefreshReceipt::default(),
        listings: vec![search_row],
    };

    let value = serde_json::to_value(receipt).expect("receipt json");

    assert_eq!(value["source"], "local_projection_fts");
    assert!(value["listings"][0]["locality_city"].is_null());
    assert!(value["listings"][0]["locality_region"].is_null());
    assert!(value["listings"][0]["locality_country"].is_null());
}

#[tokio::test]
async fn market_search_reports_projection_refresh_errors_before_querying_rows() {
    let sdk = crate::RadrootsClient::builder()
        .clock(crate::RadrootsSdkClock::BeforeUnixEpoch)
        .build()
        .await
        .expect("sdk");

    assert!(matches!(
        sdk.market()
            .search(MarketSearchRequest::new("berries"))
            .await,
        Err(RadrootsSdkError::ClockBeforeUnixEpoch)
    ));
}
