use super::*;

#[test]
fn validation_receipt_limit_rejects_out_of_range_values() {
    assert!(validate_validation_receipt_limit(1).is_ok());
    assert!(validate_validation_receipt_limit(TRADE_STATUS_MAX_LIMIT).is_ok());

    let zero = validate_validation_receipt_limit(0).expect_err("zero limit");
    assert_eq!(
        zero.to_string(),
        format!("sdk order status limit invalid: limit=0, min=1, max={TRADE_STATUS_MAX_LIMIT}")
    );

    let over_max =
        validate_validation_receipt_limit(TRADE_STATUS_MAX_LIMIT + 1).expect_err("over max");
    assert_eq!(
        over_max.to_string(),
        format!(
            "sdk order status limit invalid: limit={}, min=1, max={TRADE_STATUS_MAX_LIMIT}",
            TRADE_STATUS_MAX_LIMIT + 1
        )
    );
}

#[test]
fn relay_outcome_labels_cover_current_variants() {
    for (kind, label) in [
        (TradeResyncNostrRelayOutcomeKind::Eose, "eose"),
        (TradeResyncNostrRelayOutcomeKind::Closed, "closed"),
        (TradeResyncNostrRelayOutcomeKind::Notice, "notice"),
    ] {
        assert_eq!(kind.as_str(), label);
    }

    for (kind, label) in [
        (TradeValidationReceiptNostrRelayOutcomeKind::Eose, "eose"),
        (
            TradeValidationReceiptNostrRelayOutcomeKind::Closed,
            "closed",
        ),
        (
            TradeValidationReceiptNostrRelayOutcomeKind::Notice,
            "notice",
        ),
    ] {
        assert_eq!(kind.as_str(), label);
    }

    for (kind, label) in [
        (
            TradeResyncNostrRelayTransportOutcomeKind::Accepted,
            "accepted",
        ),
        (
            TradeResyncNostrRelayTransportOutcomeKind::DuplicateAccepted,
            "duplicate_accepted",
        ),
        (
            TradeResyncNostrRelayTransportOutcomeKind::Blocked,
            "blocked",
        ),
        (
            TradeResyncNostrRelayTransportOutcomeKind::RateLimited,
            "rate_limited",
        ),
        (
            TradeResyncNostrRelayTransportOutcomeKind::Invalid,
            "invalid",
        ),
        (
            TradeResyncNostrRelayTransportOutcomeKind::PowRequired,
            "pow_required",
        ),
        (
            TradeResyncNostrRelayTransportOutcomeKind::Restricted,
            "restricted",
        ),
        (
            TradeResyncNostrRelayTransportOutcomeKind::AuthRequired,
            "auth_required",
        ),
        (TradeResyncNostrRelayTransportOutcomeKind::Muted, "muted"),
        (
            TradeResyncNostrRelayTransportOutcomeKind::Unsupported,
            "unsupported",
        ),
        (
            TradeResyncNostrRelayTransportOutcomeKind::PaymentRequired,
            "payment_required",
        ),
        (TradeResyncNostrRelayTransportOutcomeKind::Error, "error"),
        (
            TradeResyncNostrRelayTransportOutcomeKind::Timeout,
            "timeout",
        ),
        (
            TradeResyncNostrRelayTransportOutcomeKind::ConnectionFailed,
            "connection_failed",
        ),
        (
            TradeResyncNostrRelayTransportOutcomeKind::RelayUrlRejected,
            "relay_url_rejected",
        ),
        (
            TradeResyncNostrRelayTransportOutcomeKind::SkippedAlreadyAccepted,
            "skipped_already_accepted",
        ),
        (
            TradeResyncNostrRelayTransportOutcomeKind::Unknown,
            "unknown",
        ),
    ] {
        assert_eq!(kind.as_str(), label);
    }

    for (kind, label) in [
        (
            TradeValidationReceiptNostrRelayTransportOutcomeKind::Accepted,
            "accepted",
        ),
        (
            TradeValidationReceiptNostrRelayTransportOutcomeKind::DuplicateAccepted,
            "duplicate_accepted",
        ),
        (
            TradeValidationReceiptNostrRelayTransportOutcomeKind::Blocked,
            "blocked",
        ),
        (
            TradeValidationReceiptNostrRelayTransportOutcomeKind::RateLimited,
            "rate_limited",
        ),
        (
            TradeValidationReceiptNostrRelayTransportOutcomeKind::Invalid,
            "invalid",
        ),
        (
            TradeValidationReceiptNostrRelayTransportOutcomeKind::PowRequired,
            "pow_required",
        ),
        (
            TradeValidationReceiptNostrRelayTransportOutcomeKind::Restricted,
            "restricted",
        ),
        (
            TradeValidationReceiptNostrRelayTransportOutcomeKind::AuthRequired,
            "auth_required",
        ),
        (
            TradeValidationReceiptNostrRelayTransportOutcomeKind::Muted,
            "muted",
        ),
        (
            TradeValidationReceiptNostrRelayTransportOutcomeKind::Unsupported,
            "unsupported",
        ),
        (
            TradeValidationReceiptNostrRelayTransportOutcomeKind::PaymentRequired,
            "payment_required",
        ),
        (
            TradeValidationReceiptNostrRelayTransportOutcomeKind::Error,
            "error",
        ),
        (
            TradeValidationReceiptNostrRelayTransportOutcomeKind::Timeout,
            "timeout",
        ),
        (
            TradeValidationReceiptNostrRelayTransportOutcomeKind::ConnectionFailed,
            "connection_failed",
        ),
        (
            TradeValidationReceiptNostrRelayTransportOutcomeKind::RelayUrlRejected,
            "relay_url_rejected",
        ),
        (
            TradeValidationReceiptNostrRelayTransportOutcomeKind::SkippedAlreadyAccepted,
            "skipped_already_accepted",
        ),
        (
            TradeValidationReceiptNostrRelayTransportOutcomeKind::Unknown,
            "unknown",
        ),
    ] {
        assert_eq!(kind.as_str(), label);
    }
}

#[test]
fn camel_to_snake_converts_debug_case_labels() {
    assert_eq!(camel_to_snake("AwaitValidation"), "await_validation");
    assert_eq!(
        camel_to_snake("InspectEvidenceIssues"),
        "inspect_evidence_issues"
    );
    assert_eq!(camel_to_snake("Terminal"), "terminal");
}
