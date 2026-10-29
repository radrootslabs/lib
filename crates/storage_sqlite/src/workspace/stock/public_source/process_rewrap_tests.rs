//! Private F13/F14 process boundaries reuse the actual native owner and keys.
use super::*;
use radroots_domain::EnvelopeEventId;
use radroots_storage::{command::PrivateEnvelopeCandidate, inventory::*};
use sha2::{Digest, Sha256};
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Original {
    id: OperationId,
    event: EnvelopeEventId,
    transaction: TransactionId,
    digest: [u8; 32],
}
fn base(
    binding: radroots_storage::workspace::WorkspaceBinding,
    minimum: InventoryCheckpointRef,
    c: &Control,
) -> StockEnvelopeRequest {
    StockEnvelopeRequest::new(binding, minimum, c.operation, c.digest, c.recipient, None).unwrap()
}
fn request(
    binding: radroots_storage::workspace::WorkspaceBinding,
    minimum: InventoryCheckpointRef,
    c: &Control,
    selected: bool,
) -> StockRewrapRequest {
    let request = StockRewrapRequest::new(base(binding, minimum, c), c.rewrap.as_ref().unwrap().id);
    if selected {
        request.with_envelope(delivery::candidate(c).event_id())
    } else {
        request
    }
}
fn proof(r: &Rig) -> AttachmentProof {
    r.enrollment
        .proof()
        .with_secret(|bytes| AttachmentProof::from_protected_input(bytes))
        .unwrap()
}
pub(super) async fn prepare(r: &Rig, c: &mut Control) {
    let StockEnvelopeOutcome::Committed(first) = r
        .store
        .capture_stock_envelope(
            base(r.binding.clone(), inventory_minimum(r).await, c),
            proof(r),
            &delivery::candidate(c),
        )
        .await
        .unwrap()
    else {
        panic!("original F10 required")
    };
    c.rewrap = Some(Original {
        id: OperationId::from_bytes([28; 32]),
        event: first.event_id(),
        transaction: first.attempt().transaction_id().unwrap(),
        digest: *first.digest(),
    });
    if matches!(c.edge, Edge::RewrapAdmission) {
        return;
    }
    assert!(matches!(
        r.store
            .prepare_stock_rewrap(
                request(r.binding.clone(), inventory_minimum(r).await, c, false),
                proof(r)
            )
            .await
            .unwrap(),
        StockRewrapOutcome::Claimed(_)
    ));
    let source = r
        .store
        .stock_signing_evidence(
            StockClaimRequest::new(
                r.binding.clone(),
                inventory_minimum(r).await,
                c.operation,
                c.digest,
                c.recipient,
            ),
            *first.evidence().digest(),
            r.enrollment.proof(),
        )
        .await
        .unwrap()
        .unwrap();
    let wrapped = source.with_json(|raw| {
        radroots_nostr::signing::gift_wrap::wrap_retained_seal(
            raw,
            c.recipient,
            std::num::NonZeroU64::new(150).unwrap(),
            &radroots_nostr::signing::OsNonceSource,
        )
        .unwrap()
    });
    c.raw = wrapped.signed_json().to_owned();
    c.wrapping_key = Some(wrapped.with_wrapping_key(|key| *key));
    assert_ne!(delivery::candidate(c).event_id(), first.event_id());
    if matches!(c.edge, Edge::RewrapEnvelope) {
        return;
    }
    assert!(matches!(
        r.store
            .capture_stock_rewrap(
                request(r.binding.clone(), inventory_minimum(r).await, c, true),
                proof(r),
                &delivery::candidate(c)
            )
            .await
            .unwrap(),
        StockEnvelopeOutcome::Committed(_)
    ));
    if matches!(c.edge, Edge::RewrapObservation) {
        let StockDeliveryAttemptOutcome::Claimed(attempt) = r
            .store
            .prepare_stock_delivery_attempt(
                delivery::request(r.binding.clone(), inventory_minimum(r).await, c),
                proof(r),
            )
            .await
            .unwrap()
        else {
            panic!("selected D required")
        };
        c.delivery_transaction = Some(attempt.transaction_id());
    }
}
pub(super) async fn apply(
    store: &NativeWorkspace,
    binding: radroots_storage::workspace::WorkspaceBinding,
    minimum: InventoryCheckpointRef,
    proof: &AttachmentProof,
    c: &Control,
) -> TransactionId {
    let credential = proof
        .with_secret(|bytes| AttachmentProof::from_protected_input(bytes))
        .unwrap();
    match c.edge {
        Edge::RewrapAdmission => {
            let StockRewrapOutcome::Claimed(admitted) = store
                .prepare_stock_rewrap(request(binding, minimum, c, false), credential)
                .await
                .unwrap()
            else {
                panic!("new confirmed admission required")
            };
            admitted.transaction_id()
        }
        Edge::RewrapEnvelope => {
            let candidate: PrivateEnvelopeCandidate = delivery::candidate(c);
            let StockEnvelopeOutcome::Committed(saved) = store
                .capture_stock_rewrap(request(binding, minimum, c, true), credential, &candidate)
                .await
                .unwrap()
            else {
                panic!("confirmed additional outer required")
            };
            saved.attempt().transaction_id().unwrap()
        }
        _ => unreachable!(),
    }
}
pub(super) async fn verify(r: &Rig, c: &Control, transaction: Option<TransactionId>) {
    let original = c.rewrap.as_ref().unwrap();
    let first = r
        .store
        .stock_envelope(
            base(r.binding.clone(), inventory_minimum(r).await, c),
            proof(r),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(first.receipt().event_id(), original.event);
    assert_eq!(
        first.receipt().attempt().transaction_id(),
        Some(original.transaction)
    );
    first.with_json(|raw| {
        assert_eq!(
            <[u8; 32]>::from(Sha256::digest(raw.as_bytes())),
            original.digest
        )
    });
    let state = r
        .store
        .stock_rewrap_attempt(
            request(r.binding.clone(), inventory_minimum(r).await, c, false),
            proof(r),
        )
        .await
        .unwrap();
    if matches!(c.edge, Edge::RewrapAdmission) {
        assert_eq!(state.is_some(), !c.before);
        assert_eq!(transaction.is_some(), !c.before);
        if let Some(state) = state {
            assert!(state.envelope().is_none());
            assert_eq!(Some(state.admission().transaction_id()), transaction);
            let StockRewrapOutcome::Retained(repeated) = r
                .store
                .prepare_stock_rewrap(
                    request(r.binding.clone(), inventory_minimum(r).await, c, false),
                    proof(r),
                )
                .await
                .unwrap()
            else {
                panic!("reopened admission grants no wrapping")
            };
            assert_eq!(*repeated, state.admission());
        }
    } else if matches!(c.edge, Edge::RewrapEnvelope) {
        let state = state.unwrap();
        assert_eq!(state.envelope().is_some(), !c.before);
        assert_eq!(
            state.envelope().and_then(|v| v.attempt().transaction_id()),
            transaction
        );
        let StockEnvelopeOutcome::Committed(saved) = r
            .store
            .reconcile_stock_rewrap(
                request(r.binding.clone(), inventory_minimum(r).await, c, true),
                proof(r),
                &delivery::candidate(c),
            )
            .await
            .unwrap()
        else {
            panic!("original obtained outer must reconcile")
        };
        assert_eq!(
            saved.rewrap_admission_transaction_id(),
            Some(state.admission().transaction_id())
        );
        if let Some(original) = state.envelope() {
            assert_eq!(*saved, original);
        }
        let found = r
            .store
            .stock_envelope(
                delivery::source(r.binding.clone(), inventory_minimum(r).await, c),
                proof(r),
            )
            .await
            .unwrap()
            .unwrap();
        found.with_json(|raw| assert_eq!(raw, c.raw));
        assert_eq!(found.receipt(), *saved);
    } else {
        delivery::verify(r, c, transaction).await;
        let summary = r
            .store
            .stock_delivery_summary(
                base(r.binding.clone(), inventory_minimum(r).await, c),
                proof(r),
            )
            .await
            .unwrap();
        assert!(!summary.acceptance_satisfied().unwrap());
    }
    let first = r
        .store
        .stock_envelope(
            base(r.binding.clone(), inventory_minimum(r).await, c),
            proof(r),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        first.receipt().attempt().transaction_id(),
        Some(original.transaction)
    );
    first.with_json(|raw| {
        assert_eq!(
            <[u8; 32]>::from(Sha256::digest(raw.as_bytes())),
            original.digest
        )
    });
}
