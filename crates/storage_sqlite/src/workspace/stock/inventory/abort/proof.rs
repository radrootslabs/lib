//! Protected original nonissuance and current-catalog replay absence.
use super::super::super::issuance::{claim, unissued};
use super::*;
use radroots_domain::PublicKey;
use serde::{Deserializer, Serializer};
use sha2::{Digest, Sha256};

#[derive(Clone, Copy, Serialize, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub(super) struct Record {
    operation: OperationId,
    revision: Sequence,
    head: CheckpointDigest,
    protected_digest: [u8; 32],
}
impl From<EntryIdentity> for Record {
    fn from(v: EntryIdentity) -> Self {
        Self {
            operation: v.operation,
            revision: v.revision,
            head: v.head,
            protected_digest: v.protected_digest,
        }
    }
}
fn claim_record(v: &claim::codec::Retained) -> Record {
    Record {
        operation: v.frame.header.operation,
        revision: v.frame.header.revision,
        head: v.frame.header.head,
        protected_digest: Sha256::digest(&v.frame.protected).into(),
    }
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum Recipient {
    Absent {
        recipient: PublicKey,
    },
    OriginalUnissued {
        recipient: PublicKey,
        claim: Box<Record>,
        unissued: Box<Record>,
    },
}
impl Recipient {
    fn recipient(&self) -> PublicKey {
        match self {
            Self::Absent { recipient } | Self::OriginalUnissued { recipient, .. } => *recipient,
        }
    }
}
/// Missing is distinct from a present null/empty field in retained payload1.
#[derive(Clone, Default)]
pub(super) enum Payload {
    #[default]
    Legacy,
    Current(Box<[Recipient; 2]>),
}
impl Payload {
    pub(super) fn legacy(&self) -> bool {
        matches!(self, Self::Legacy)
    }
}
impl Serialize for Payload {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Legacy => Err(serde::ser::Error::custom("missing cleanup proof")),
            Self::Current(v) => v.serialize(s),
        }
    }
}
impl<'de> Deserialize<'de> for Payload {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        <Box<[Recipient; 2]>>::deserialize(d).map(Self::Current)
    }
}
struct Known {
    claim: claim::codec::Retained,
    negative: unissued::Retained,
}
pub(super) struct Witness {
    pub(super) payload: Payload,
    operation: OperationId,
    actor: PublicKey,
    minimum: Vec<MinimumProof>,
    known: Vec<Known>,
}
async fn no_risks(
    connection: &mut SqliteConnection,
    actor: PublicKey,
    operation: OperationId,
) -> Result<(), OperationError> {
    let risk:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM stock_signing_evidence WHERE actor=? AND operation_id=?) OR EXISTS(SELECT 1 FROM stock_agreements WHERE actor=? AND operation_id=?) OR EXISTS(SELECT 1 FROM stock_replay_admissions WHERE actor=? AND operation_id=?) OR EXISTS(SELECT 1 FROM stock_operations WHERE actor=? AND related_operation=? AND family IN (5,6,9))")
        .bind(actor.as_bytes().as_slice()).bind(operation.as_bytes().as_slice())
        .bind(actor.as_bytes().as_slice()).bind(operation.as_bytes().as_slice())
        .bind(actor.as_bytes().as_slice()).bind(operation.as_bytes().as_slice())
        .bind(actor.as_bytes().as_slice()).bind(operation.as_bytes().as_slice())
        .fetch_one(connection).await.map_err(|_|OperationError::RecoveryRequired)?;
    if risk {
        return Err(OperationError::RecoveryRequired);
    }
    Ok(())
}
impl Witness {
    pub(super) async fn confirm(
        &self,
        connection: &mut SqliteConnection,
    ) -> Result<(), OperationError> {
        for proof in &self.minimum {
            Box::pin(proof.confirm(connection)).await?;
        }
        if self.payload.legacy() {
            return no_claims(connection, self.actor, self.operation).await;
        }
        no_risks(connection, self.actor, self.operation).await?;
        let claims: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM stock_signing_claims WHERE actor=? AND operation_id=?",
        )
        .bind(self.actor.as_bytes().as_slice())
        .bind(self.operation.as_bytes().as_slice())
        .fetch_one(&mut *connection)
        .await
        .map_err(|_| OperationError::RecoveryRequired)?;
        if claims != self.known.len() as i64 {
            return Err(OperationError::RecoveryRequired);
        }
        for value in &self.known {
            Box::pin(value.claim.confirm(connection)).await?;
            Box::pin(value.negative.confirm(connection)).await?;
        }
        Ok(())
    }
}
pub(super) async fn prepare(
    turn: &mut Turn<'_>,
    auth: &signing_plan::Authorized,
    operation: OperationId,
    at: UnixSeconds,
) -> Result<Witness, OperationError> {
    let mut minimum = Vec::with_capacity(2);
    let mut known = Vec::with_capacity(2);
    let mut encoded = Vec::with_capacity(2);
    for recipient in auth.original.receipt.rumor().recipients() {
        let (original, proof) =
            Box::pin(claim::existing(turn, auth, operation, *recipient)).await?;
        minimum.push(proof);
        match original {
            None => encoded.push(Recipient::Absent {
                recipient: *recipient,
            }),
            Some(original) => {
                if !matches!(auth.access.authority.catalog, 15..=17) {
                    return Err(OperationError::RecoveryRequired);
                }
                if at < original.report.claimed_at() {
                    return Err(WorkspaceError::Unavailable.into());
                }
                let negative = Box::pin(unissued::lookup(
                    turn.connection,
                    turn.selected,
                    turn.crypto,
                    operation,
                    *recipient,
                ))
                .await?
                .ok_or(OperationError::RecoveryRequired)?;
                if negative.receipt().signing_attempt_id() != original.report.attempt_id() {
                    return Err(OperationError::RecoveryRequired);
                }
                encoded.push(Recipient::OriginalUnissued {
                    recipient: *recipient,
                    claim: Box::new(claim_record(&original)),
                    unissued: Box::new(negative.identity().into()),
                });
                known.push(Known {
                    claim: original,
                    negative,
                });
            }
        }
    }
    let payload = if matches!(auth.access.authority.catalog, 15..=17) {
        Payload::Current(Box::new(
            encoded
                .try_into()
                .map_err(|_| OperationError::RecoveryRequired)?,
        ))
    } else {
        Payload::Legacy
    };
    let value = Witness {
        payload,
        operation,
        actor: turn.selected.participant(),
        minimum,
        known,
    };
    Box::pin(value.confirm(turn.connection)).await?;
    Ok(value)
}
pub(super) async fn read(
    connection: &mut SqliteConnection,
    input: &AttachmentInput,
    crypto: &WorkspaceCrypto,
    body: &Body,
    header: &ledger::Header,
    recipients: &[PublicKey; 2],
) -> Result<Witness, OperationError> {
    let operation = body.operation;
    let at = body.aborted_at;
    let revision = header.revision;
    let payload = body.proofs.clone();
    let mut known = Vec::with_capacity(2);
    if let Payload::Current(proofs) = &payload {
        let catalog: i64 = sqlx::query_scalar("PRAGMA user_version")
            .fetch_one(&mut *connection)
            .await
            .map_err(|_| OperationError::RecoveryRequired)?;
        if !matches!(catalog, 15..=17) {
            return Err(OperationError::RecoveryRequired);
        }
        for (proof, expected) in proofs.iter().zip(recipients) {
            if proof.recipient() != *expected {
                return Err(OperationError::RecoveryRequired);
            }
            if let Recipient::OriginalUnissued {
                claim: reference,
                unissued: negative_ref,
                ..
            } = proof
            {
                let original = Box::pin(claim::read_entry(
                    connection,
                    input,
                    crypto,
                    reference.operation,
                ))
                .await?
                .ok_or(OperationError::RecoveryRequired)?;
                let negative = Box::pin(unissued::lookup(
                    connection, input, crypto, operation, *expected,
                ))
                .await?
                .ok_or(OperationError::RecoveryRequired)?;
                if claim_record(&original) != **reference
                    || Record::from(negative.identity()) != **negative_ref
                    || original.report.attempt().operation_id() != operation
                    || original.report.recipient() != *expected
                    || original.report.claimed_at() > at
                    || negative.receipt().signing_attempt_id() != original.report.attempt_id()
                    || reference.revision >= revision
                    || negative_ref.revision >= revision
                {
                    return Err(OperationError::RecoveryRequired);
                }
                known.push(Known {
                    claim: original,
                    negative,
                });
            }
        }
    }
    let value = Witness {
        payload,
        operation,
        actor: input.participant(),
        minimum: vec![],
        known,
    };
    Box::pin(value.confirm(connection)).await?;
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proof_wire_requires_two_nonnull_entries_and_rejects_duplicate_fields() {
        let key = crate::workspace::tests::participant();
        let entry = serde_json::json!({"kind":"absent","recipient":key});
        let good = serde_json::to_string(&[entry.clone(), entry.clone()]).unwrap();
        assert!(serde_json::from_str::<Payload>(&good).is_ok());
        for bad in [
            "null".into(),
            "[]".into(),
            serde_json::to_string(std::slice::from_ref(&entry)).unwrap(),
            serde_json::to_string(&[entry.clone(), entry.clone(), entry]).unwrap(),
            good.replace(
                "\"kind\":\"absent\"",
                "\"kind\":\"absent\",\"kind\":\"absent\"",
            ),
            good.replace(
                "\"kind\":\"absent\"",
                "\"kind\":\"absent\",\"release\":true",
            ),
        ] {
            assert!(serde_json::from_str::<Payload>(&bad).is_err());
        }
    }
}
