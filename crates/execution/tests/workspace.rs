#![cfg(feature = "workspace_session")]
// Port-model tests for captured context invariants. Concrete native credential,
// persistence and cancellation qualification belongs to its actual backend.
use futures::{executor::block_on, task::AtomicWaker};
use radroots_domain::*;
use radroots_execution::session::{ClientSession, HostFuture, SessionHost};
use radroots_execution::workspace::WorkspaceSession;
use radroots_storage::workspace::{
    AttachmentInput, AttachmentProof, WorkspaceBinding, WorkspaceError, WorkspaceStore,
};
#[cfg(target_arch = "wasm32")]
use std::rc::Rc as Shared;
#[cfg(not(target_arch = "wasm32"))]
use std::sync::Arc as Shared;
use std::{
    future::poll_fn,
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
    task::Poll,
};

struct Model {
    binding: Mutex<WorkspaceBinding>,
    error: Mutex<Option<WorkspaceError>>,
    pending: AtomicBool,
    waker: AtomicWaker,
}
impl WorkspaceStore for Model {
    fn authenticate(
        &self,
        _input: AttachmentInput,
        _proof: AttachmentProof,
    ) -> radroots_storage::event::BoxFuture<'_, Result<WorkspaceBinding, WorkspaceError>> {
        Box::pin(async move {
            poll_fn(|cx| {
                self.waker.register(cx.waker());
                if self.pending.load(Ordering::Acquire) {
                    Poll::Pending
                } else {
                    Poll::Ready(())
                }
            })
            .await;
            if let Some(error) = *self.error.lock().unwrap() {
                return Err(error);
            }
            Ok(self.binding.lock().unwrap().clone())
        })
    }
    fn close_workspace(
        &self,
    ) -> radroots_storage::event::BoxFuture<'_, Result<(), WorkspaceError>> {
        Box::pin(async { Ok(()) })
    }
}
fn key() -> PublicKey {
    PublicKey::from_hex("79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798").unwrap()
}
fn input(route: u8) -> AttachmentInput {
    AttachmentInput::new(
        WorkspaceId::from_bytes([1; 32]),
        AuthorityRef::new(ExecutorId::from_bytes([2; 32]), Generation::new(1).unwrap()),
        key(),
        "local://model",
        key(),
        GrantId::from_bytes([3; 32]),
        CheckpointRef::new(
            CheckpointDigest::from_bytes([4; 32]),
            Sequence::new(0).unwrap(),
            Coverage::Partial,
            vec![],
        )
        .unwrap(),
        RouteDigest::from_bytes([route; 32]),
    )
    .unwrap()
}
fn binding(change: u8, observed: u64) -> WorkspaceBinding {
    let input = input(if change == 1 { 9 } else { 5 });
    WorkspaceBinding::from_authenticated_host(
        input.clone(),
        ActorRef::new(
            key(),
            if change == 2 {
                ActorRole::Seller
            } else {
                ActorRole::Buyer
            },
            None,
            Some(input.grant_id()),
        ),
        if change == 3 {
            "other-device"
        } else {
            "device"
        },
        Sequence::new(if change == 4 { 2 } else { 1 }).unwrap(),
        UnixSeconds::new(if change == 5 { 201 } else { 200 }).unwrap(),
        UnixSeconds::new(observed).unwrap(),
    )
    .unwrap()
}
fn new_model() -> Shared<Model> {
    Shared::new(Model {
        binding: Mutex::new(binding(0, 100)),
        error: Mutex::new(None),
        pending: AtomicBool::new(false),
        waker: AtomicWaker::new(),
    })
}
fn proof() -> AttachmentProof {
    AttachmentProof::from_protected_input(&[7; 32]).unwrap()
}

