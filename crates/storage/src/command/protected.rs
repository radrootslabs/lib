//! Bounded protected Prepared representation and pure scope checks shared by
//! concrete transaction hosts. Decoding supplies no current access or commit.
use super::{AdmissionRequest, OperationError, PreparedOperation};
use crate::workspace::{AttachmentInput, LocalOutboxScope, WorkspaceBinding};
use radroots_domain::{
    CommandDigest, IssuanceWindow, OperationId, Sequence, TransactionId, UnixSeconds,
};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

pub mod issuance;
pub mod plan;

/// Maximum complete preparation, before a host splits private records.
pub const PREPARED_BYTES_MAX: usize = 65_536;

#[derive(Serialize)]
struct Encoded<'a> {
    version: u8,
    selection: &'a AttachmentInput,
    scope: LocalOutboxScope,
    principal: &'a str,
    grant_revision: Sequence,
    grant_expires_at: UnixSeconds,
    captured_observed_at: UnixSeconds,
    transaction_id: TransactionId,
    revision: Sequence,
    prepared_at: UnixSeconds,
    first_issue_not_after: UnixSeconds,
    command: &'a serde_json::value::RawValue,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Decoded<'a> {
    version: u8,
    selection: AttachmentInput,
    #[serde(default, deserialize_with = "present_scope")]
    scope: Option<LocalOutboxScope>,
    principal: String,
    grant_revision: Sequence,
    grant_expires_at: UnixSeconds,
    captured_observed_at: UnixSeconds,
    transaction_id: TransactionId,
    revision: Sequence,
    prepared_at: UnixSeconds,
    first_issue_not_after: UnixSeconds,
    #[serde(borrow)]
    command: &'a serde_json::value::RawValue,
}

fn present_scope<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<LocalOutboxScope>, D::Error> {
    LocalOutboxScope::deserialize(deserializer).map(Some)
}

/// Encodes the original immutable preparation before encryption. A host must
/// independently commit it and enforce its smaller private record limit.
pub fn encode(
    request: &AdmissionRequest,
    transaction_id: TransactionId,
    window: IssuanceWindow,
) -> Result<Zeroizing<Vec<u8>>, OperationError> {
    let binding = request.binding();
    let encoded = Encoded {
        version: 3,
        selection: binding.input(),
        scope: binding.scope(),
        principal: binding.principal_id(),
        grant_revision: binding.grant_revision(),
        grant_expires_at: binding.expires_at(),
        captured_observed_at: binding.observed_at(),
        transaction_id,
        revision: Sequence::new(1).map_err(|_| OperationError::RecoveryRequired)?,
        prepared_at: window.prepared_at(),
        first_issue_not_after: window.first_issue_not_after(),
        command: serde_json::from_slice(request.command().protected_wire())
            .map_err(|_| OperationError::InvalidCommand)?,
    };
    let plaintext =
        Zeroizing::new(serde_json::to_vec(&encoded).map_err(|_| OperationError::InvalidCommand)?);
    if plaintext.len() > PREPARED_BYTES_MAX {
        return Err(OperationError::InvalidCommand);
    }
    Ok(plaintext)
}

/// Validates bounded decrypted retained bytes against the independently selected
/// namespace/operation. This is a historical observation, never current authority.
pub fn decode(
    bytes: &[u8],
    selected: &AttachmentInput,
    operation_id: OperationId,
) -> Result<PreparedOperation, OperationError> {
    if bytes.len() > PREPARED_BYTES_MAX {
        return Err(OperationError::RecoveryRequired);
    }
    let decoded: Decoded<'_> =
        serde_json::from_slice(bytes).map_err(|_| OperationError::RecoveryRequired)?;
    let scope = match (decoded.version, decoded.scope) {
        // The historical profile could only enroll buyer outboxes. It must
        // never infer seller authority from a substituted retained actor.
        (2, None) => LocalOutboxScope::NewBuyerOutbox,
        (3, Some(scope)) => scope,
        _ => return Err(OperationError::RecoveryRequired),
    };
    if decoded.revision.get() != 1 {
        return Err(OperationError::RecoveryRequired);
    }
    let command_context: radroots_domain::CommandContextData = {
        #[derive(Deserialize)]
        struct ContextOnly {
            context: radroots_domain::CommandContextData,
        }
        serde_json::from_str::<ContextOnly>(decoded.command.get())
            .map_err(|_| OperationError::RecoveryRequired)?
            .context
    };
    let binding = WorkspaceBinding::from_authenticated_host(
        decoded.selection,
        scope,
        command_context.actor(),
        decoded.principal,
        decoded.grant_revision,
        decoded.grant_expires_at,
        decoded.captured_observed_at,
    )
    .map_err(|_| OperationError::RecoveryRequired)?;
    if binding.input() != selected {
        return Err(OperationError::RecoveryRequired);
    }
    let request = AdmissionRequest::from_protected_wire(binding, decoded.command.get().as_bytes())
        .map_err(|_| OperationError::RecoveryRequired)?;
    let context = request.command().context();
    if context.workspace_id() != selected.workspace_id() || context.operation_id() != operation_id {
        return Err(OperationError::RecoveryRequired);
    }
    let snapshot = PreparedOperation::from_committed_store(
        request,
        decoded.transaction_id,
        decoded.revision,
        IssuanceWindow::new(decoded.prepared_at, decoded.first_issue_not_after)
            .map_err(|_| OperationError::RecoveryRequired)?,
    )?;
    Ok::<_, OperationError>(snapshot)
}

