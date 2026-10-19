//! Closed v2 private record positions under one opaque operation prefix.
use super::{RecordKey, State};
use crate::{ReadSnapshot, RecordPrefix};
use radroots_domain::OperationId;
use radroots_storage::command::OperationError;
use sha2::{Digest, Sha256};

pub(super) const PREPARED: u8 = 1;
pub(super) const PLAN: u8 = 2;
pub(super) const CLAIM: u8 = 3;
pub(super) const EVIDENCE: u8 = 4;

fn prefix_bytes(operation: OperationId) -> [u8; 16] {
    let digest = Sha256::new()
        .chain_update(b"radroots.browser.local_outbox.operation.prefix.v2\0")
        .chain_update(operation.as_bytes())
        .finalize();
    let mut bytes = [0; 16];
    bytes.copy_from_slice(&digest[..16]);
    bytes
}
pub(super) fn prefix(operation: OperationId) -> RecordPrefix {
    RecordPrefix::new(prefix_bytes(operation))
}
pub(super) fn key(operation: OperationId, kind: u8, recipient: u8, part: u8) -> RecordKey {
    let mut bytes = [0; 32];
    bytes[..16].copy_from_slice(&prefix_bytes(operation));
    bytes[16] = kind;
    bytes[17] = recipient;
    bytes[20] = part;
    RecordKey::new(bytes)
}
pub(super) fn fields(key: RecordKey) -> Result<(u8, u8, u8), OperationError> {
    let bytes = key.as_bytes();
    let kind = bytes[16];
    let recipient = bytes[17];
    let part = bytes[20];
    if bytes[18..20]
        .iter()
        .chain(bytes[21..].iter())
        .any(|b| *b != 0)
        || match kind {
            PREPARED => recipient != 0 || part > 4,
            PLAN => recipient != 0 || part > 24,
            CLAIM => recipient > 1 || part != 0,
            EVIDENCE => recipient > 1 || part > 4,
            // Later reserved families remain unsupported until their actual
            // implementations and proving cohort land. Never ignore them.
            _ => true,
        }
    {
        return Err(OperationError::RecoveryRequired);
    }
    Ok((kind, recipient, part))
}
impl State {
    pub(super) async fn read_view(
        &self,
        operation: OperationId,
    ) -> Result<ReadSnapshot, OperationError> {
        let view = self
            .db
            .read_snapshot(prefix(operation))
            .await
            .map_err(super::journal::storage_error)?;
        for key in view.keys() {
            fields(key)?;
        }
        Ok(view)
    }
    pub(super) async fn read_view_key(
        &self,
        view: &ReadSnapshot,
        key: RecordKey,
    ) -> Result<Option<Vec<u8>>, OperationError> {
        if !view.contains(key) {
            return Ok(None);
        }
        let value = self
            .db
            .get(key)
            .await
            .map_err(super::journal::storage_error)?;
        view.verify(key, value.as_deref())
            .map_err(super::journal::storage_error)?;
        Ok(value)
    }
}
