use core::fmt;

/// Transport contract failure.
pub type Error = RadrootsTransportError;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsTransportError {
    UnsupportedOperation,
    EmptyTransportKind,
    InvalidTransportKind,
    EmptyTargetUri,
    InvalidTargetUri,
    EmptyTargetScope,
    InvalidTargetScope,
    EmptyTargetLabel,
    InvalidTargetLabel,
    EmptyTargetSet,
    TargetSetTooLarge,
    DuplicateTargetFingerprint,
    InvalidTargetFingerprint,
    EmptyFetchRequestId,
    InvalidFetchRequestId,
    InvalidFetchLimit,
    InvalidFetchDeadline,
    EmptyFetchCursor,
    InvalidFetchCursor,
    InvalidObservedAt,
    UnexpectedFetchProvenance,
    UnexpectedFetchTargetOutcome,
    DuplicateFetchTargetOutcome,
    FetchPageLimitExceeded,
    FetchPageRequestMismatch,
    InvalidSatisfactionPolicy,
    EmptyRequiredTargetSet,
    DuplicateRequiredTargetFingerprint,
    RequiredTargetNotRequested,
    EmptyDeliveryRequestId,
    InvalidDeliveryRequestId,
    InvalidDeliveryTimestamp,
    UnexpectedDeliveryTargetReceipt,
    DuplicateDeliveryTargetReceipt,
    MissingDeliveryTargetReceipt,
    DeliveryTargetReceiptStatusMismatch,
    DeliveryTargetReceiptAttemptMismatch,
    TransportOutcomeStatusMismatch,
    DeliveryReceiptRequestIdMismatch,
    DeliveryReceiptTargetSetMismatch,
    EmptyPayloadId,
    InvalidPayloadId,
    EmptyPayloadLabel,
    InvalidPayloadLabel,
    EmptyPayloadBytes,
    InvalidPayloadBytes,
    InvalidPayloadDigest,
    PayloadDigestMismatch,
}

impl fmt::Display for RadrootsTransportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedOperation => f.write_str("transport operation is unsupported"),
            Self::EmptyTransportKind => f.write_str("transport kind is empty"),
            Self::InvalidTransportKind => f.write_str("transport kind is invalid"),
            Self::EmptyTargetUri => f.write_str("transport target URI is empty"),
            Self::InvalidTargetUri => f.write_str("transport target URI is invalid"),
            Self::EmptyTargetScope => f.write_str("transport target scope is empty"),
            Self::InvalidTargetScope => f.write_str("transport target scope is invalid"),
            Self::EmptyTargetLabel => f.write_str("transport target label is empty"),
            Self::InvalidTargetLabel => f.write_str("transport target label is invalid"),
            Self::EmptyTargetSet => f.write_str("transport target set is empty"),
            Self::TargetSetTooLarge => f.write_str("transport target set exceeds its item limit"),
            Self::DuplicateTargetFingerprint => {
                f.write_str("transport target set contains duplicate fingerprints")
            }
            Self::InvalidTargetFingerprint => {
                f.write_str("transport target fingerprint is invalid")
            }
            Self::EmptyFetchRequestId => f.write_str("transport fetch request id is empty"),
            Self::InvalidFetchRequestId => f.write_str("transport fetch request id is invalid"),
            Self::InvalidFetchLimit => f.write_str("transport fetch limit is invalid"),
            Self::InvalidFetchDeadline => f.write_str("transport fetch deadline is invalid"),
            Self::EmptyFetchCursor => f.write_str("transport fetch cursor is empty"),
            Self::InvalidFetchCursor => f.write_str("transport fetch cursor is invalid"),
            Self::InvalidObservedAt => f.write_str("transport event observation time is invalid"),
            Self::UnexpectedFetchProvenance => {
                f.write_str("transport fetch page contains unexpected event provenance")
            }
            Self::UnexpectedFetchTargetOutcome => {
                f.write_str("transport fetch page contains an unexpected target outcome")
            }
            Self::DuplicateFetchTargetOutcome => {
                f.write_str("transport fetch page contains a duplicate target outcome")
            }
            Self::FetchPageLimitExceeded => {
                f.write_str("transport fetch page exceeds its requested event limit")
            }
            Self::FetchPageRequestMismatch => {
                f.write_str("transport fetch page does not match its request")
            }
            Self::InvalidSatisfactionPolicy => {
                f.write_str("transport satisfaction policy is invalid")
            }
            Self::EmptyRequiredTargetSet => f.write_str("transport required target set is empty"),
            Self::DuplicateRequiredTargetFingerprint => {
                f.write_str("transport required target set contains duplicate fingerprints")
            }
            Self::RequiredTargetNotRequested => {
                f.write_str("transport required target was not requested")
            }
            Self::EmptyDeliveryRequestId => f.write_str("transport delivery request id is empty"),
            Self::InvalidDeliveryRequestId => {
                f.write_str("transport delivery request id is invalid")
            }
            Self::InvalidDeliveryTimestamp => {
                f.write_str("transport delivery timestamp is invalid")
            }
            Self::UnexpectedDeliveryTargetReceipt => {
                f.write_str("transport delivery receipt contains an unexpected target")
            }
            Self::DuplicateDeliveryTargetReceipt => {
                f.write_str("transport delivery receipt contains a duplicate target")
            }
            Self::MissingDeliveryTargetReceipt => {
                f.write_str("transport delivery receipt is missing a requested target")
            }
            Self::DeliveryTargetReceiptStatusMismatch => {
                f.write_str("transport delivery target receipt status does not match its outcome")
            }
            Self::DeliveryTargetReceiptAttemptMismatch => {
                f.write_str("transport delivery target receipt attempt evidence is incoherent")
            }
            Self::TransportOutcomeStatusMismatch => {
                f.write_str("transport outcome status does not match its outcome kind")
            }
            Self::DeliveryReceiptRequestIdMismatch => {
                f.write_str("transport delivery receipt request id does not match its request")
            }
            Self::DeliveryReceiptTargetSetMismatch => {
                f.write_str("transport delivery receipt target set does not match its request")
            }
            Self::EmptyPayloadId => f.write_str("transport payload id is empty"),
            Self::InvalidPayloadId => f.write_str("transport payload id is invalid"),
            Self::EmptyPayloadLabel => f.write_str("transport payload label is empty"),
            Self::InvalidPayloadLabel => f.write_str("transport payload label is invalid"),
            Self::EmptyPayloadBytes => f.write_str("transport payload bytes are empty"),
            Self::InvalidPayloadBytes => f.write_str("transport payload bytes are invalid"),
            Self::InvalidPayloadDigest => f.write_str("transport payload digest is invalid"),
            Self::PayloadDigestMismatch => {
                f.write_str("transport payload digest does not match payload bytes")
            }
        }
    }
}
