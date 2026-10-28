//! Closed bounded records and exact references on the stock protected chain.
use super::*;
use serde::{Deserialize, Serialize};
use std::io::{Error, ErrorKind, Write};
use zeroize::Zeroizing;

pub(super) use super::super::super::delivery::json::{bounded, policy, small_vec, targets};

#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Reference {
    pub entry: OperationId,
    pub transaction: TransactionId,
    pub head: CheckpointDigest,
}
impl Reference {
    pub fn new(header: &ledger::Header) -> Self {
        Self {
            entry: header.operation,
            transaction: header.transaction,
            head: header.head,
        }
    }
    pub fn confirm(self, header: &ledger::Header) -> Result<(), OperationError> {
        if self.entry != header.operation
            || self.transaction != header.transaction
            || self.head != header.head
        {
            return Err(OperationError::RecoveryRequired);
        }
        Ok(())
    }
}
struct Writer {
    bytes: Zeroizing<Vec<u8>>,
    limit: usize,
}
impl Write for Writer {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if self
            .bytes
            .len()
            .checked_add(bytes.len())
            .is_none_or(|len| len > self.limit)
        {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                "stock delivery record limit",
            ));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
pub(super) fn encode<T: Serialize>(
    value: &T,
    limit: usize,
) -> Result<Zeroizing<Vec<u8>>, OperationError> {
    let mut writer = Writer {
        bytes: Zeroizing::new(Vec::with_capacity(limit.min(4096))),
        limit,
    };
    serde_json::to_writer(&mut writer, value).map_err(|_| OperationError::InvalidEvidence)?;
    Ok(writer.bytes)
}
pub(super) fn decode<T: for<'de> Deserialize<'de>>(
    raw: &[u8],
    limit: usize,
) -> Result<T, OperationError> {
    if raw.len() > limit || !bounded(raw, 32) {
        return Err(OperationError::RecoveryRequired);
    }
    serde_json::from_slice(raw).map_err(|_| OperationError::RecoveryRequired)
}
pub(super) fn head(header: &ledger::Header, body: &[u8]) -> CheckpointDigest {
    let mut hash = Sha256::new();
    hash.update(b"radroots.seller_stock.delivery_chain.v1\0");
    hash.update([header.family.code() as u8]);
    hash.update(header.namespace.workspace_id().as_bytes());
    hash.update(header.namespace.authority().executor_id().as_bytes());
    hash.update(
        header
            .namespace
            .authority()
            .generation()
            .get()
            .to_be_bytes(),
    );
    hash.update(header.namespace.route_digest().as_bytes());
    hash.update(header.actor.participant().as_bytes());
    hash.update(
        header
            .actor
            .grant_id()
            .map(|id| *id.as_bytes())
            .unwrap_or([0; 32]),
    );
    hash.update(header.operation.as_bytes());
    hash.update(header.digest.as_bytes());
    hash.update(header.transaction.as_bytes());
    hash.update(header.revision.get().to_be_bytes());
    hash.update([u8::from(header.previous.is_some())]);
    hash.update(
        header
            .previous
            .map(|value| *value.as_bytes())
            .unwrap_or([0; 32]),
    );
    hash.update(Sha256::digest(body));
    CheckpointDigest::from_bytes(hash.finalize().into())
}
