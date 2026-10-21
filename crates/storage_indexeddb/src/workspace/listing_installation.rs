//! Protected public installation metadata, validated against actual retained S.
use super::{
    Mutation, State, address, crypto,
    journal::{Access, OperationChange, Records, storage_error},
    retained::Graph,
};
use crate::ReadSnapshot;
use radroots_domain::{OperationId, TransactionId, UnixSeconds};
use radroots_secrets::context::{
    EnvelopeContext, EnvelopePurpose, EnvelopeSubject, PayloadSchemaId,
};
use radroots_storage::command::{
    BoundSigningPlan, CommitAttempt, EvidenceReceipt, InstallationOutcome, InstallationRequest,
    ListingInstallation, OperationError, OperationSnapshot, PublicPublicationWork, SigningTarget,
};
use sha2::{Digest, Sha256};

fn context(
    state: &State,
    bound: &BoundSigningPlan,
    work: bool,
) -> Result<EnvelopeContext, OperationError> {
    let subject = Sha256::new()
        .chain_update(b"radroots.browser.local_outbox.public_installation.context.v1\0")
        .chain_update(state.selected.workspace_id().as_bytes())
        .chain_update(bound.prepared().identity().operation_id().as_bytes())
        .chain_update(bound.plan().digest())
        .chain_update([u8::from(work)])
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    Ok(EnvelopeContext::new(
        EnvelopePurpose::parse("radroots.browser.workspace.public_installation")
            .map_err(|_| OperationError::InvalidCommand)?,
        EnvelopeSubject::parse("public_installation", subject)
            .map_err(|_| OperationError::InvalidCommand)?,
        PayloadSchemaId::parse(if work {
            "radroots.browser.workspace.public_work.v1"
        } else {
            "radroots.browser.workspace.listing_installation.v1"
        })
        .map_err(|_| OperationError::InvalidCommand)?,
    ))
}
fn evidence(hash: Sha256, value: EvidenceReceipt) -> Sha256 {
    hash.chain_update(value.workspace_id().as_bytes())
        .chain_update(value.operation_id().as_bytes())
        .chain_update(value.command_digest().as_bytes())
        .chain_update(value.attempt_id().as_bytes())
        .chain_update(value.event_id().as_bytes())
        .chain_update(value.digest())
        .chain_update(value.transaction_id().as_bytes())
        .chain_update([u8::from(value.observed_at_unix_ms().is_some())])
        .chain_update(
            value
                .observed_at_unix_ms()
                .map_or(0, |v| v.get())
                .to_be_bytes(),
        )
}
fn frame(
    magic: &[u8; 4],
    transaction: TransactionId,
    time: UnixSeconds,
    digest: [u8; 32],
) -> Vec<u8> {
    let mut output = Vec::with_capacity(76);
    output.extend_from_slice(magic);
    output.extend_from_slice(transaction.as_bytes());
    output.extend_from_slice(&time.get().to_be_bytes());
    output.extend_from_slice(&digest);
    output
}
fn selectors(raw: &[u8], magic: &[u8; 4]) -> Result<(TransactionId, UnixSeconds), OperationError> {
    if raw.len() != 76 || &raw[..4] != magic {
        return Err(OperationError::RecoveryRequired);
    }
    let transaction = TransactionId::from_bytes(
        raw[4..36]
            .try_into()
            .map_err(|_| OperationError::RecoveryRequired)?,
    );
    let time = UnixSeconds::new(u64::from_be_bytes(
        raw[36..44]
            .try_into()
            .map_err(|_| OperationError::RecoveryRequired)?,
    ))
    .map_err(|_| OperationError::RecoveryRequired)?;
    Ok((transaction, time))
}
fn installation(value: ListingInstallation) -> Vec<u8> {
    let hash = Sha256::new()
        .chain_update(b"radroots.browser.listing_installation.record.v1\0")
        .chain_update(value.plan_digest())
        .chain_update(value.author().as_bytes())
        .chain_update(value.listing_id().as_bytes())
        .chain_update(value.revision().as_bytes())
        .chain_update(value.preparation_transaction_id().as_bytes())
        .chain_update(value.plan_transaction_id().as_bytes());
    frame(
        b"RLI1",
        value.transaction_id(),
        value.installed_at(),
        evidence(hash, value.basis()).finalize().into(),
    )
}
fn publication(installed: ListingInstallation, value: PublicPublicationWork) -> Vec<u8> {
    let hash = Sha256::new()
        .chain_update(b"radroots.browser.public_work.record.v1\0")
        .chain_update(installation(installed))
        .chain_update(value.installation_transaction_id().as_bytes())
        .chain_update(value.plan_digest());
    frame(
        b"RPW1",
        value.transaction_id(),
        value.queued_at(),
        evidence(hash, value.evidence()).finalize().into(),
    )
}
pub(super) fn commitment(installed: ListingInstallation, work: PublicPublicationWork) -> [u8; 32] {
    Sha256::digest(publication(installed, work)).into()
}

