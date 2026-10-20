//! Validated historical T1 metadata shared by concrete protected hosts.
//! This codec establishes neither a commit nor current issuance authority.
use super::{OperationError, PreparedOperation};
use crate::command::{BoundSigningPlan, ChatSigningPlan};
use radroots_domain::{CommandDigest, OperationId, TransactionId, UnixSeconds, WorkspaceId};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

/// Maximum plaintext in each private plan record.
pub const PART_BYTES: usize = 16 * 1024;
/// Maximum parts in the complete existing known-kind plan profile.
pub const PARTS_MAX: usize = PLAN_BYTES_MAX / PART_BYTES;
/// Maximum complete retained plan representation.
pub const PLAN_BYTES_MAX: usize = radroots_protocol::chat::PLAN_MAX_BYTES;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Wire {
    version: u8,
    workspace: WorkspaceId,
    operation_id: OperationId,
    command_digest: CommandDigest,
    preparation_transaction: TransactionId,
    plan_digest: CommandDigest,
    transaction_id: TransactionId,
    bound_at: UnixSeconds,
    part_count: usize,
    byte_count: usize,
}

/// Opaque validated retained metadata; its Serde representation stays private.
pub struct PlanManifest(Wire);
impl PlanManifest {
    /// Captures structurally valid original T1 data before protection. The host
    /// must still authenticate current access and confirm its actual transaction.
    pub fn capture(
        prepared: &PreparedOperation,
        plan: &ChatSigningPlan,
        transaction: TransactionId,
        bound_at: UnixSeconds,
    ) -> Result<Self, OperationError> {
        let anchor = prepared.identity();
        if plan.operation_id() != anchor.operation_id()
            || plan.command_digest() != anchor.command_digest()
            || plan.created_at() < prepared.issuance_window().prepared_at()
            || plan.created_at() > bound_at
        {
            return Err(OperationError::InvalidCommand);
        }
        let result = Self(Wire {
            version: 1,
            workspace: anchor.workspace_id(),
            operation_id: anchor.operation_id(),
            command_digest: anchor.command_digest(),
            preparation_transaction: prepared.transaction_id(),
            plan_digest: CommandDigest::from_bytes(*plan.digest()),
            transaction_id: transaction,
            bound_at,
            part_count: plan.protected_wire().len().div_ceil(PART_BYTES),
            byte_count: plan.protected_wire().len(),
        });
        result.validate(prepared)?;
        Ok(result)
    }
    fn validate(&self, prepared: &PreparedOperation) -> Result<(), OperationError> {
        let w = &self.0;
        let anchor = prepared.identity();
        let window = prepared.issuance_window();
        if w.version != 1
            || w.workspace != anchor.workspace_id()
            || w.operation_id != anchor.operation_id()
            || w.command_digest != anchor.command_digest()
            || w.preparation_transaction != prepared.transaction_id()
            || !(1..=PARTS_MAX).contains(&w.part_count)
            || !(1..=PLAN_BYTES_MAX).contains(&w.byte_count)
            || w.byte_count.div_ceil(PART_BYTES) != w.part_count
            || w.bound_at < window.prepared_at()
            || w.bound_at >= window.first_issue_not_after()
        {
            return Err(OperationError::RecoveryRequired);
        }
        Ok(())
    }
    /// Encodes bounded metadata for explicit trusted-host protection.
    pub fn encode(&self) -> Result<Zeroizing<Vec<u8>>, OperationError> {
        let bytes = Zeroizing::new(
            serde_json::to_vec(&self.0).map_err(|_| OperationError::RecoveryRequired)?,
        );
        if bytes.len() > PART_BYTES {
            return Err(OperationError::RecoveryRequired);
        }
        Ok(bytes)
    }
    /// Checks bounded decrypted metadata against the independently retained T0.
    pub fn decode(bytes: &[u8], prepared: &PreparedOperation) -> Result<Self, OperationError> {
        if bytes.len() > PART_BYTES {
            return Err(OperationError::RecoveryRequired);
        }
        let result =
            Self(serde_json::from_slice(bytes).map_err(|_| OperationError::RecoveryRequired)?);
        result.validate(prepared)?;
        Ok(result)
    }
    pub const fn plan_digest(&self) -> CommandDigest {
        self.0.plan_digest
    }
    pub const fn transaction_id(&self) -> TransactionId {
        self.0.transaction_id
    }
    pub const fn bound_at(&self) -> UnixSeconds {
        self.0.bound_at
    }
    pub const fn part_count(&self) -> usize {
        self.0.part_count
    }
    pub const fn byte_count(&self) -> usize {
        self.0.byte_count
    }
    /// Checks complete original bytes before constructing a retained T1 report.
    /// This is historical data validation; concrete completion remains required.
    pub fn bind(
        self,
        prepared: PreparedOperation,
        plan: ChatSigningPlan,
    ) -> Result<BoundSigningPlan, OperationError> {
        self.validate(&prepared)?;
        if plan.digest() != self.plan_digest().as_bytes()
            || plan.protected_wire().len() != self.byte_count()
        {
            return Err(OperationError::RecoveryRequired);
        }
        BoundSigningPlan::from_committed_store(
            prepared,
            plan,
            self.transaction_id(),
            self.bound_at(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::protected::tests::request;
    use radroots_domain::{EnvelopeEventId, IssuanceWindow, Sequence};
    use radroots_protocol::chat::{CapturedDmRoute, ChatReplay, ChatRumor, ChatSeal};
    use serde_json::json;

    fn make_prepared() -> PreparedOperation {
        PreparedOperation::from_committed_store(
            request(),
            TransactionId::from_bytes([7; 32]),
            Sequence::new(1).unwrap(),
            IssuanceWindow::new(
                UnixSeconds::new(100).unwrap(),
                UnixSeconds::new(150).unwrap(),
            )
            .unwrap(),
        )
        .unwrap()
    }
    fn make_plan(prepared: &PreparedOperation, created: u64) -> ChatSigningPlan {
        // These are structural descriptors for pure codec tests. They supply
        // neither authenticated ciphertext nor a verified relay advertisement.
        let command = prepared.request().chat_command().unwrap();
        let now = UnixSeconds::new(created).unwrap();
        let rumor = ChatRumor::from_command(command, now).unwrap();
        let recipient = rumor.recipients()[0];
        let size = rumor.budget().rumor_ciphertext_bytes();
        let seal = (0..=2)
            .find_map(|pad| {
                let text = format!("Ag{}{}", "A".repeat(size - 2 - pad), "=".repeat(pad));
                ChatSeal::new(&rumor, recipient, now, &text).ok()
            })
            .unwrap();
        let route = CapturedDmRoute::from_observed_descriptor(
            recipient,
            EnvelopeEventId::from_bytes([8; 32]),
            r#"{"synthetic":"structural codec descriptor"}"#,
            &["wss://codec.example/".into()],
        )
        .unwrap();
        ChatSigningPlan::new(
            command,
            now,
            vec![seal],
            vec![route],
            ChatReplay::NonReplayable,
        )
        .unwrap()
    }
    #[test]
    fn manifest_binds_original_preparation_plan_and_window_without_minting_authority() {
        let prepared = make_prepared();
        let plan = make_plan(&prepared, 100);
        let transaction = TransactionId::from_bytes([9; 32]);
        let manifest = PlanManifest::capture(
            &prepared,
            &plan,
            transaction,
            UnixSeconds::new(100).unwrap(),
        )
        .unwrap();
        let bytes = manifest.encode().unwrap();
        let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(manifest.plan_digest().as_bytes(), plan.digest());
        assert_eq!(manifest.transaction_id(), transaction);
        assert_eq!(manifest.bound_at().get(), 100);
        assert_eq!(
            manifest.part_count(),
            plan.protected_wire().len().div_ceil(PART_BYTES)
        );
        assert_eq!(manifest.byte_count(), plan.protected_wire().len());
        for (pointer, replacement) in [
            ("/version", json!(2)),
            ("/workspace", json!("11".repeat(32))),
            ("/operation_id", json!("22".repeat(32))),
            ("/command_digest", json!("33".repeat(32))),
            ("/preparation_transaction", json!("44".repeat(32))),
            ("/part_count", json!(0)),
            ("/part_count", json!(25)),
            ("/part_count", json!(2)),
            ("/byte_count", json!(0)),
            ("/byte_count", json!(PLAN_BYTES_MAX + 1)),
            ("/bound_at", json!("99")),
            ("/bound_at", json!("150")),
        ] {
            let mut damaged = value.clone();
            *damaged.pointer_mut(pointer).unwrap() = replacement;
            assert!(
                PlanManifest::decode(&serde_json::to_vec(&damaged).unwrap(), &prepared).is_err(),
                "{pointer}"
            );
        }
        assert!(PlanManifest::decode(b"{}", &prepared).is_err());
        assert!(PlanManifest::decode(&vec![0; PART_BYTES + 1], &prepared).is_err());
        let mut foreign = value.clone();
        foreign["plan_digest"] = json!("55".repeat(32));
        let foreign =
            PlanManifest::decode(&serde_json::to_vec(&foreign).unwrap(), &prepared).unwrap();
        assert!(
            foreign
                .bind(make_prepared(), make_plan(&make_prepared(), 100))
                .is_err()
        );
        let decoded = PlanManifest::decode(&bytes, &prepared).unwrap();
        let bound = decoded.bind(prepared, plan).unwrap();
        assert_eq!(bound.transaction_id(), transaction);
        let late = make_prepared();
        let late_plan = make_plan(&late, 101);
        assert!(
            PlanManifest::capture(
                &late,
                &late_plan,
                transaction,
                UnixSeconds::new(151).unwrap()
            )
            .is_err()
        );
        assert!(
            PlanManifest::capture(
                &late,
                &late_plan,
                transaction,
                UnixSeconds::new(100).unwrap()
            )
            .is_err()
        );
        assert!(
            PlanManifest::capture(
                &late,
                &make_plan(&late, 99),
                transaction,
                UnixSeconds::new(100).unwrap()
            )
            .is_err()
        );
    }
}
