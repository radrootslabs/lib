//! Coordinator-only hostile-report models; these confer no stock authority.
use super::*;
use crate::session::commerce::{
    AcceptanceUnissued, AcceptanceUnissuedOutcome, AcceptanceUnissuedReceipt,
};
use alloc::boxed::Box;
use radroots_domain::TransactionId;
use radroots_signing::provider::ProviderError;
use std::sync::atomic::{AtomicUsize, Ordering};
struct Observation {
    claim: AcceptanceClaim,
    receipt: AcceptanceUnissuedReceipt,
    mode: Shared<AtomicUsize>,
}
impl AcceptanceUnissued for Observation {
    fn claim(&self) -> AcceptanceClaim {
        let mut claim = self.claim;
        if self.mode.load(Ordering::SeqCst) == 9 {
            claim.id = SigningAttemptId::from_bytes([0xfa; 32]);
        }
        claim
    }
    fn reason(&self) -> ProviderError {
        ProviderError::Cancelled
    }
    fn capture(
        &self,
    ) -> crate::session::HostFuture<'_, Result<AcceptanceUnissuedOutcome, OperationError>> {
        Box::pin(async move {
            let mode = self.mode.load(Ordering::SeqCst);
            let mut receipt = self.receipt;
            let a = receipt.attempt;
            receipt.attempt = radroots_storage::command::CommitAttempt::new(
                if mode == 1 {
                    WorkspaceId::from_bytes([0xfb; 32])
                } else {
                    a.workspace_id()
                },
                if mode == 2 {
                    OperationId::from_bytes([0xfb; 32])
                } else {
                    a.operation_id()
                },
                if mode == 3 {
                    CommandDigest::from_bytes([0xfb; 32])
                } else {
                    a.command_digest()
                },
                if mode == 4 { None } else { a.transaction_id() },
            );
            if mode == 5 {
                receipt.claim = SigningAttemptId::from_bytes([0xfb; 32]);
            }
            if mode == 6 {
                receipt.recipient = PublicKey::from_hex(
                    "f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9",
                )
                .unwrap();
            }
            if mode == 7 {
                receipt.reason = ProviderError::DeadlineExceeded;
            }
            if mode == 8 {
                receipt.checkpoint = InventoryCheckpointRef::from_host_metadata(
                    AuthorityRef::new(
                        ExecutorId::from_bytes([0xfb; 32]),
                        Generation::new(1).unwrap(),
                    ),
                    Sequence::new(0).unwrap(),
                    None,
                )
                .unwrap();
            }
            if mode == 10 {
                return Ok(AcceptanceUnissuedOutcome::Unknown(receipt.attempt));
            }
            if mode == 11 {
                return Err(OperationError::StorageFailure);
            }
            Ok(AcceptanceUnissuedOutcome::Committed(Box::new(receipt)))
        })
    }
    fn reconcile(
        &self,
    ) -> crate::session::HostFuture<'_, Result<AcceptanceUnissuedOutcome, OperationError>> {
        self.capture()
    }
}
#[test]
fn every_mismatched_or_uncertain_host_report_keeps_original_observation_and_lease_until_confirmed()
{
    for mode in 1..=11 {
        let (context, _artifact, weak) = fixture();
        let control = Shared::new(AtomicUsize::new(mode));
        let receipt = AcceptanceUnissuedReceipt {
            attempt: radroots_storage::command::CommitAttempt::new(
                context.selection.binding.input().workspace_id(),
                context.selection.operation,
                context.selection.digest,
                Some(TransactionId::from_bytes([0xaa; 32])),
            ),
            claim: context.claim.id,
            recipient: context.recipient,
            reason: ProviderError::Cancelled,
            checkpoint: context.selection.minimum,
        };
        assert_ne!(
            receipt.recipient.to_hex(),
            "f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9"
        );
        let owner = Owner::default();
        let operation = context.selection.operation;
        let observation = Box::new(Observation {
            claim: context.claim,
            receipt,
            mode: control.clone(),
        });
        let entry = owner.retain_unissued(
            context.selection,
            context.claim,
            context.recipient,
            observation,
            context.lease,
        );
        let result = block_on(owner.capture_unissued(&entry));
        if mode == 10 {
            assert!(matches!(result, Ok(AcceptanceUnissuedOutcome::Unknown(_))));
        } else {
            let error = result
                .err()
                .expect("mismatched or failed capture must refuse");
            assert_eq!(
                error.code(),
                if mode == 11 {
                    crate::Code::StorageFailure
                } else {
                    crate::Code::RecoveryRequired
                }
            );
            assert_eq!(error.context().operation_id(), Some(operation));
            assert_eq!(error.context().admission(), crate::Admission::Confirmed);
        }
        drop(entry);
        assert!(weak.upgrade().is_some());
        assert!(owner.ensure_ready().is_err());
        assert_eq!(owner.unissued.lock().unwrap().len(), 1);
        control.store(0, Ordering::SeqCst);
        block_on(owner.drain()).unwrap();
        assert!(owner.ensure_ready().is_ok());
        assert!(weak.upgrade().is_none());
    }
}
