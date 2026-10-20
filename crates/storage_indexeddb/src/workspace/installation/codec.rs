//! Closed host-private v2 metadata. Bodies stay in the original command.
use super::super::retained::Graph;
use radroots_domain::{PublicKey, TransactionId, UnixSeconds};
use radroots_storage::command::{
    BoundSigningPlan, ChatInstallation, EvidenceReceipt, OperationError, PublicationWork,
};
use zeroize::Zeroizing;

const LENGTH: usize = 4 + 6 * 32 + 8 + 8 * 32 + 8;
const TRANSACTION: usize = 4 + 5 * 32;
const TIME: usize = 4 + 6 * 32;
const RECIPIENT: usize = TIME + 8 + 3 * 32;
const MESSAGE: &[u8; 4] = b"RRM2";
const WORK: &[u8; 4] = b"RRW2";
const _: () = assert!(LENGTH <= 16 * 1024);

fn encode(
    magic: &[u8; 4],
    fields: [&[u8; 32]; 6],
    time: UnixSeconds,
    receipt: EvidenceReceipt,
    recipient: PublicKey,
) -> Zeroizing<Vec<u8>> {
    let mut raw = Zeroizing::new(Vec::with_capacity(LENGTH));
    raw.extend_from_slice(magic);
    for field in fields {
        raw.extend_from_slice(field);
    }
    raw.extend_from_slice(&time.get().to_be_bytes());
    for field in [
        receipt.workspace_id().as_bytes(),
        receipt.operation_id().as_bytes(),
        receipt.command_digest().as_bytes(),
        recipient.as_bytes(),
        receipt.attempt_id().as_bytes(),
        receipt.event_id().as_bytes(),
        receipt.digest(),
        receipt.transaction_id().as_bytes(),
    ] {
        raw.extend_from_slice(field);
    }
    raw.extend_from_slice(
        &receipt
            .observed_at_unix_ms()
            .map_or(0, |time| time.get())
            .to_be_bytes(),
    );
    raw
}
fn selectors(
    raw: &[u8],
    magic: &[u8; 4],
) -> Result<(TransactionId, UnixSeconds, PublicKey), OperationError> {
    if raw.len() != LENGTH || &raw[..4] != magic {
        return Err(OperationError::RecoveryRequired);
    }
    let transaction = TransactionId::from_bytes(
        raw[TRANSACTION..TRANSACTION + 32]
            .try_into()
            .map_err(|_| OperationError::RecoveryRequired)?,
    );
    let time = UnixSeconds::new(u64::from_be_bytes(
        raw[TIME..TIME + 8]
            .try_into()
            .map_err(|_| OperationError::RecoveryRequired)?,
    ))
    .map_err(|_| OperationError::RecoveryRequired)?;
    let recipient = PublicKey::from_bytes(
        raw[RECIPIENT..RECIPIENT + 32]
            .try_into()
            .map_err(|_| OperationError::RecoveryRequired)?,
    )
    .map_err(|_| OperationError::RecoveryRequired)?;
    Ok((transaction, time, recipient))
}
pub(super) fn message(
    bound: &BoundSigningPlan,
    value: ChatInstallation,
) -> Result<Zeroizing<Vec<u8>>, OperationError> {
    Ok(encode(
        MESSAGE,
        [
            value.plan_digest(),
            value.message_id().as_bytes(),
            bound
                .prepared()
                .request()
                .chat_command()?
                .thread_id()
                .as_bytes(),
            value.preparation_transaction_id().as_bytes(),
            value.plan_transaction_id().as_bytes(),
            value.transaction_id().as_bytes(),
        ],
        value.installed_at(),
        value.basis(),
        value.basis().recipient()?,
    ))
}
pub(super) fn decode_message(
    raw: &[u8],
    graph: &Graph,
) -> Result<ChatInstallation, OperationError> {
    let (transaction, time, recipient) = selectors(raw, MESSAGE)?;
    let bound = graph.bound().ok_or(OperationError::RecoveryRequired)?;
    let report = ChatInstallation::from_committed_store(
        bound,
        graph.receipt(recipient)?,
        transaction,
        time,
    )?;
    if message(bound, report)?.as_slice() != raw {
        return Err(OperationError::RecoveryRequired);
    }
    Ok(report)
}
pub(super) fn work(installed: ChatInstallation, value: PublicationWork) -> Zeroizing<Vec<u8>> {
    encode(
        WORK,
        [
            value.plan_digest(),
            installed.message_id().as_bytes(),
            value.installation_transaction_id().as_bytes(),
            installed.preparation_transaction_id().as_bytes(),
            installed.plan_transaction_id().as_bytes(),
            value.transaction_id().as_bytes(),
        ],
        value.queued_at(),
        value.evidence(),
        value.recipient(),
    )
}
pub(super) fn decode_work(
    raw: &[u8],
    graph: &Graph,
    installed: ChatInstallation,
    expected: PublicKey,
) -> Result<PublicationWork, OperationError> {
    let (transaction, time, recipient) = selectors(raw, WORK)?;
    if recipient != expected {
        return Err(OperationError::RecoveryRequired);
    }
    let bound = graph.bound().ok_or(OperationError::RecoveryRequired)?;
    let report = PublicationWork::from_committed_store(
        bound,
        installed,
        graph.receipt(recipient)?,
        transaction,
        time,
    )?;
    if work(installed, report).as_slice() != raw {
        return Err(OperationError::RecoveryRequired);
    }
    Ok(report)
}