/// Checks the actor and optional exact original commitment without fresh issuance.
pub fn check_digest(
    snapshot: &PreparedOperation,
    actor: radroots_domain::ActorRef,
    expected: Option<CommandDigest>,
) -> Result<(), OperationError> {
    if snapshot.request().command().context().actor().participant() != actor.participant() {
        return Err(OperationError::Unauthorized);
    }
    if expected.is_some_and(|digest| digest != snapshot.request().command().digest()) {
        return Err(OperationError::IdempotencyConflict);
    }
    Ok(())
}

/// Compares the retained scope and current grant revision; observation times may differ.
pub fn same_binding(
    captured: &WorkspaceBinding,
    current: &WorkspaceBinding,
) -> Result<(), OperationError> {
    if captured.input() != current.input()
        || captured.scope() != current.scope()
        || captured.actor() != current.actor()
        || captured.principal_id() != current.principal_id()
        || captured.grant_revision() != current.grant_revision()
        || captured.expires_at() != current.expires_at()
    {
        return Err(OperationError::Unauthorized);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::{AttachmentProof, local_outbox::LocalOutboxConfig};
    use radroots_domain::{CommandContextData, ExecutorId, GrantId, PublicKey, WorkspaceId};
    use serde_json::json;

    pub(super) fn request() -> AdmissionRequest {
        let participant =
            PublicKey::from_hex("79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798")
                .unwrap();
        let now = UnixSeconds::new(100).unwrap();
        let (input, proof, grant) = LocalOutboxConfig::new(
            participant,
            participant,
            "synthetic-codec",
            UnixSeconds::new(200).unwrap(),
        )
        .unwrap()
        .enroll(
            now,
            WorkspaceId::from_bytes([1; 32]),
            ExecutorId::from_bytes([2; 32]),
            GrantId::from_bytes([3; 32]),
            AttachmentProof::from_protected_input(&[4; 32]).unwrap(),
        )
        .unwrap();
        let binding = grant.authenticate(&input, &proof, now).unwrap();
        let context = CommandContextData::new(
            OperationId::from_bytes([5; 32]),
            binding.actor(),
            input.workspace_id(),
            input.authority(),
            input.route_digest(),
            None,
            None,
            UnixSeconds::new(150).unwrap(),
            None,
        )
        .unwrap();
        let wire = serde_json::to_vec(&json!({"type":"inbox.inquire","context":context,
            "input":{"thread_id":"66".repeat(32),"peer":participant,"source":null,"text":"synthetic retained preparation"}})).unwrap();
        AdmissionRequest::from_protected_wire(binding, &wire).unwrap()
    }
    #[test]
    fn retained_scope_is_explicit_and_legacy_preparation_never_infers_seller_authority() {
        let request = request();
        let operation = request.command().context().operation_id();
        let bytes = encode(
            &request,
            TransactionId::from_bytes([7; 32]),
            IssuanceWindow::capture(
                UnixSeconds::new(100).unwrap(),
                UnixSeconds::new(150).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        let original: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(original["version"], 3);
        assert_eq!(original["scope"], "new_buyer_outbox");
        let read = |wire: &serde_json::Value| {
            decode(
                &serde_json::to_vec(wire).unwrap(),
                request.binding().input(),
                operation,
            )
        };
        let mut legacy = original.clone();
        legacy["version"] = json!(2);
        assert!(read(&legacy).is_err());
        legacy.as_object_mut().unwrap().remove("scope");
        assert_eq!(
            read(&legacy).unwrap().request().binding().scope(),
            LocalOutboxScope::NewBuyerOutbox
        );
        legacy["command"]["context"]["actor"]["role"] = json!("seller");
        assert!(read(&legacy).is_err());
        for version in [2, 3] {
            let mut null = original.clone();
            null["version"] = json!(version);
            null["scope"] = serde_json::Value::Null;
            assert!(read(&null).is_err());
        }
        let mut missing = original.clone();
        missing.as_object_mut().unwrap().remove("scope");
        assert!(read(&missing).is_err());
        let mut mismatched = original.clone();
        mismatched["scope"] = json!("new_seller_outbox");
        assert!(read(&mismatched).is_err());
        mismatched["command"]["context"]["actor"]["role"] = json!("seller");
        let decoded = read(&mismatched).unwrap();
        assert_eq!(
            decoded.request().binding().scope(),
            LocalOutboxScope::NewSellerOutbox
        );
        assert!(same_binding(request.binding(), decoded.request().binding()).is_err());
    }
    #[test]
    fn original_preparation_round_trip_and_scope_are_checked_without_current_authority() {
        let request = request();
        let operation = request.command().context().operation_id();
        let transaction = TransactionId::from_bytes([7; 32]);
        let window = IssuanceWindow::capture(
            UnixSeconds::new(100).unwrap(),
            UnixSeconds::new(150).unwrap(),
        )
        .unwrap();
        let bytes = encode(&request, transaction, window).unwrap();
        let decoded = decode(&bytes, request.binding().input(), operation).unwrap();
        assert_eq!(decoded.transaction_id(), transaction);
        assert_eq!(decoded.issuance_window(), window);
        assert_eq!(
            decoded.request().command().protected_wire(),
            request.command().protected_wire()
        );
        assert!(same_binding(request.binding(), decoded.request().binding()).is_ok());
        assert!(check_digest(&decoded, request.binding().actor(), None).is_ok());
        assert!(
            check_digest(
                &decoded,
                request.binding().actor(),
                Some(request.command().digest())
            )
            .is_ok()
        );
        assert_eq!(
            check_digest(
                &decoded,
                request.binding().actor(),
                Some(CommandDigest::from_bytes([8; 32]))
            ),
            Err(OperationError::IdempotencyConflict)
        );
        assert!(
            decode(
                &bytes,
                request.binding().input(),
                OperationId::from_bytes([9; 32])
            )
            .is_err()
        );
        assert!(
            decode(
                &vec![0; PREPARED_BYTES_MAX + 1],
                request.binding().input(),
                operation
            )
            .is_err()
        );
        assert!(decode(b"{}", request.binding().input(), operation).is_err());
        let original: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        for (pointer, value) in [
            ("/version", json!(1)),
            ("/revision", json!("0")),
            ("/revision", json!("2")),
            ("/selection/workspace_id", json!("88".repeat(32))),
            ("/principal", json!("")),
            ("/prepared_at", json!("0")),
            ("/prepared_at", json!("99")),
            ("/first_issue_not_after", json!("100")),
            ("/first_issue_not_after", json!("151")),
            ("/command/type", json!("unknown")),
            ("/command/context", json!({})),
        ] {
            let mut damaged = original.clone();
            *damaged.pointer_mut(pointer).unwrap() = value;
            let wire = serde_json::to_vec(&damaged).unwrap();
            assert!(
                decode(&wire, request.binding().input(), operation).is_err(),
                "{pointer}"
            );
        }
        let binding = request.binding();
        let other = WorkspaceBinding::from_authenticated_host(
            binding.input().clone(),
            binding.scope(),
            binding.actor(),
            "foreign-principal",
            binding.grant_revision(),
            binding.expires_at(),
            binding.observed_at(),
        )
        .unwrap();
        assert_eq!(
            same_binding(binding, &other),
            Err(OperationError::Unauthorized)
        );
        let foreign =
            PublicKey::from_hex("c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5")
                .unwrap();
        let actor = radroots_domain::ActorRef::new(
            foreign,
            binding.actor().role(),
            None,
            binding.actor().grant_id(),
        );
        assert_eq!(
            check_digest(&decoded, actor, None),
            Err(OperationError::Unauthorized)
        );
    }
}