impl State {
    pub(super) async fn read_public_installation(
        &self,
        view: &ReadSnapshot,
        graph: &Graph,
        operation: OperationId,
    ) -> Result<(Option<ListingInstallation>, Option<PublicPublicationWork>), OperationError> {
        let left = self
            .read_view_key(
                view,
                address::key(operation, address::LISTING_INSTALLATION, 0, 0),
            )
            .await?;
        let right = self
            .read_view_key(view, address::key(operation, address::PUBLIC_WORK, 0, 0))
            .await?;
        let (Some(left), Some(right)) = (left, right) else {
            if view.keys().any(|key| {
                matches!(
                    address::fields(key),
                    Ok((address::LISTING_INSTALLATION | address::PUBLIC_WORK, _, _))
                )
            }) {
                return Err(OperationError::RecoveryRequired);
            }
            return Ok((None, None));
        };
        let bound = graph.bound().ok_or(OperationError::RecoveryRequired)?;
        let basis = graph.receipt_target(SigningTarget::PublicEvent)?;
        let raw = crypto::open_bound(
            self.provider.as_ref(),
            &self.reference,
            context(self, bound, false)?,
            &left,
        )
        .await?;
        let installed = raw.expose_secret(|raw| {
            let (transaction, time) = selectors(raw, b"RLI1")?;
            let checked =
                ListingInstallation::from_committed_store(bound, basis, transaction, time)?;
            if raw != installation(checked) {
                return Err(OperationError::RecoveryRequired);
            }
            Ok(checked)
        })?;
        let raw = crypto::open_bound(
            self.provider.as_ref(),
            &self.reference,
            context(self, bound, true)?,
            &right,
        )
        .await?;
        let work = raw.expose_secret(|raw| {
            let (transaction, time) = selectors(raw, b"RPW1")?;
            let checked = PublicPublicationWork::from_committed_store(
                bound,
                installed,
                basis,
                transaction,
                time,
            )?;
            if raw != publication(installed, checked) {
                return Err(OperationError::RecoveryRequired);
            }
            Ok(checked)
        })?;
        Ok((Some(installed), Some(work)))
    }
    pub(super) async fn install_listing(
        &self,
        request: InstallationRequest,
        access: Access,
        view: ReadSnapshot,
        t0: Records,
        snapshot: OperationSnapshot,
    ) -> Result<InstallationOutcome, OperationError> {
        let bound = snapshot
            .bound_plan()
            .ok_or(OperationError::RecoveryRequired)?;
        let mut extra = Vec::new();
        let installed = if let Some(installed) = snapshot.listing_installation() {
            if snapshot.public_publications().len() != 1 {
                return Err(OperationError::RecoveryRequired);
            }
            installed
        } else {
            if !snapshot.public_publications().is_empty() {
                return Err(OperationError::RecoveryRequired);
            }
            let basis = snapshot
                .artifacts()
                .iter()
                .find(|v| v.target() == SigningTarget::PublicEvent)
                .and_then(|v| v.obtained())
                .ok_or(OperationError::InvalidEvidence)?;
            let transaction = TransactionId::from_bytes(*crypto::random::<32>()?);
            let installed = ListingInstallation::from_committed_store(
                bound,
                basis,
                transaction,
                access.binding.observed_at(),
            )?;
            let work = PublicPublicationWork::from_committed_store(
                bound,
                installed,
                basis,
                transaction,
                access.binding.observed_at(),
            )?;
            for (kind, is_work, bytes) in [
                (
                    address::LISTING_INSTALLATION,
                    false,
                    installation(installed),
                ),
                (address::PUBLIC_WORK, true, publication(installed, work)),
            ] {
                let protected = crypto::seal_bound(
                    self.provider.as_ref(),
                    &self.reference,
                    context(self, bound, is_work)?,
                    &bytes,
                )
                .await?;
                extra.push(
                    Mutation::new(
                        address::key(request.operation_id(), kind, 0, 0),
                        None,
                        Some(protected),
                    )
                    .map_err(storage_error)?,
                );
            }
            installed
        };
        let anchor = CommitAttempt::new(
            installed.workspace_id(),
            installed.operation_id(),
            installed.command_digest(),
            Some(installed.transaction_id()),
        );
        match self
            .check_operation(
                access,
                request.operation_id(),
                OperationChange {
                    before: t0,
                    after: None,
                    window: None,
                    delivery_deadline: None,
                    extra,
                },
                view,
            )
            .await
        {
            Ok(()) => Ok(InstallationOutcome::ListingCommitted(Box::new(installed))),
            Err(OperationError::Workspace(super::WorkspaceError::CommitUnknown)) => {
                self.mark_unknown();
                Ok(InstallationOutcome::Unknown(anchor))
            }
            Err(error) => Err(error),
        }
    }
}
