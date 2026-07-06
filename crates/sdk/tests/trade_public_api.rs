#![cfg(feature = "runtime")]

use radroots_authority::RadrootsActorContext;
use radroots_events::contract::RadrootsActorRole;
use radroots_sdk::{
    RadrootsClient, TradeResyncRequest, TradeSellerInboxRequest, TradeStatusKind,
    TradeStatusRequest, TradeValidationReceiptListRequest,
};

const SELLER_PUBLIC_KEY_HEX: &str =
    "e0266e3cfb0d2886f91c73f5f868f3b98273713e5fcd97c081663f5518a4b3af";

#[tokio::test]
async fn grouped_trade_surface_is_the_public_product_entrypoint() {
    let sdk = RadrootsClient::builder().build().await.expect("sdk");
    let trades = sdk.trades();

    let _seller = trades.seller();
    let _resync = trades.resync();
    let validation_receipts = trades.validation_receipts();

    let status = trades
        .status(TradeStatusRequest::parse("trade-public-api-order").expect("status request"))
        .await
        .expect("status");

    assert_eq!(status.status, TradeStatusKind::Missing);

    let resync_error = trades
        .resync()
        .resync(TradeResyncRequest::new(status.locator))
        .await
        .expect_err("resync requires configured relays");

    assert_eq!(resync_error.code(), "empty_transport_targets");

    let validation_receipt_error = validation_receipts
        .list(
            TradeValidationReceiptListRequest::parse("trade-public-api-order")
                .expect("validation receipt list request"),
        )
        .await
        .expect_err("validation receipt list requires configured relays");

    assert_eq!(validation_receipt_error.code(), "empty_transport_targets");

    let seller_actor =
        RadrootsActorContext::test(SELLER_PUBLIC_KEY_HEX, [RadrootsActorRole::Seller])
            .expect("seller actor");
    let inbox = trades
        .seller()
        .inbox(TradeSellerInboxRequest::new(seller_actor))
        .await
        .expect("seller inbox");

    assert_eq!(inbox.seller_pubkey.as_str(), SELLER_PUBLIC_KEY_HEX);
    assert!(inbox.statuses.is_empty());
}
