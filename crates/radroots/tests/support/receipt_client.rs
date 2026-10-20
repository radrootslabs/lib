use super::*;
use radroots::{
    Client, CommandContext, NativeConfig, StartMode, WorkspaceSession, inbox::InquireInput,
};
use radroots_domain::{ChatMessage, ThreadId};
use radroots_execution::{
    Admission, BusinessInstallation, Code, Delivery, Issuance, OperationReceipt,
    RecoveryRequirement,
};
use radroots_storage::event::SourceGeneration;
use radroots_storage_sqlite::{OpenMode, OpenOptions, Paths};

struct Ordinary {
    fixture: Fixture,
    store: Arc<NativeWorkspace>,
    client: Client,
    scope: WorkspaceSession,
    input: AttachmentInput,
    proof: AttachmentProof,
}
impl Ordinary {
    async fn new() -> Self {
        let fixture = Fixture::new();
        let (store, input, proof, _) = fixture.create().await;
        let cache = fixture.path.parent().unwrap().join("cache");
        std::fs::create_dir(&cache).unwrap();
        let options = OpenOptions::new(Paths::from_directory(cache).unwrap(), OpenMode::Create)
            .with_source_generation(SourceGeneration::new([17; 32]).unwrap(), 1)
            .unwrap();
        let client =
            Client::open(NativeConfig::public_read(options).with_workspace(store.as_ref().clone()))
                .await
                .unwrap();
        let scope = client.attach(input.clone(), copy(&proof)).await.unwrap();
        Self {
            fixture,
            store,
            client,
            scope,
            input,
            proof,
        }
    }
    fn context(&self, id: u8, correlation: &str) -> CommandContext {
        self.scope
            .command_context_with_correlation(
                OperationId::from_bytes([id; 32]),
                None,
                None,
                UnixSeconds::new(150).unwrap(),
                Some(CorrelationId::new(correlation).unwrap()),
            )
            .unwrap()
    }
    async fn inquire(
        &self,
        id: u8,
        text: &str,
        correlation: &str,
    ) -> radroots::Result<OperationReceipt<ChatMessage>> {
        self.client
            .inbox()
            .inquire(self.context(id, correlation), input(text))
            .await
    }
    async fn start(&self) {
        self.client.start(StartMode::LocalOnly).await.unwrap();
    }
}
fn input(text: &str) -> InquireInput {
    InquireInput::new(ThreadId::from_bytes([0x66; 32]), key(), None, text).unwrap()
}

#[tokio::test]
async fn ordinary_inquiry_confirms_actual_t0_and_current_pending_receipt_without_signing() {
    let ready = Ordinary::new().await;
    let before = ready
        .inquire(91, "private ordinary inquiry", "first")
        .await
        .unwrap_err();
    assert_eq!(before.code(), Code::CapabilityUnavailable);
    assert_eq!(
        before.context().operation_id(),
        Some(OperationId::from_bytes([91; 32]))
    );
    assert_eq!(before.context().admission(), Admission::NotAdmitted);
    ready.start().await;
    let receipt = ready
        .inquire(91, "private ordinary inquiry", "first")
        .await
        .unwrap();
    assert!(receipt.durable());
    assert_eq!(
        receipt.business_installation(),
        BusinessInstallation::NotInstalled
    );
    assert!(receipt.business_result().is_none());
    assert_eq!(receipt.issuance(), Issuance::None);
    assert_eq!(receipt.delivery(), Delivery::NotStarted);
    assert_eq!(
        receipt.recovery(),
        &[RecoveryRequirement::ContinueOriginalOperation]
    );
    assert!(receipt.plan_transaction_id().is_none());
    assert!(
        receipt
            .artifacts()
            .iter()
            .all(|artifact| artifact.attempt_id().is_none())
    );
    assert!(!format!("{receipt:?}").contains("private ordinary"));
    let queried = ready
        .client
        .operations(&ready.scope)
        .get_matching(
            OperationId::from_bytes([91; 32]),
            Some(receipt.command_digest()),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        queried.admission_transaction_id(),
        receipt.admission_transaction_id()
    );
    assert_eq!(
        queried.operation_checkpoint(),
        receipt.operation_checkpoint()
    );
    let actual = ready
        .store
        .operation_snapshot(
            ready.scope.binding().clone(),
            OperationId::from_bytes([91; 32]),
            None,
            copy(&ready.proof),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        actual.prepared().request().chat_command().unwrap().text(),
        "private ordinary inquiry"
    );
    assert!(actual.bound_plan().is_none());
    assert!(actual.delivery_attempts().is_empty());
    ready.client.close().await.unwrap();
}