struct Host;
impl SessionHost for Host {
    fn start(&self) -> HostFuture<'_, radroots_execution::Result<()>> {
        Box::pin(async { Ok(()) })
    }
    fn quiesce(&self) -> HostFuture<'_, radroots_execution::Result<()>> {
        Box::pin(async { Ok(()) })
    }
    fn disconnect(&self) -> HostFuture<'_, radroots_execution::Result<()>> {
        Box::pin(async { Ok(()) })
    }
    fn dispose_callbacks(&self) -> HostFuture<'_, radroots_execution::Result<()>> {
        Box::pin(async { Ok(()) })
    }
    fn close_storage(&self) -> HostFuture<'_, radroots_execution::Result<()>> {
        Box::pin(async { Ok(()) })
    }
}
#[test]
fn client_bounds_and_retires_scopes_and_refuses_attachment_finishing_after_quiesce() {
    block_on(async {
        let model = new_model();
        let client = ClientSession::new(Host, core::num::NonZeroU16::MIN);
        let mut scopes = Vec::new();
        for _ in 0..16 {
            scopes.push(
                client
                    .attach_workspace(model.clone(), input(5), proof())
                    .await
                    .unwrap(),
            );
        }
        assert_eq!(
            client
                .attach_workspace(model.clone(), input(5), proof())
                .await
                .unwrap_err()
                .code(),
            radroots_execution::Code::CapabilityUnavailable
        );
        scopes[0].close();
        scopes.push(
            client
                .attach_workspace(model.clone(), input(5), proof())
                .await
                .unwrap(),
        );
        model.pending.store(true, Ordering::Release);
        let mut attachment = Box::pin(client.attach_workspace(model.clone(), input(5), proof()));
        assert!(futures::poll!(attachment.as_mut()).is_pending());
        let mut closing = Box::pin(client.close());
        assert!(futures::poll!(closing.as_mut()).is_pending());
        for scope in &scopes {
            assert!(
                scope
                    .command_context(
                        OperationId::from_bytes([10; 32]),
                        None,
                        None,
                        UnixSeconds::new(150).unwrap()
                    )
                    .is_err()
            );
        }
        model.pending.store(false, Ordering::Release);
        model.waker.wake();
        assert_eq!(
            attachment.await.unwrap_err().code(),
            radroots_execution::Code::CapabilityUnavailable
        );
        closing.await.unwrap();
        assert!(
            client
                .attach_workspace(model, input(5), proof())
                .await
                .is_err()
        );
    })
}
#[test]
fn captured_context_retains_all_fields_and_refuses_each_semantic_grant_change() {
    block_on(async {
        let model = new_model();
        let session = WorkspaceSession::attach(model.clone(), input(5), proof())
            .await
            .unwrap();
        let expected = ExpectedOrder::new(
            OrderId::from_bytes([8; 32]),
            None,
            Sequence::new(0).unwrap(),
            None,
            input(5).checkpoint().clone(),
        );
        let approval = Approval::new(
            ApprovalIntentDigest::from_bytes([9; 32]),
            "scope",
            UnixSeconds::new(160).unwrap(),
        )
        .unwrap();
        let context = session
            .command_context_with_correlation(
                OperationId::from_bytes([10; 32]),
                Some(expected.clone()),
                Some(approval.clone()),
                UnixSeconds::new(150).unwrap(),
                Some(CorrelationId::new("diagnostic").unwrap()),
            )
            .unwrap();
        assert_eq!(context.data().expected(), Some(&expected));
        assert_eq!(context.data().approval(), Some(&approval));
        assert_eq!(context.data().actor(), session.binding().actor());
        assert_eq!(format!("{session:?}"), "WorkspaceSession(<redacted>)");
        assert_eq!(format!("{context:?}"), "CommandContext(<redacted>)");
        context.revalidate().await.unwrap();
        for change in 1..=5 {
            *model.binding.lock().unwrap() = binding(change, 100);
            assert_eq!(
                context.revalidate().await.unwrap_err().code(),
                radroots_execution::Code::GrantRevoked
            );
        }
        *model.binding.lock().unwrap() = binding(0, 150);
        assert_eq!(
            context.revalidate().await.unwrap_err().code(),
            radroots_execution::Code::Unauthorized
        );
        *model.binding.lock().unwrap() = binding(0, 100);
        for deadline in [0, 100, 201] {
            assert!(
                session
                    .command_context(
                        OperationId::from_bytes([10; 32]),
                        None,
                        None,
                        UnixSeconds::new(deadline).unwrap()
                    )
                    .is_err()
            );
        }
        *model.error.lock().unwrap() = Some(WorkspaceError::Unavailable);
        // Construction is pure even while the host is unavailable; admission must
        // perform the separate current-rights check and refuse this unavailable host.
        session
            .command_context(
                OperationId::from_bytes([10; 32]),
                None,
                None,
                UnixSeconds::new(150).unwrap(),
            )
            .unwrap();
        assert_eq!(
            context.revalidate().await.unwrap_err().code(),
            radroots_execution::Code::CapabilityUnavailable
        );
    })
}

