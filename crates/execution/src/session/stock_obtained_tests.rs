//! Conversion refusal must retain actual verified original bytes without IO.
use super::*;
use crate::session::commerce::{AcceptanceClaim, AcceptanceSelection};
use alloc::vec;
use futures::executor::block_on;
use radroots_domain::{
    ActorRef, ActorRole, AuthorityRef, CheckpointDigest, CheckpointRef, CommandDigest, Coverage,
    ExecutorId, Generation, GrantId, OperationId, PoolId, RouteDigest, Sequence, SigningAttemptId,
    UnixSeconds, WorkspaceId,
    stock::{OptionMappingKey, OptionPoolMapping, PoolView},
};
use radroots_protocol::{
    command::CapturedOrderAcceptance,
    commerce::{self, CommerceSigningPlan, EncodedCommerceRumor},
};
use radroots_signing::{
    issuance::{OutcomeKind, SigningIntent, SigningOutcome},
    provider::ProviderCall,
    request::CancellationSignal,
};
use radroots_storage::{
    event::BoxFuture,
    inventory::{InventoryCheckpointRef, InventoryOutcome, InventoryReceipt, InventoryRequest},
    workspace::{
        AttachmentInput, WorkspaceBinding, WorkspaceError, WorkspaceScope, WorkspaceStore,
    },
};
use std::num::NonZeroU64;
struct NoIo;
impl WorkspaceStore for NoIo {
    fn authenticate(
        &self,
        _: AttachmentInput,
        _: AttachmentProof,
    ) -> BoxFuture<'_, Result<WorkspaceBinding, WorkspaceError>> {
        panic!("mapping refusal must execute no host IO")
    }
    fn close_workspace(&self) -> BoxFuture<'_, Result<(), WorkspaceError>> {
        panic!("mapping refusal must execute no host IO")
    }
}
impl InventoryStore for NoIo {
    fn option_mapping(
        &self,
        _: WorkspaceBinding,
        _: InventoryCheckpointRef,
        _: OptionMappingKey,
        _: AttachmentProof,
    ) -> BoxFuture<'_, Result<Option<OptionPoolMapping>, OperationError>> {
        panic!("unexpected IO")
    }
    fn execute_inventory(
        &self,
        _: InventoryRequest,
        _: AttachmentProof,
    ) -> BoxFuture<'_, Result<InventoryOutcome, OperationError>> {
        panic!("unexpected IO")
    }
    fn inventory_operation(
        &self,
        _: WorkspaceBinding,
        _: InventoryCheckpointRef,
        _: OperationId,
        _: Option<CommandDigest>,
        _: AttachmentProof,
    ) -> BoxFuture<'_, Result<Option<InventoryReceipt>, OperationError>> {
        panic!("unexpected IO")
    }
    fn pool(
        &self,
        _: WorkspaceBinding,
        _: InventoryCheckpointRef,
        _: PoolId,
        _: AttachmentProof,
    ) -> BoxFuture<'_, Result<Option<PoolView>, OperationError>> {
        panic!("unexpected IO")
    }
}
#[test]
fn mapping_refusal_keeps_verified_artifact_credentials_and_lease_before_any_storage_request() {
    let command = CapturedOrderAcceptance::parse(include_bytes!(
        "../../tests/fixtures/commerce_verification/command_wire.json"
    ))
    .unwrap();
    let record = commerce::decode(include_bytes!(
        "../../tests/fixtures/commerce_verification/record_wire.json"
    ))
    .unwrap();
    let rumor = EncodedCommerceRumor::capture(record.content()).unwrap();
    let plan = CommerceSigningPlan::parse(
        command.context().operation_id(),
        command.digest(),
        &rumor,
        include_bytes!("../../tests/fixtures/commerce_verification/plan_wire.bin"),
    )
    .unwrap();
    let seal = &plan.seals()[0];
    let intent = SigningIntent::from_commerce_seal(
        seal,
        ProviderCall::new(
            rumor.author(),
            NonZeroU64::new(600000).unwrap(),
            CancellationSignal::new(),
        ),
    )
    .unwrap();
    let wire = include_str!("../../tests/fixtures/commerce_verification/seal_signed.json");
    let (_, OutcomeKind::Obtained(artifact)) =
        SigningOutcome::capture(&intent, wire.into(), None).into_parts()
    else {
        panic!("fixture must verify as the exact original signed artifact")
    };
    // A deliberately wrong non-stock descriptor forces conversion to fail.
    // Neither the fake host nor this descriptor has transaction authority.
    let authority = AuthorityRef::new(ExecutorId::from_bytes([2; 32]), Generation::new(1).unwrap());
    let input = AttachmentInput::new(
        WorkspaceId::from_bytes([1; 32]),
        authority,
        rumor.author(),
        "local://model",
        rumor.author(),
        GrantId::from_bytes([3; 32]),
        CheckpointRef::new(
            CheckpointDigest::from_bytes([4; 32]),
            Sequence::new(0).unwrap(),
            Coverage::Partial,
            vec![],
        )
        .unwrap(),
        RouteDigest::from_bytes([5; 32]),
    )
    .unwrap();
    let binding = WorkspaceBinding::from_authenticated_host(
        input.clone(),
        WorkspaceScope::NewSellerOutbox,
        ActorRef::new(
            rumor.author(),
            ActorRole::Seller,
            None,
            Some(input.grant_id()),
        ),
        "device",
        Sequence::new(1).unwrap(),
        UnixSeconds::new(200).unwrap(),
        UnixSeconds::new(100).unwrap(),
    )
    .unwrap();
    let lease = Shared::new(());
    let weak = Shared::downgrade(&lease);
    let context = ObtainedContext {
        selection: AcceptanceSelection {
            binding,
            minimum: InventoryCheckpointRef::from_host_metadata(
                authority,
                Sequence::new(0).unwrap(),
                None,
            )
            .unwrap(),
            operation: command.context().operation_id(),
            digest: command.digest(),
        },
        claim: AcceptanceClaim {
            id: SigningAttemptId::from_bytes([6; 32]),
            binding: intent.binding(),
        },
        recipient: seal.recipient(),
        store: Shared::new(NoIo),
        proof: AttachmentProof::from_protected_input(&[7; 32]).unwrap(),
        lease: lease.clone(),
    };
    let owner = Owner::default();
    let entry = owner.retain_obtained(context, artifact);
    drop(lease);
    assert_eq!(
        block_on(owner.capture_obtained(&entry)).unwrap_err().code(),
        crate::Code::Unauthorized
    );
    drop(entry);
    assert!(weak.upgrade().is_some());
    assert!(owner.ensure_ready().is_err());
    assert_eq!(
        block_on(owner.drain()).unwrap_err().code(),
        crate::Code::Unauthorized
    );
    let retained = owner.unmapped.lock().unwrap();
    assert_eq!(retained.len(), 1);
    assert_eq!(retained[0].context.proof.with_secret(|b| *b), [7; 32]);
    retained[0].artifact.with_json(|raw| assert_eq!(raw, wire));
    assert!(owner.pending.lock().unwrap().is_empty());
}