#[tokio::test]
async fn ordinary_replay_preserves_original_correlation_and_window_after_deadline() {
    let ready = Ordinary::new().await;
    ready.start().await;
    let receipt = ready
        .inquire(92, "retained inquiry", "first")
        .await
        .unwrap();
    ready.fixture.clock.now.store(151, Ordering::SeqCst);
    let replay = ready
        .inquire(92, "retained inquiry", "replacement")
        .await
        .unwrap();
    assert_eq!(
        replay.admission_transaction_id(),
        receipt.admission_transaction_id()
    );
    assert_eq!(replay.issuance_window(), receipt.issuance_window());
    assert_eq!(replay.context().correlation_id().unwrap().as_str(), "first");
    assert_eq!(replay.command_digest(), receipt.command_digest());
    assert_eq!(
        replay.operation_checkpoint(),
        receipt.operation_checkpoint()
    );
    assert_eq!(replay.error().unwrap().code(), Code::DeadlineExceeded);
    let conflict = ready
        .inquire(92, "changed private text", "replacement")
        .await
        .unwrap_err();
    assert_eq!(conflict.code(), Code::IdempotencyConflict);
    assert_eq!(
        conflict.context().operation_id(),
        Some(OperationId::from_bytes([92; 32]))
    );
    let new = ready
        .inquire(93, "new expired inquiry", "first")
        .await
        .unwrap_err();
    assert_eq!(new.code(), Code::DeadlineExceeded);
    assert!(
        ready
            .client
            .operations(&ready.scope)
            .get(OperationId::from_bytes([93; 32]))
            .await
            .unwrap()
            .is_none()
    );
    let mismatch = ready
        .client
        .operations(&ready.scope)
        .get_matching(
            OperationId::from_bytes([92; 32]),
            Some(CommandDigest::from_bytes([0x55; 32])),
        )
        .await
        .unwrap_err();
    assert_eq!(mismatch.code(), Code::IdempotencyConflict);
    ready.client.close().await.unwrap();
}

#[tokio::test]
async fn ordinary_context_must_belong_to_this_client_and_remain_open_and_authorized() {
    let ready = Ordinary::new().await;
    ready.start().await;
    let other_workspace = Ordinary::new().await;
    other_workspace.start().await;
    assert_eq!(
        other_workspace
            .client
            .inbox()
            .inquire(ready.context(94, "other journal"), input("private"))
            .await
            .unwrap_err()
            .code(),
        Code::Unauthorized
    );
    assert!(
        other_workspace
            .client
            .operations(&other_workspace.scope)
            .get(OperationId::from_bytes([94; 32]))
            .await
            .unwrap()
            .is_none()
    );
    other_workspace.client.close().await.unwrap();
    let other = Client::preview();
    other.start(StartMode::LocalOnly).await.unwrap();
    assert_eq!(
        other
            .inbox()
            .inquire(ready.context(94, "wrong client"), input("private"))
            .await
            .unwrap_err()
            .code(),
        Code::MissingAuthority
    );
    // The same persisted credential in a separately attached scope must not
    // bypass this client's attachment registry, even with the exact same store.
    let foreign =
        WorkspaceSession::attach(ready.store.clone(), ready.input.clone(), copy(&ready.proof))
            .await
            .unwrap();
    let context = foreign
        .command_context(
            OperationId::from_bytes([94; 32]),
            None,
            None,
            UnixSeconds::new(150).unwrap(),
        )
        .unwrap();
    assert_eq!(
        ready
            .client
            .inbox()
            .inquire(context, input("private"))
            .await
            .unwrap_err()
            .code(),
        Code::Unauthorized
    );
    assert_eq!(
        ready
            .client
            .operations(&foreign)
            .get(OperationId::from_bytes([94; 32]))
            .await
            .unwrap_err()
            .code(),
        Code::Unauthorized
    );
    assert!(
        ready
            .client
            .operations(&ready.scope)
            .get(OperationId::from_bytes([94; 32]))
            .await
            .unwrap()
            .is_none()
    );
    let closed_context = ready.context(94, "closed");
    ready.scope.close();
    assert_eq!(
        ready
            .client
            .inbox()
            .inquire(closed_context, input("private"))
            .await
            .unwrap_err()
            .code(),
        Code::CapabilityUnavailable
    );
    let scope = ready
        .client
        .attach(ready.input.clone(), copy(&ready.proof))
        .await
        .unwrap();
    let context = scope
        .command_context(
            OperationId::from_bytes([94; 32]),
            None,
            None,
            UnixSeconds::new(150).unwrap(),
        )
        .unwrap();
    ready
        .store
        .revoke(ready.input.clone(), &ready.proof)
        .await
        .unwrap();
    assert_eq!(
        ready
            .client
            .inbox()
            .inquire(context, input("private"))
            .await
            .unwrap_err()
            .code(),
        Code::GrantRevoked
    );
    assert_eq!(
        ready
            .client
            .operations(&scope)
            .get(OperationId::from_bytes([94; 32]))
            .await
            .unwrap_err()
            .code(),
        Code::GrantRevoked
    );
    ready.client.close().await.unwrap();
    other.close().await.unwrap();
}

