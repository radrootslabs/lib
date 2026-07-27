use crate::{
    RADROOTS_TRANSPORT_DIAGNOSTIC_MAX_BYTES, RADROOTS_TRANSPORT_FETCH_ADMITTED_EVENT_MAX_COUNT,
    RADROOTS_TRANSPORT_IDENTIFIER_MAX_BYTES, RADROOTS_TRANSPORT_TARGET_MAX_COUNT,
    RadrootsTransportDeliveryReceipt, RadrootsTransportDeliveryRequest, RadrootsTransportError,
    RadrootsTransportKind, RadrootsTransportStatus, RadrootsTransportTargetReceipt,
    RadrootsTransportTargetSet,
};
use alloc::boxed::Box;
use alloc::collections::{BTreeMap, BTreeSet};
use alloc::string::String;
use alloc::vec::Vec;
use core::future::Future;
use core::pin::Pin;

pub type RadrootsTransportFuture<'a, T> =
    Pin<Box<dyn Future<Output = Result<T, RadrootsTransportError>> + Send + 'a>>;

pub const RADROOTS_TRANSPORT_FETCH_REQUEST_ID_MAX_BYTES: usize =
    RADROOTS_TRANSPORT_IDENTIFIER_MAX_BYTES;

pub trait RadrootsTransport: Send + Sync {
    fn transport_kind(&self) -> RadrootsTransportKind;

    fn status<'a>(&'a self) -> RadrootsTransportFuture<'a, RadrootsTransportStatus>;

    fn deliver<'a>(
        &'a self,
        request: RadrootsTransportDeliveryRequest,
    ) -> RadrootsTransportFuture<'a, RadrootsTransportDeliveryReceipt>;

    fn fetch<'a>(
        &'a self,
        request: RadrootsTransportFetchRequest,
    ) -> RadrootsTransportFuture<'a, RadrootsTransportFetchReceipt>;
}

#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsTransportFetchRequest {
    request_id: String,
    target_set: RadrootsTransportTargetSet,
}

impl RadrootsTransportFetchRequest {
    pub fn new(
        request_id: impl Into<String>,
        target_set: RadrootsTransportTargetSet,
    ) -> Result<Self, RadrootsTransportError> {
        let request_id = request_id.into();
        validate_fetch_request_id(request_id.as_str())?;
        Ok(Self {
            request_id,
            target_set,
        })
    }

    pub fn request_id(&self) -> &str {
        self.request_id.as_str()
    }

    pub fn target_set(&self) -> &RadrootsTransportTargetSet {
        &self.target_set
    }
}

fn validate_fetch_request_id(value: &str) -> Result<(), RadrootsTransportError> {
    if value.is_empty() {
        return Err(RadrootsTransportError::EmptyFetchRequestId);
    }
    crate::limits::ensure_resource_limit(
        "fetch_request_id",
        value.len(),
        RADROOTS_TRANSPORT_FETCH_REQUEST_ID_MAX_BYTES,
    )?;
    if value != value.trim() || value.chars().any(char::is_control) {
        return Err(RadrootsTransportError::InvalidFetchRequestId);
    }
    Ok(())
}

