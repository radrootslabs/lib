#![cfg(feature = "command_journal")]

use radroots_domain::{
    ActorRef, ActorRole, AuthorityRef, CheckpointDigest, CheckpointRef, CommandContextData,
    Coverage, ExecutorId, Generation, GrantId, OperationId, PublicKey, RouteDigest, Sequence,
    TransactionId, UnixSeconds, WorkspaceId,
};
use radroots_storage::{
    command::{AdmissionRequest, CommitAttempt, OperationError, PreparedOperation},
    workspace::{AttachmentInput, WorkspaceBinding, WorkspaceError},
};

fn binding() -> WorkspaceBinding {
    let key =
        PublicKey::from_hex("79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798")
            .unwrap();
    let grant = GrantId::from_bytes([3; 32]);
    let input = AttachmentInput::new(
        WorkspaceId::from_bytes([1; 32]),
        AuthorityRef::new(ExecutorId::from_bytes([2; 32]), Generation::new(1).unwrap()),
        key,
        "local://workspace/selector",
        key,
        grant,
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
    WorkspaceBinding::from_authenticated_host(
        input,
        radroots_storage::workspace::LocalOutboxScope::NewBuyerOutbox,
        ActorRef::new(key, ActorRole::Buyer, None, Some(grant)),
        "device",
        Sequence::new(1).unwrap(),
        UnixSeconds::new(200).unwrap(),
        UnixSeconds::new(100).unwrap(),
    )
    .unwrap()
}
fn request() -> AdmissionRequest {
    request_until(150)
}
fn request_until(not_after: u64) -> AdmissionRequest {
    let binding = binding();
    let input = binding.input();
    let context = CommandContextData::new(
        OperationId::from_bytes([6; 32]),
        binding.actor(),
        input.workspace_id(),
        input.authority(),
        input.route_digest(),
        None,
        None,
        UnixSeconds::new(not_after).unwrap(),
        None,
    )
    .unwrap();
    let command = radroots_protocol::command::CapturedChatCommand::inquire(
        context,
        radroots_protocol::command::InquireInput::new(
            radroots_domain::ThreadId::from_bytes([7; 32]),
            input.participant(),
            None,
            "private field",
        )
        .unwrap(),
    )
    .unwrap();
    AdmissionRequest::new(binding, command).unwrap()
}

#[test]
fn retained_window_cannot_precede_attachment_or_exceed_command_or_grant() {
    for (prepared, deadline, authorization) in [(99, 150, 150), (100, 151, 150), (100, 201, 300)] {
        let window = radroots_domain::IssuanceWindow::new(
            UnixSeconds::new(prepared).unwrap(),
            UnixSeconds::new(deadline).unwrap(),
        )
        .unwrap();
        assert!(matches!(
            PreparedOperation::from_committed_store(
                request_until(authorization),
                TransactionId::from_bytes([8; 32]),
                Sequence::new(1).unwrap(),
                window,
            ),
            Err(OperationError::RecoveryRequired)
        ));
    }
}

#[test]
fn structural_reports_preserve_full_original_anchor_without_minting_commit_proof() {
    let captured = request();
    let digest = captured.command().digest();
    let wire = captured.command().protected_wire().to_vec();
    let (binding, command) = captured.into_parts();
    assert_eq!(command.protected_wire(), wire);
    let parsed = AdmissionRequest::from_protected_wire(binding, &wire).unwrap();
    let transaction = TransactionId::from_bytes([8; 32]);
    assert!(matches!(
        PreparedOperation::from_committed_store(
            request(),
            transaction,
            Sequence::new(0).unwrap(),
            radroots_domain::IssuanceWindow::capture(
                UnixSeconds::new(100).unwrap(),
                UnixSeconds::new(150).unwrap()
            )
            .unwrap()
        ),
        Err(OperationError::RecoveryRequired)
    ));
    let reported = PreparedOperation::from_committed_store(
        parsed,
        transaction,
        Sequence::new(1).unwrap(),
        radroots_domain::IssuanceWindow::capture(
            UnixSeconds::new(100).unwrap(),
            UnixSeconds::new(150).unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    let anchor = reported.identity();
    assert_eq!(reported.transaction_id(), transaction);
    assert_eq!(reported.revision().get(), 1);
    assert_eq!(
        anchor.workspace_id(),
        reported.request().binding().input().workspace_id()
    );
    assert_eq!(anchor.operation_id(), OperationId::from_bytes([6; 32]));
    assert_eq!(anchor.command_digest(), digest);
    assert_eq!(anchor.transaction_id(), Some(transaction));
    assert_eq!(
        CommitAttempt::new(anchor.workspace_id(), anchor.operation_id(), digest, None)
            .transaction_id(),
        None
    );
    assert!(!format!("{reported:?}").contains("private field"));
}

#[test]
fn a_data_constructor_refuses_substituted_scope_but_never_authenticates_descriptors() {
    let original = request();
    let wire: serde_json::Value =
        serde_json::from_slice(original.command().protected_wire()).unwrap();
    for (pointer, value) in [
        ("/context/workspace_id", serde_json::json!("99".repeat(32))),
        ("/context/authority/generation", serde_json::json!("2")),
        ("/context/route_digest", serde_json::json!("99".repeat(32))),
        ("/context/actor/role", serde_json::json!("seller")),
    ] {
        let mut changed = wire.clone();
        *changed.pointer_mut(pointer).unwrap() = value;
        assert!(matches!(
            AdmissionRequest::from_protected_wire(
                binding(),
                &serde_json::to_vec(&changed).unwrap()
            ),
            Err(OperationError::InvalidCommand)
        ));
    }
    assert!(matches!(
        AdmissionRequest::from_protected_wire(binding(), b"{bad}"),
        Err(OperationError::RecoveryRequired)
    ));
    assert!(!format!("{original:?}").contains("private field"));
}

#[test]
fn operation_failures_are_fixed_redacted_and_distinguish_uncertainty() {
    let errors = [
        OperationError::from(WorkspaceError::Closed),
        OperationError::InvalidCommand,
        OperationError::Unauthorized,
        OperationError::MissingHistory,
        OperationError::UnverifiedApproval,
        OperationError::DeadlineExceeded,
        OperationError::IdempotencyConflict,
        OperationError::StorageFailure,
        OperationError::RecoveryRequired,
    ];
    let mut codes = std::collections::BTreeSet::new();
    for error in errors {
        assert!(codes.insert(error.code()));
        assert!(error.to_string().len() < 64);
        assert_eq!(error.to_string(), error.code());
        assert!(std::error::Error::source(&error).is_none());
    }
    let attempt = CommitAttempt::new(
        WorkspaceId::from_bytes([1; 32]),
        OperationId::from_bytes([2; 32]),
        radroots_domain::CommandDigest::from_bytes([3; 32]),
        None,
    );
    assert_eq!(
        format!(
            "{:?}",
            radroots_storage::command::AdmissionOutcome::Unknown(attempt)
        ),
        "AdmissionOutcome::Unknown(<retained>)"
    );
}