#[test]
fn pre_admission_workspace_errors_retain_proposed_identity_and_typed_uncertainty() {
    block_on(async {
        use WorkspaceError as W;
        use radroots_execution::{Admission, Code as C, RetryClass};
        let model = new_model();
        let session = WorkspaceSession::attach(model.clone(), input(5), proof())
            .await
            .unwrap();
        let operation = OperationId::from_bytes([10; 32]);
        let context = session
            .command_context(operation, None, None, UnixSeconds::new(150).unwrap())
            .unwrap();
        for (source, code) in [
            (W::InvalidInput, C::InvalidInput),
            (W::Unauthorized, C::Unauthorized),
            (W::GrantExpired, C::Unauthorized),
            (W::GrantRevoked, C::GrantRevoked),
            (W::WriterActive, C::MissingAuthority),
            (W::WrongNamespace, C::Unauthorized),
            (W::StaleAuthority, C::StaleState),
            (W::StaleCheckpoint, C::StaleState),
            (W::Unavailable, C::CapabilityUnavailable),
            (W::Closed, C::CapabilityUnavailable),
            (W::StorageFailure, C::StorageFailure),
            (W::NamespaceUnavailable, C::RecoveryRequired),
            (W::RecoveryRequired, C::RecoveryRequired),
            (W::CommitUnknown, C::RecoveryRequired),
        ] {
            *model.error.lock().unwrap() = Some(source);
            let error = context.revalidate().await.unwrap_err();
            assert_eq!(error.code(), code);
            assert_eq!(error.context().operation_id(), Some(operation));
            assert_eq!(error.context().admission(), Admission::NotAdmitted);
            assert!(error.detail().is_some());
            if source == W::CommitUnknown {
                assert_eq!(error.retry(), RetryClass::ManualReconciliation);
            }
        }
    })
}
#[test]
fn attachment_never_trusts_an_adapter_result_for_another_selection() {
    block_on(async {
        let model = new_model();
        *model.binding.lock().unwrap() = binding(1, 100);
        assert_eq!(
            WorkspaceSession::attach(model, input(5), proof())
                .await
                .unwrap_err()
                .code(),
            radroots_execution::Code::Unauthorized
        );
        let model = new_model();
        *model.error.lock().unwrap() = Some(WorkspaceError::Unauthorized);
        assert_eq!(
            WorkspaceSession::attach(model, input(5), proof())
                .await
                .unwrap_err()
                .code(),
            radroots_execution::Code::Unauthorized
        );
    })
}
#[test]
fn close_invalidates_clones_and_an_already_pending_context_revalidation() {
    block_on(async {
        let model = new_model();
        let session = WorkspaceSession::attach(model.clone(), input(5), proof())
            .await
            .unwrap();
        let context = session
            .command_context(
                OperationId::from_bytes([10; 32]),
                None,
                None,
                UnixSeconds::new(150).unwrap(),
            )
            .unwrap();
        model.pending.store(true, Ordering::Release);
        let mut pending = Box::pin(context.revalidate());
        assert!(futures::poll!(pending.as_mut()).is_pending());
        session.clone().close();
        model.pending.store(false, Ordering::Release);
        model.waker.wake();
        assert_eq!(
            pending.await.unwrap_err().code(),
            radroots_execution::Code::CapabilityUnavailable
        );
        assert_eq!(
            context.revalidate().await.unwrap_err().code(),
            radroots_execution::Code::CapabilityUnavailable
        );
        assert!(
            session
                .command_context(
                    OperationId::from_bytes([10; 32]),
                    None,
                    None,
                    UnixSeconds::new(150).unwrap()
                )
                .is_err()
        );
    })
}

#[path = "support/workspace_commands.rs"]
mod commands;
