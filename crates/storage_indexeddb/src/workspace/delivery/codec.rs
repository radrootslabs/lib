//! Closed private v2 frame, manifest and independent request-index metadata.
use core::num::NonZeroU64;
use radroots_domain::{EnvelopeEventId, TransactionId, UnixSeconds};
use radroots_storage::command::{DeliveryAttempt, DeliveryIntent, OperationError, PublicationWork};
use radroots_transport::{TargetSet, policy::SatisfactionPolicy, target::TargetFingerprint};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

pub(super) const MAX_BYTES: usize = 393216;
pub(super) const PART_BYTES: usize = 16384;
pub(super) const PARTS: u8 = 24;
const _: () = assert!(MAX_BYTES == PART_BYTES * PARTS as usize);
fn bad<E>(_: E) -> OperationError {
    OperationError::RecoveryRequired
}
pub(super) fn valid_request(raw: &str) -> bool {
    raw.len() == 64
        && raw
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Header<'a> {
    version: u8,
    work: [u8; 32],
    #[serde(borrow)]
    request: &'a str,
    event: EnvelopeEventId,
    intent: [u8; 32],
    #[serde(deserialize_with = "super::json::targets")]
    targets: TargetSet,
    #[serde(deserialize_with = "super::json::small_vec")]
    selection: Vec<TargetFingerprint>,
    #[serde(deserialize_with = "super::json::policy")]
    policy: SatisfactionPolicy,
    deadline: u64,
    transaction: TransactionId,
    retained_at: UnixSeconds,
    #[serde(deserialize_with = "super::json::small_vec")]
    ordinals: Vec<u8>,
}
pub(super) fn encode(
    value: &DeliveryAttempt,
    work: [u8; 32],
) -> Result<Zeroizing<Vec<u8>>, OperationError> {
    for target in value.intent().targets().targets() {
        if serde_json::to_vec(target).map_err(bad)?.len() > PART_BYTES {
            return Err(OperationError::InvalidCommand);
        }
    }
    let header = Header {
        version: 2,
        work,
        request: value.intent().request_id().as_str(),
        event: value.intent().event_id(),
        intent: *value.intent().digest(),
        targets: value.intent().targets().clone(),
        selection: value.intent().selection().to_vec(),
        policy: value.intent().policy().clone(),
        deadline: value.intent().deadline_unix_ms().get(),
        transaction: value.transaction_id(),
        retained_at: value.retained_at(),
        ordinals: value.target_ordinals().to_vec(),
    };
    let header = Zeroizing::new(serde_json::to_vec(&header).map_err(bad)?);
    let length = 4usize
        .checked_add(header.len())
        .and_then(|n| n.checked_add(value.raw_json().len()))
        .filter(|n| *n <= MAX_BYTES)
        .ok_or(OperationError::InvalidCommand)?;
    let mut raw = Zeroizing::new(Vec::with_capacity(length));
    raw.extend_from_slice(&(header.len() as u32).to_be_bytes());
    raw.extend_from_slice(&header);
    raw.extend_from_slice(value.raw_json().as_bytes());
    Ok(raw)
}
pub(super) fn decode(
    raw: &[u8],
    work: PublicationWork,
    commitment: [u8; 32],
) -> Result<DeliveryAttempt, OperationError> {
    if raw.len() < 5 || raw.len() > MAX_BYTES {
        return Err(OperationError::RecoveryRequired);
    }
    let boundary = 4usize
        .checked_add(u32::from_be_bytes(raw[..4].try_into().map_err(bad)?) as usize)
        .filter(|n| *n < raw.len())
        .ok_or(OperationError::RecoveryRequired)?;
    if !super::json::bounded(&raw[4..boundary], 32) {
        return Err(OperationError::RecoveryRequired);
    }
    let header: Header<'_> = serde_json::from_slice(&raw[4..boundary]).map_err(bad)?;
    if header.version != 2 || header.work != commitment {
        return Err(OperationError::RecoveryRequired);
    }
    let intent = DeliveryIntent::new(
        header.request,
        header.event,
        &header.targets,
        &header.policy,
        NonZeroU64::new(header.deadline).ok_or(OperationError::RecoveryRequired)?,
    )
    .map_err(bad)?;
    let selected = TargetSet::new(
        intent
            .targets()
            .targets()
            .iter()
            .filter(|target| header.selection.contains(target.fingerprint()))
            .cloned()
            .collect(),
    )
    .map_err(bad)?;
    if selected
        .targets()
        .iter()
        .map(|target| target.fingerprint())
        .ne(header.selection.iter())
    {
        return Err(OperationError::RecoveryRequired);
    }
    let intent = intent.with_selection(&selected).map_err(bad)?;
    if intent.digest() != &header.intent {
        return Err(OperationError::RecoveryRequired);
    }
    DeliveryAttempt::from_store_report(
        work,
        intent,
        core::str::from_utf8(&raw[boundary..]).map_err(bad)?,
        header.transaction,
        header.retained_at,
        header.ordinals,
    )
    .map_err(bad)
}
pub(super) struct Manifest {
    pub request: String,
    pub event: EnvelopeEventId,
    pub transaction: TransactionId,
    pub bytes: usize,
    pub digest: [u8; 32],
}
impl Manifest {
    pub(super) fn encode(
        value: &DeliveryAttempt,
        commitment: [u8; 32],
        frame: &[u8],
    ) -> Zeroizing<Vec<u8>> {
        let mut raw = Zeroizing::new(Vec::with_capacity(200));
        raw.extend_from_slice(b"RDD2");
        raw.extend_from_slice(&commitment);
        raw.extend_from_slice(value.intent().request_id().as_str().as_bytes());
        raw.extend_from_slice(value.intent().event_id().as_bytes());
        raw.extend_from_slice(value.transaction_id().as_bytes());
        raw.extend_from_slice(&(frame.len() as u32).to_be_bytes());
        raw.extend_from_slice(&Sha256::digest(frame));
        raw
    }
    pub(super) fn decode(raw: &[u8], commitment: [u8; 32]) -> Result<Self, OperationError> {
        if raw.len() != 200 || &raw[..4] != b"RDD2" || raw[4..36] != commitment {
            return Err(OperationError::RecoveryRequired);
        }
        let request = core::str::from_utf8(&raw[36..100]).map_err(bad)?;
        let bytes = u32::from_be_bytes(raw[164..168].try_into().map_err(bad)?) as usize;
        if !valid_request(request) || !(5..=MAX_BYTES).contains(&bytes) {
            return Err(OperationError::RecoveryRequired);
        }
        Ok(Self {
            request: request.into(),
            event: EnvelopeEventId::from_bytes(raw[100..132].try_into().map_err(bad)?),
            transaction: TransactionId::from_bytes(raw[132..164].try_into().map_err(bad)?),
            bytes,
            digest: raw[168..200].try_into().map_err(bad)?,
        })
    }
    pub(super) fn matches(&self, value: &DeliveryAttempt) -> bool {
        self.request == value.intent().request_id().as_str()
            && self.event == value.intent().event_id()
            && self.transaction == value.transaction_id()
    }
}
pub(super) fn index(manifest: &[u8], request: &str) -> Zeroizing<Vec<u8>> {
    let mut raw = Zeroizing::new(Vec::with_capacity(100));
    raw.extend_from_slice(b"RDI2");
    raw.extend_from_slice(&Sha256::digest(manifest));
    raw.extend_from_slice(request.as_bytes());
    raw
}
