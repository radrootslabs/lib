//! Original invocation observations on actual claims, separate from abort.
use super::*;
use crate::workspace::StockDispatchOutcome;
use radroots_signing::{
    issuance::{ExactIssuer, IntentBinding, RejectionReason, SigningIntent, SigningOutcome},
    provider::ProviderError,
    signer::BoxFuture,
};
use std::sync::atomic::{AtomicUsize, Ordering};
static CLOCK_MODE: AtomicUsize = AtomicUsize::new(0);
static CLOCK_CALLS: AtomicUsize = AtomicUsize::new(0);
fn clock() -> Result<u64, radroots_signing::Error> {
    CLOCK_CALLS.fetch_add(1, Ordering::SeqCst);
    match CLOCK_MODE.load(Ordering::SeqCst) {
        0 => Ok(0),
        1 => Ok(230000),
        2 => Err(radroots_signing::Error::new(
            radroots_signing::error::Kind::InternalError,
        )),
        _ => Ok(150000),
    }
}
#[tokio::test]
async fn owned_original_local_refusal_retains_claim_and_holds_without_authorizing_abort_or_reissue()
{
    for (mode, reason) in [
        (0, ProviderError::ProviderFailure),
        (1, ProviderError::DeadlineExceeded),
        (2, ProviderError::ProviderFailure),
        (3, ProviderError::Cancelled),
    ] {
        let mut r = Rig::new().await;
        let receipt = ready(&r).await;
        let recipient = receipt.rumor().recipients()[0];
        let StockClaimOutcome::Fresh(permit) = claim(&r, &receipt, recipient).await.unwrap() else {
            panic!("new committed claim");
        };
        let original = permit.report().clone();
        let minimum = inventory_minimum(&r).await;
        CLOCK_MODE.store(mode, Ordering::SeqCst);
        CLOCK_CALLS.store(0, Ordering::SeqCst);
        let signer =
            LocalSigner::with_clock(SecretKey::parse(&format!("{:064x}", 3)).unwrap(), clock)
                .unwrap();
        let cancellation = CancellationSignal::new();
        if mode == 3 {
            cancellation.cancel();
        }
        let StockDispatchOutcome::Unissued(observation) =
            permit.dispatch(&signer, cancellation).await.unwrap()
        else {
            panic!("actual qualified local pre-crypto refusal");
        };
        assert_eq!(CLOCK_CALLS.load(Ordering::SeqCst), 1);
        assert_eq!(observation.attempt().attempt_id(), original.attempt_id());
        assert_eq!(observation.attempt().binding(), original.binding());
        assert_eq!(
            observation.attempt().transaction_id(),
            original.transaction_id()
        );
        assert_eq!(observation.reason(), reason);
        assert_eq!(inventory_minimum(&r).await, minimum);
        assert!(format!("{observation:?}").contains("<redacted>"));
        assert!(matches!(
            claim(&r, &receipt, recipient).await.unwrap(),
            StockClaimOutcome::Existing(_)
        ));
        let mut raw = database(&r).await;
        let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM stock_signing_evidence")
            .fetch_one(&mut raw)
            .await
            .unwrap();
        assert_eq!(rows, 0);
        raw.close().await.unwrap();
        let abort = radroots_storage::inventory::StockAbortRequest::new(
            r.binding.clone(),
            inventory_minimum(&r).await,
            receipt.attempt().operation_id(),
            receipt.attempt().command_digest(),
        );
        assert!(matches!(
            radroots_storage::inventory::InventoryStore::abort_proven_unissued(
                &r.store,
                abort,
                r.enrollment
                    .proof()
                    .with_secret(|bytes| AttachmentProof::from_protected_input(bytes))
                    .unwrap(),
            )
            .await,
            Err(OperationError::RecoveryRequired)
        ));
        assert_eq!(inventory_minimum(&r).await, minimum);
        assert_pool(&r, 80, 9, 1, 2).await;
        assert_pool(&r, 81, 8, 2, 2).await;
        drop(observation);
        reopen_plain(&mut r).await;
        assert_eq!(
            read(&r, &receipt, recipient)
                .await
                .unwrap()
                .unwrap()
                .binding(),
            original.binding()
        );
        assert!(matches!(
            claim(&r, &receipt, recipient).await.unwrap(),
            StockClaimOutcome::Existing(_)
        ));
        r.store.close().await.unwrap();
    }
}

// Controlled IO classification model, not qualification of a remote adapter.
struct Callback {
    foreign: IntentBinding,
    mode: u8,
    calls: AtomicUsize,
}
impl ExactIssuer for Callback {
    fn issue_exact(&self, intent: SigningIntent) -> BoxFuture<'_, SigningOutcome> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Box::pin(async move {
            match self.mode {
                0 => SigningOutcome::not_dispatched(self.foreign, ProviderError::Cancelled),
                1 => SigningOutcome::unknown(intent.binding(), ProviderError::ProviderFailure),
                _ => SigningOutcome::capture(&intent, "{}".into(), None),
            }
        })
    }
}
#[tokio::test]
async fn foreign_refusal_unknown_and_malformed_output_never_create_original_unissued_observations()
{
    for mode in 0..3 {
        let r = Rig::new().await;
        let receipt = ready(&r).await;
        let recipient = receipt.rumor().recipients()[0];
        let other = receipt.rumor().recipients()[1];
        let StockClaimOutcome::Fresh(permit) = claim(&r, &receipt, recipient).await.unwrap() else {
            panic!("first claim")
        };
        let StockClaimOutcome::Fresh(foreign) = claim(&r, &receipt, other).await.unwrap() else {
            panic!("second claim")
        };
        let original = permit.report().clone();
        let callback = Callback {
            foreign: foreign.report().binding(),
            mode,
            calls: AtomicUsize::new(0),
        };
        drop(foreign);
        match permit
            .dispatch(&callback, CancellationSignal::new())
            .await
            .unwrap()
        {
            StockDispatchOutcome::Rejected { attempt, rejection } if mode != 1 => {
                assert_eq!(attempt.binding(), original.binding());
                if mode == 0 {
                    assert_eq!(rejection.reason(), RejectionReason::BindingMismatch);
                    assert_eq!(rejection.fingerprint(), None);
                } else {
                    assert!(rejection.fingerprint().is_some());
                }
            }
            StockDispatchOutcome::Unknown { attempt, reason } if mode == 1 => {
                assert_eq!(attempt.binding(), original.binding());
                assert_eq!(reason, ProviderError::ProviderFailure);
            }
            other => panic!("no modeled outcome can prove original nonissuance: {other:?}"),
        }
        assert_eq!(callback.calls.load(Ordering::SeqCst), 1);
        assert!(matches!(
            claim(&r, &receipt, recipient).await.unwrap(),
            StockClaimOutcome::Existing(_)
        ));
        assert_pool(&r, 80, 9, 1, 2).await;
        assert_pool(&r, 81, 8, 2, 2).await;
        r.store.close().await.unwrap();
    }
}