#[tokio::test]
async fn accepted_ordinary_inquiry_survives_observer_loss_and_client_close_drains_native_t0() {
    use std::{
        future::Future,
        task::{Context, Poll},
    };
    let ready = Ordinary::new().await;
    ready.start().await;
    let inbox = ready.client.inbox();
    let (pause, release) = ready.fixture.clock.arm(0);
    let mut wait = Box::pin(inbox.inquire(
        ready.context(95, "lost observer"),
        input("retained after wait loss"),
    ));
    assert!(matches!(
        wait.as_mut()
            .poll(&mut Context::from_waker(futures::task::noop_waker_ref())),
        Poll::Pending
    ));
    reached(pause).await;
    drop(wait);
    let mut close = Box::pin(ready.client.close());
    assert!(matches!(
        close
            .as_mut()
            .poll(&mut Context::from_waker(futures::task::noop_waker_ref())),
        Poll::Pending
    ));
    release.send(()).unwrap();
    close.await.unwrap();
    let reopened = ready.fixture.reopen(ready.input.clone()).await;
    let current = reopened
        .authenticate(ready.input.clone(), &ready.proof)
        .await
        .unwrap();
    let snapshot = reopened
        .operation_snapshot(
            current,
            OperationId::from_bytes([95; 32]),
            None,
            copy(&ready.proof),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        snapshot.prepared().request().chat_command().unwrap().text(),
        "retained after wait loss"
    );
    assert!(snapshot.bound_plan().is_none());
    assert!(snapshot.publications().is_empty());
    reopened.close().await.unwrap();
}

#[tokio::test]
async fn unpolled_ordinary_inquiry_and_post_close_admission_have_no_effect() {
    let ready = Ordinary::new().await;
    ready.start().await;
    let inbox = ready.client.inbox();
    drop(inbox.inquire(ready.context(96, "not polled"), input("not submitted")));
    assert!(
        ready
            .client
            .operations(&ready.scope)
            .get(OperationId::from_bytes([96; 32]))
            .await
            .unwrap()
            .is_none()
    );
    let context = ready.context(96, "after close");
    ready.client.close().await.unwrap();
    let error = inbox
        .inquire(context, input("not submitted"))
        .await
        .unwrap_err();
    assert_eq!(error.code(), Code::CapabilityUnavailable);
    assert_eq!(error.context().admission(), Admission::NotAdmitted);
    assert!(
        ready
            .client
            .operations(&ready.scope)
            .get(OperationId::from_bytes([96; 32]))
            .await
            .is_err()
    );
}

