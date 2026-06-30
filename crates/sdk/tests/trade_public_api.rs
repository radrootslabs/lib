#![cfg(feature = "runtime")]

use radroots_sdk::{RadrootsClient, TradeResyncRequest, TradeStatusKind, TradeStatusRequest};

#[tokio::test]
async fn grouped_trade_surface_is_the_public_product_entrypoint() {
    let sdk = RadrootsClient::builder().build().await.expect("sdk");
    let trades = sdk.trades();

    let _buyer = trades.buyer();
    let _seller = trades.seller();
    let _resync = trades.resync();

    let status = trades
        .status(TradeStatusRequest::parse("trade-public-api-order").expect("status request"))
        .await
        .expect("status");

    assert_eq!(status.status, TradeStatusKind::Missing);

    let resync = trades
        .resync()
        .resync(TradeResyncRequest::new(status.locator))
        .await
        .expect("resync");

    assert_eq!(resync.status.status, TradeStatusKind::Missing);
}
