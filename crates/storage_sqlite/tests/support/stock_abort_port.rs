use super::*;
use radroots_domain::{CommandDigest, OperationId, Sequence};
use radroots_storage::command::OperationError;
use radroots_storage::inventory::{InventoryCheckpointRef, InventoryStore, StockAbortRequest};

#[tokio::test]
async fn shared_abort_port_never_upgrades_an_ordinary_outbox_or_fabricates_cleanup() {
    let f = Fixture::new();
    let (store, input, proof) = f.create().await;
    let binding = store.authenticate(input.clone(), &proof).await.unwrap();
    let minimum = InventoryCheckpointRef::from_host_metadata(
        input.authority(),
        Sequence::new(0).unwrap(),
        None,
    )
    .unwrap();
    let selected = || {
        StockAbortRequest::new(
            binding.clone(),
            minimum,
            OperationId::from_bytes([80; 32]),
            CommandDigest::from_bytes([81; 32]),
        )
    };
    let copy = || {
        proof
            .with_secret(|bytes| AttachmentProof::from_protected_input(bytes))
            .unwrap()
    };
    let port: &dyn InventoryStore = &store;
    for error in [
        port.abort_proven_unissued(selected(), copy())
            .await
            .unwrap_err(),
        port.stock_preparation_abort(selected(), copy())
            .await
            .unwrap_err(),
    ] {
        assert_eq!(error, OperationError::UnsupportedOperation);
        assert_eq!(error.code(), "capability_unavailable");
    }
    assert!(!f.path.join("stock_execution_v1.sqlite").exists());
    store.authenticate(input, &proof).await.unwrap();
    store.close().await.unwrap();
}
