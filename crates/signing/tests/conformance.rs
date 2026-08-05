use radroots_signing::{
    Error, Signer,
    capability::{CancellationSupport, SignerCapability, SignerKind},
    error::{CATALOG, Kind},
    recovery::ReplayCapability,
    request::{CancellationPolicy, SignPolicy},
};

#[test]
fn public_error_catalog_is_unique_consistent_and_protocol_redacted() {
    let mut codes = std::collections::BTreeSet::new();
    for descriptor in CATALOG.iter().copied() {
        assert!(codes.insert(descriptor.code()));
        let error = Error::new(descriptor.kind());
        assert_eq!(error.kind(), descriptor.kind());
        assert_eq!(error.code(), descriptor.code());
        assert_eq!(error.class(), descriptor.class());
        assert_eq!(error.retryable(), descriptor.retryable());
        assert_eq!(error.recovery_actions(), descriptor.recovery_actions());
        let report = error.to_report(None);
        assert_eq!(report.code().as_str(), descriptor.code());
        assert_eq!(report.message().as_str(), "[redacted]");
    }
    assert_eq!(codes.len(), Kind::ALL.len());
}

#[test]
fn deadline_and_cancellation_contracts_are_explicit() {
    let error = SignPolicy::new(0, CancellationPolicy::LocalCooperative)
        .expect_err("zero deadline must fail");
    assert_eq!(error.kind(), Kind::InvalidArgument);

    let local = SignPolicy::new(42, CancellationPolicy::LocalCooperative).expect("local policy");
    let remote =
        SignPolicy::new(42, CancellationPolicy::PreservePublishedRequest).expect("remote policy");
    assert_eq!(local.deadline_unix_ms(), 42);
    assert_eq!(local.cancellation(), CancellationPolicy::LocalCooperative);
    assert_eq!(
        remote.cancellation(),
        CancellationPolicy::PreservePublishedRequest
    );

    let capability = SignerCapability::new(
        SignerKind::Remote,
        ReplayCapability::ExactReplayByRequestId,
        CancellationSupport::BeforeAndAfterPublication,
        true,
        true,
    );
    assert_eq!(capability.kind(), SignerKind::Remote);
    assert_eq!(
        capability.replay(),
        ReplayCapability::ExactReplayByRequestId
    );
    assert_eq!(
        capability.cancellation(),
        CancellationSupport::BeforeAndAfterPublication
    );
    assert!(capability.reports_progress());
    assert!(capability.may_require_authentication());
}

#[cfg(feature = "serde")]
#[test]
fn policy_wire_labels_are_stable_and_round_trip() {
    let policy =
        SignPolicy::new(42, CancellationPolicy::PreservePublishedRequest).expect("remote policy");
    let json = serde_json::to_string(&policy).expect("serialize policy");
    assert!(json.contains("preserve_published_request"));
    assert_eq!(
        serde_json::from_str::<SignPolicy>(&json).expect("deserialize policy"),
        policy
    );
    assert!(serde_json::from_str::<SignPolicy>(
        r#"{"deadline_unix_ms":0,"cancellation":"local_cooperative","deprecated_plan":"deny","managed_signing":"any_validated_source"}"#
    )
    .is_err());
}

#[test]
fn public_service_types_preserve_dyn_and_thread_safety() {
    fn assert_dyn(_: &dyn Signer) {}
    fn assert_send_sync<T: Send + Sync + ?Sized>() {}

    let _ = assert_dyn;
    assert_send_sync::<dyn Signer>();
    assert_send_sync::<Error>();
}

#[cfg(feature = "std")]
#[test]
fn native_sources_are_opt_in_and_never_appear_in_diagnostics() {
    use std::error::Error as _;

    #[derive(Debug)]
    struct Sensitive;

    impl core::fmt::Display for Sensitive {
        fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            formatter.write_str("nsec1-integration-secret")
        }
    }

    impl std::error::Error for Sensitive {}

    let error = Error::with_source(Kind::SignerUnavailable, Sensitive);
    assert!(error.source().is_some());
    assert!(!error.to_string().contains("nsec1"));
    assert!(!format!("{error:?}").contains("nsec1"));
    assert!(!serde_or_debug_report(&error).contains("nsec1"));
}

#[cfg(all(feature = "std", feature = "serde"))]
fn serde_or_debug_report(error: &Error) -> String {
    serde_json::to_string(&error.to_report(None)).expect("serialize report")
}

#[cfg(all(feature = "std", not(feature = "serde")))]
fn serde_or_debug_report(error: &Error) -> String {
    format!("{:?}", error.to_report(None))
}