#[cfg(feature = "serde")]
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RadrootsTransportFetchRequestWire {
    #[serde(deserialize_with = "deserialize_fetch_request_id")]
    request_id: String,
    target_set: RadrootsTransportTargetSet,
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for RadrootsTransportFetchRequest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let wire = RadrootsTransportFetchRequestWire::deserialize(deserializer)?;
        Self::new(wire.request_id, wire.target_set).map_err(serde::de::Error::custom)
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsTransportFetchReceipt {
    request_id: String,
    target_set: RadrootsTransportTargetSet,
    target_receipts: Vec<RadrootsTransportTargetReceipt>,
    fetched_count: usize,
}

impl RadrootsTransportFetchReceipt {
    pub fn for_request(
        request: &RadrootsTransportFetchRequest,
        target_receipts: Vec<RadrootsTransportTargetReceipt>,
        fetched_count: usize,
    ) -> Result<Self, RadrootsTransportError> {
        Self::new(
            request.request_id(),
            request.target_set().clone(),
            target_receipts,
            fetched_count,
        )
    }

    pub fn new(
        request_id: impl Into<String>,
        target_set: RadrootsTransportTargetSet,
        target_receipts: Vec<RadrootsTransportTargetReceipt>,
        fetched_count: usize,
    ) -> Result<Self, RadrootsTransportError> {
        let request_id = request_id.into();
        validate_fetch_request_id(request_id.as_str())?;
        crate::limits::ensure_resource_limit(
            "fetch_admitted_event_count",
            fetched_count,
            RADROOTS_TRANSPORT_FETCH_ADMITTED_EVENT_MAX_COUNT,
        )?;
        let target_receipts = canonicalize_fetch_target_receipts(&target_set, target_receipts)?;
        Ok(Self {
            request_id,
            target_set,
            target_receipts,
            fetched_count,
        })
    }

    pub fn request_id(&self) -> &str {
        self.request_id.as_str()
    }

    pub fn target_set(&self) -> &RadrootsTransportTargetSet {
        &self.target_set
    }

    pub fn target_receipts(&self) -> &[RadrootsTransportTargetReceipt] {
        &self.target_receipts
    }

    pub fn fetched_count(&self) -> usize {
        self.fetched_count
    }

    pub fn validate_for_request(
        &self,
        request: &RadrootsTransportFetchRequest,
    ) -> Result<(), RadrootsTransportError> {
        if self.request_id() != request.request_id() {
            return Err(RadrootsTransportError::FetchReceiptRequestIdMismatch);
        }
        if self.target_set() != request.target_set() {
            return Err(RadrootsTransportError::FetchReceiptTargetSetMismatch);
        }
        Ok(())
    }
}

fn canonicalize_fetch_target_receipts(
    target_set: &RadrootsTransportTargetSet,
    target_receipts: Vec<RadrootsTransportTargetReceipt>,
) -> Result<Vec<RadrootsTransportTargetReceipt>, RadrootsTransportError> {
    crate::limits::ensure_resource_limit(
        "fetch_target_receipt_count",
        target_receipts.len(),
        RADROOTS_TRANSPORT_TARGET_MAX_COUNT,
    )?;
    let mut receipt_fingerprints = BTreeSet::new();
    let mut diagnostic_bytes = 0usize;
    for receipt in &target_receipts {
        receipt.validate()?;
        diagnostic_bytes = diagnostic_bytes
            .checked_add(receipt.outcome().message().map_or(0, str::len))
            .ok_or(RadrootsTransportError::ResourceLimitExceeded {
                field: "fetch_diagnostic_bytes",
                max: RADROOTS_TRANSPORT_DIAGNOSTIC_MAX_BYTES,
                actual: usize::MAX,
            })?;
        crate::limits::ensure_resource_limit(
            "fetch_diagnostic_bytes",
            diagnostic_bytes,
            RADROOTS_TRANSPORT_DIAGNOSTIC_MAX_BYTES,
        )?;
        let Some(requested_target) = target_set
            .targets()
            .iter()
            .find(|target| target.fingerprint() == receipt.target().fingerprint())
        else {
            return Err(RadrootsTransportError::UnexpectedFetchTargetReceipt);
        };
        if requested_target != receipt.target() {
            return Err(RadrootsTransportError::UnexpectedFetchTargetReceipt);
        }
        if !receipt_fingerprints.insert(receipt.target().fingerprint().as_str()) {
            return Err(RadrootsTransportError::DuplicateFetchTargetReceipt);
        }
    }
    if receipt_fingerprints.len() != target_set.len() {
        return Err(RadrootsTransportError::MissingFetchTargetReceipt);
    }
    let mut receipts_by_fingerprint = target_receipts
        .into_iter()
        .map(|receipt| {
            (
                String::from(receipt.target().fingerprint().as_str()),
                receipt,
            )
        })
        .collect::<BTreeMap<_, _>>();
    target_set
        .targets()
        .iter()
        .map(|target| {
            receipts_by_fingerprint
                .remove(target.fingerprint().as_str())
                .ok_or(RadrootsTransportError::MissingFetchTargetReceipt)
        })
        .collect()
}

#[cfg(feature = "serde")]
fn deserialize_fetch_request_id<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    crate::serde_bounds::deserialize_string(
        deserializer,
        "fetch_request_id",
        RADROOTS_TRANSPORT_FETCH_REQUEST_ID_MAX_BYTES,
    )
}

#[cfg(feature = "serde")]
fn deserialize_fetch_target_receipts<'de, D>(
    deserializer: D,
) -> Result<Vec<RadrootsTransportTargetReceipt>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    crate::serde_bounds::deserialize_vec(
        deserializer,
        "fetch_target_receipt_count",
        RADROOTS_TRANSPORT_TARGET_MAX_COUNT,
    )
}

#[cfg(feature = "serde")]
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RadrootsTransportFetchReceiptWire {
    #[serde(deserialize_with = "deserialize_fetch_request_id")]
    request_id: String,
    target_set: RadrootsTransportTargetSet,
    #[serde(deserialize_with = "deserialize_fetch_target_receipts")]
    target_receipts: Vec<RadrootsTransportTargetReceipt>,
    fetched_count: usize,
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for RadrootsTransportFetchReceipt {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let wire = RadrootsTransportFetchReceiptWire::deserialize(deserializer)?;
        Self::new(
            wire.request_id,
            wire.target_set,
            wire.target_receipts,
            wire.fetched_count,
        )
        .map_err(serde::de::Error::custom)
    }
}