#[tokio::test]
async fn ordinary_owned_budget_survives_lost_observers_and_all_accepted_commands_drain() {
    use std::{
        future::Future,
        task::{Context, Poll},
    };
    let ready = Ordinary::new().await;
    ready.start().await;
    let inbox = ready.client.inbox();
    let (paused, release) = ready.fixture.clock.arm(0);
    let mut first = Box::pin(inbox.inquire(
        ready.context(97, "first owned"),
        input("first owned command"),
    ));
    assert!(matches!(
        first
            .as_mut()
            .poll(&mut Context::from_waker(futures::task::noop_waker_ref())),
        Poll::Pending
    ));
    reached(paused).await;
    let mut second = Box::pin(inbox.inquire(
        ready.context(98, "second owned"),
        input("second owned command"),
    ));
    assert!(matches!(
        second
            .as_mut()
            .poll(&mut Context::from_waker(futures::task::noop_waker_ref())),
        Poll::Pending
    ));
    drop(first);
    drop(second);
    let copy_client = ready.client.clone();
    let error = copy_client
        .inbox()
        .inquire(ready.context(99, "over quota"), input("not accepted"))
        .await
        .unwrap_err();
    assert_eq!(error.code(), Code::CapabilityUnavailable);
    assert_eq!(
        error.context().operation_id(),
        Some(OperationId::from_bytes([99; 32]))
    );
    assert_eq!(error.context().admission(), Admission::NotAdmitted);
    let mut close = Box::pin(ready.client.close());
    assert!(matches!(
        close
            .as_mut()
            .poll(&mut Context::from_waker(futures::task::noop_waker_ref())),
        Poll::Pending
    ));
    release.send(()).unwrap();
    close.await.unwrap();
    let reopened = ready.fixture.reopen(ready.input.clone()).await;
    let binding = reopened
        .authenticate(ready.input.clone(), &ready.proof)
        .await
        .unwrap();
    for (id, body) in [(97, "first owned command"), (98, "second owned command")] {
        let snapshot = reopened
            .operation_snapshot(
                binding.clone(),
                OperationId::from_bytes([id; 32]),
                None,
                copy(&ready.proof),
            )
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            snapshot.prepared().request().chat_command().unwrap().text(),
            body
        );
    }
    assert!(
        reopened
            .operation_snapshot(
                binding,
                OperationId::from_bytes([99; 32]),
                None,
                copy(&ready.proof)
            )
            .await
            .unwrap()
            .is_none()
    );
    reopened.close().await.unwrap();
}

#[tokio::test]
async fn owned_inquiry_continues_to_its_coherent_query_after_the_caller_disappears() {
    use std::{
        future::Future,
        task::{Context, Poll},
    };
    let ready = Ordinary::new().await;
    ready.start().await;
    let inbox = ready.client.inbox();
    let (first_paused, release_first) = ready.fixture.clock.arm(0);
    let mut wait = Box::pin(inbox.inquire(
        ready.context(100, "lost before admission reply"),
        input("independent owner continuation"),
    ));
    assert!(matches!(
        wait.as_mut()
            .poll(&mut Context::from_waker(futures::task::noop_waker_ref())),
        Poll::Pending
    ));
    reached(first_paused).await;
    drop(wait);
    // After the first access observation, T0's fence consumes one clock read;
    // the coherent query's access is the next actual boundary.
    let (query_paused, release_query) = ready.fixture.clock.arm(1);
    release_first.send(()).unwrap();
    reached(query_paused).await;
    let mut close = Box::pin(ready.client.close());
    assert!(matches!(
        close
            .as_mut()
            .poll(&mut Context::from_waker(futures::task::noop_waker_ref())),
        Poll::Pending
    ));
    release_query.send(()).unwrap();
    close.await.unwrap();
    let reopened = ready.fixture.reopen(ready.input.clone()).await;
    let binding = reopened
        .authenticate(ready.input.clone(), &ready.proof)
        .await
        .unwrap();
    let snapshot = reopened
        .operation_snapshot(
            binding,
            OperationId::from_bytes([100; 32]),
            None,
            copy(&ready.proof),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        snapshot.prepared().request().chat_command().unwrap().text(),
        "independent owner continuation"
    );
    reopened.close().await.unwrap();
}
