use radroots_transport::Error;

#[test]
fn every_transport_error_has_stable_operator_facing_text() {
    let cases = [
        (
            Error::UnsupportedOperation,
            "transport operation is unsupported",
        ),
        (Error::EmptyTransportKind, "transport kind is empty"),
        (Error::InvalidTransportKind, "transport kind is invalid"),
        (Error::EmptyTargetUri, "transport target URI is empty"),
        (Error::InvalidTargetUri, "transport target URI is invalid"),
        (Error::EmptyTargetScope, "transport target scope is empty"),
        (
            Error::InvalidTargetScope,
            "transport target scope is invalid",
        ),
        (Error::EmptyTargetLabel, "transport target label is empty"),
        (
            Error::InvalidTargetLabel,
            "transport target label is invalid",
        ),
        (Error::EmptyTargetSet, "transport target set is empty"),
        (
            Error::TargetSetTooLarge,
            "transport target set exceeds its item limit",
        ),
        (
            Error::DuplicateTargetFingerprint,
            "transport target set contains duplicate fingerprints",
        ),
        (
            Error::InvalidTargetFingerprint,
            "transport target fingerprint is invalid",
        ),
        (
            Error::EmptyFetchRequestId,
            "transport fetch request id is empty",
        ),
        (
            Error::InvalidFetchRequestId,
            "transport fetch request id is invalid",
        ),
        (Error::InvalidFetchLimit, "transport fetch limit is invalid"),
        (
            Error::InvalidFetchDeadline,
            "transport fetch deadline is invalid",
        ),
        (
            Error::FetchSelectorTooLarge,
            "transport fetch selector exceeds its item limit",
        ),
        (
            Error::DuplicateFetchKind,
            "transport fetch selector contains a duplicate event kind",
        ),
        (
            Error::DuplicateFetchAuthor,
            "transport fetch selector contains a duplicate author",
        ),
        (
            Error::InvalidFetchTagKey,
            "transport fetch selector tag key is invalid",
        ),
        (
            Error::InvalidFetchTagValue,
            "transport fetch selector tag value is invalid",
        ),
        (
            Error::DuplicateFetchTagValue,
            "transport fetch selector contains a duplicate tag value",
        ),
        (
            Error::InvalidFetchTimeRange,
            "transport fetch selector time range is invalid",
        ),
        (Error::EmptyFetchCursor, "transport fetch cursor is empty"),
        (
            Error::InvalidFetchCursor,
            "transport fetch cursor is invalid",
        ),
        (
            Error::InvalidObservedAt,
            "transport event observation time is invalid",
        ),
        (
            Error::UnexpectedFetchEvent,
            "transport fetch page contains an event outside its selector",
        ),
        (
            Error::UnexpectedFetchProvenance,
            "transport fetch page contains unexpected event provenance",
        ),
        (
            Error::UnexpectedFetchTargetOutcome,
            "transport fetch page contains an unexpected target outcome",
        ),
        (
            Error::DuplicateFetchTargetOutcome,
            "transport fetch page contains a duplicate target outcome",
        ),
        (
            Error::FetchPageLimitExceeded,
            "transport fetch page exceeds its requested event limit",
        ),
        (
            Error::FetchPageRequestMismatch,
            "transport fetch page does not match its request",
        ),
        (
            Error::EmptySubscriptionRequestId,
            "transport subscription request id is empty",
        ),
        (
            Error::InvalidSubscriptionRequestId,
            "transport subscription request id is invalid",
        ),
        (
            Error::InvalidSubscriptionLimit,
            "transport subscription event limit is invalid",
        ),
        (
            Error::InvalidSubscriptionDeadline,
            "transport subscription deadline is invalid",
        ),
        (
            Error::SubscriptionCheckpointSetTooLarge,
            "transport subscription checkpoint set exceeds its target limit",
        ),
        (
            Error::UnexpectedSubscriptionCheckpoint,
            "transport subscription contains an unexpected checkpoint",
        ),
        (
            Error::DuplicateSubscriptionCheckpoint,
            "transport subscription contains a duplicate checkpoint",
        ),
        (
            Error::UnexpectedSubscriptionEvent,
            "transport subscription contains an unexpected event",
        ),
        (
            Error::SubscriptionEventCheckpointMismatch,
            "transport subscription event checkpoint does not match provenance",
        ),
        (
            Error::SubscriptionEndLimitExceeded,
            "transport subscription exceeded its requested event limit",
        ),
        (
            Error::InvalidSubscriptionEnd,
            "transport subscription terminal result is invalid",
        ),
        (
            Error::SubscriptionEndRequestMismatch,
            "transport subscription result does not match its request",
        ),
        (
            Error::SubscriptionUnavailable,
            "transport subscription is unavailable",
        ),
        (
            Error::InvalidSatisfactionPolicy,
            "transport satisfaction policy is invalid",
        ),
        (
            Error::EmptyRequiredTargetSet,
            "transport required target set is empty",
        ),
        (
            Error::DuplicateRequiredTargetFingerprint,
            "transport required target set contains duplicate fingerprints",
        ),
        (
            Error::RequiredTargetNotRequested,
            "transport required target was not requested",
        ),
        (
            Error::EmptyDeliveryRequestId,
            "transport delivery request id is empty",
        ),
        (
            Error::InvalidDeliveryRequestId,
            "transport delivery request id is invalid",
        ),
        (
            Error::InvalidDeliveryTimestamp,
            "transport delivery timestamp is invalid",
        ),
        (
            Error::InvalidDeliveryDeadline,
            "transport delivery deadline is invalid",
        ),
        (
            Error::InvalidDeliveryOutcome,
            "transport delivery outcome is invalid",
        ),
        (
            Error::UnexpectedDeliveryTargetReceipt,
            "transport delivery receipt contains an unexpected target",
        ),
        (
            Error::DuplicateDeliveryTargetReceipt,
            "transport delivery receipt contains a duplicate target",
        ),
        (
            Error::MissingDeliveryTargetReceipt,
            "transport delivery receipt is missing a requested target",
        ),
        (
            Error::DeliveryTargetReceiptStatusMismatch,
            "transport delivery target receipt status does not match its outcome",
        ),
        (
            Error::DeliveryTargetReceiptAttemptMismatch,
            "transport delivery target receipt attempt evidence is incoherent",
        ),
        (
            Error::TransportOutcomeStatusMismatch,
            "transport outcome status does not match its outcome kind",
        ),
        (
            Error::DeliveryReceiptRequestIdMismatch,
            "transport delivery receipt request id does not match its request",
        ),
        (
            Error::DeliveryReceiptTargetSetMismatch,
            "transport delivery receipt target set does not match its request",
        ),
        (Error::EmptyPayloadId, "transport payload id is empty"),
        (Error::InvalidPayloadId, "transport payload id is invalid"),
        (Error::EmptyPayloadLabel, "transport payload label is empty"),
        (
            Error::InvalidPayloadLabel,
            "transport payload label is invalid",
        ),
        (
            Error::EmptyPayloadBytes,
            "transport payload bytes are empty",
        ),
        (
            Error::InvalidPayloadBytes,
            "transport payload bytes are invalid",
        ),
        (
            Error::InvalidPayloadDigest,
            "transport payload digest is invalid",
        ),
        (
            Error::PayloadDigestMismatch,
            "transport payload digest does not match payload bytes",
        ),
    ];

    for (error, expected) in cases {
        assert_eq!(error.to_string(), expected);
    }
}
