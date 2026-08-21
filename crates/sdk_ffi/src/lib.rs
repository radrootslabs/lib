//! Private mobile FFI over the shared SDK engine.

#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

use std::sync::Arc;

use radroots_sdk::{Client, ClientBuilder};

uniffi::setup_scaffolding!("radroots_sdk");

/// Generation-1 mobile DTOs.
pub mod v1 {
    use super::*;

    /// Stable capability maturity independent of runtime availability.
    #[derive(Clone, Copy, Debug, Eq, PartialEq, uniffi::Enum)]
    pub enum CapabilityMaturity {
        Stable,
        Preview,
        Experimental,
    }

    /// Current side-effect-free capability availability.
    #[derive(Clone, Copy, Debug, Eq, PartialEq, uniffi::Enum)]
    pub enum CapabilityAvailability {
        Available,
        Degraded,
        Unavailable,
        Unsupported,
    }

    /// Versioned, presentation-independent capability observation.
    #[derive(Clone, Debug, Eq, PartialEq, uniffi::Record)]
    pub struct CapabilityStatus {
        pub id: String,
        pub compiled: bool,
        pub configured: bool,
        pub availability: CapabilityAvailability,
        pub maturity: CapabilityMaturity,
    }

    /// Secret-safe SDK failure at the language boundary.
    #[derive(Debug, thiserror::Error, uniffi::Error)]
    pub enum Error {
        #[error("SDK {code}: {message}")]
        Sdk {
            code: String,
            message: String,
            retryable: bool,
        },
    }

    impl From<radroots_sdk::Error> for Error {
        fn from(error: radroots_sdk::Error) -> Self {
            let descriptor = error.descriptor();
            Self::Sdk {
                code: descriptor.code().as_str().to_owned(),
                message: descriptor.message().to_owned(),
                retryable: descriptor.retryable(),
            }
        }
    }

    impl Error {
        fn contract(code: &'static str, message: &'static str) -> Self {
            Self::Sdk {
                code: code.to_owned(),
                message: message.to_owned(),
                retryable: false,
            }
        }
    }

    /// Final evidence coverage vocabulary.
    #[derive(Clone, Copy, Debug, Eq, PartialEq, uniffi::Enum)]
    pub enum TradeEvidenceCoverage {
        Missing,
        Partial,
        ScopeSatisfied,
        Unsupported,
    }

    /// Final evidence outcome vocabulary.
    #[derive(Clone, Copy, Debug, Eq, PartialEq, uniffi::Enum)]
    pub enum TradeEvidenceOutcome {
        Valid,
        Invalid,
        Indeterminate,
    }

    /// Bounded projection of one canonical evidence manifest.
    #[derive(Clone, Debug, Eq, PartialEq, uniffi::Record)]
    pub struct TradeEvidenceManifest {
        pub contract_id: String,
        pub contract_version: u16,
        pub trade_id: String,
        pub trade_generation: String,
        pub observed_at_unix_s: String,
        pub coverage: TradeEvidenceCoverage,
        pub evidence_policy_digest: String,
        pub manifest_digest: String,
        pub canonical_bytes_hex: String,
    }

    /// Exact immutable RHI supersession reference.
    #[derive(Clone, Debug, Eq, PartialEq, uniffi::Record)]
    pub struct RhiEvidenceSupersession {
        pub report_id: String,
        pub event_id: String,
    }

    /// Bounded projection of one canonical RHI evidence report.
    #[derive(Clone, Debug, Eq, PartialEq, uniffi::Record)]
    pub struct RhiEvidenceReport {
        pub contract_id: String,
        pub contract_version: u16,
        pub issuer_pubkey: String,
        pub trade_id: String,
        pub claim_mutation_id: String,
        pub outcome: TradeEvidenceOutcome,
        pub reason_codes: Vec<String>,
        pub projection_digest: String,
        pub evidence_manifest_digest: String,
        pub evidence_policy_digest: String,
        pub observed_at_unix_s: String,
        pub trade_generation: String,
        pub statement_digest: String,
        pub supersession: Option<RhiEvidenceSupersession>,
        pub canonical_content: String,
    }

    /// One unsigned typed event plan ready for host-owned signing.
    #[derive(Clone, Debug, Eq, PartialEq, uniffi::Record)]
    pub struct TypedEvidenceEventPlan {
        pub contract_id: String,
        pub kind: u32,
        pub author_pubkey: String,
        pub created_at_unix_s: String,
        pub expected_event_id: String,
        pub tags: Vec<Vec<String>>,
        pub content: String,
    }

    /// Signed NIP-01 event input for verified attestation admission.
    #[derive(Clone, Debug, Eq, PartialEq, uniffi::Record)]
    pub struct SignedEvent {
        pub id: String,
        pub author_pubkey: String,
        pub created_at_unix_s: u64,
        pub kind: u32,
        pub tags: Vec<Vec<String>>,
        pub content: String,
        pub signature: String,
    }

    /// Verified final RHI attestation projection.
    #[derive(Clone, Debug, Eq, PartialEq, uniffi::Record)]
    pub struct RhiEvidenceAttestation {
        pub issuer_pubkey: String,
        pub trade_id: String,
        pub claim_mutation_id: String,
        pub outcome: TradeEvidenceOutcome,
        pub observed_at_unix_s: String,
        pub trade_generation: String,
        pub statement_digest: String,
        pub supersession: Option<RhiEvidenceSupersession>,
        pub canonical_content: String,
    }

    #[uniffi::export]
    pub fn parse_trade_evidence_manifest(
        canonical_bytes: Vec<u8>,
    ) -> Result<TradeEvidenceManifest, Error> {
        let manifest =
            radroots_sdk::trade::parse_evidence_manifest(&canonical_bytes).map_err(|_| {
                Error::contract("invalid_evidence_manifest", "evidence manifest is invalid")
            })?;
        Ok(TradeEvidenceManifest::from(&manifest))
    }

    #[uniffi::export]
    pub fn parse_rhi_evidence_report(
        canonical_content: String,
    ) -> Result<RhiEvidenceReport, Error> {
        let report = radroots_sdk::trade::parse_rhi_evidence_report(canonical_content.as_bytes())
            .map_err(|_| {
            Error::contract("invalid_evidence_report", "evidence report is invalid")
        })?;
        Ok(RhiEvidenceReport::from(&report))
    }

    #[uniffi::export]
    pub fn prepare_rhi_evidence_attestation(
        canonical_content: String,
        created_at_unix_s: u64,
    ) -> Result<TypedEvidenceEventPlan, Error> {
        let report = radroots_sdk::trade::parse_rhi_evidence_report(canonical_content.as_bytes())
            .map_err(|_| {
            Error::contract("invalid_evidence_report", "evidence report is invalid")
        })?;
        let plan =
            radroots_sdk::trade::prepare_rhi_evidence_attestation(&report, created_at_unix_s)
                .map_err(|_| {
                    Error::contract(
                        "invalid_evidence_attestation_plan",
                        "evidence attestation plan is invalid",
                    )
                })?;
        Ok(TypedEvidenceEventPlan::from(&plan))
    }

    #[uniffi::export]
    pub fn validate_rhi_evidence_attestation(
        event: SignedEvent,
    ) -> Result<RhiEvidenceAttestation, Error> {
        let event = radroots_event::envelope::EventEnvelope::new(
            radroots_event::envelope::EventEnvelopeParts {
                id: event.id,
                author: event.author_pubkey,
                created_at: event.created_at_unix_s,
                kind: event.kind,
                tags: event.tags,
                content: event.content,
                sig: event.signature,
            },
        )
        .map_err(|_| Error::contract("invalid_signed_event", "signed event is invalid"))?;
        let attestation =
            radroots_sdk::trade::validate_rhi_evidence_attestation(event).map_err(|error| {
                match error {
                    radroots_sdk::trade::EvidenceAttestationValidationError::Signature => {
                        Error::contract(
                            "invalid_event_signature",
                            "event signature validation failed",
                        )
                    }
                    radroots_sdk::trade::EvidenceAttestationValidationError::Contract => {
                        Error::contract(
                            "invalid_evidence_attestation",
                            "evidence attestation is invalid",
                        )
                    }
                }
            })?;
        Ok(RhiEvidenceAttestation::from(&attestation))
    }

    impl From<&radroots_sdk::trade::RadrootsTradeEvidenceManifestV1> for TradeEvidenceManifest {
        fn from(value: &radroots_sdk::trade::RadrootsTradeEvidenceManifestV1) -> Self {
            Self {
                contract_id: value.contract_id().to_owned(),
                contract_version: value.contract_version(),
                trade_id: value.trade_id().to_string(),
                trade_generation: value.trade_generation().get().to_string(),
                observed_at_unix_s: value.observed_at_unix_s().to_string(),
                coverage: value.coverage().into(),
                evidence_policy_digest: value.evidence_policy_digest().to_hex(),
                manifest_digest: value.digest().to_hex(),
                canonical_bytes_hex: hex::encode(value.canonical_bytes()),
            }
        }
    }

    impl From<radroots_sdk::trade::RadrootsTradeEvidenceCoverageV1> for TradeEvidenceCoverage {
        fn from(value: radroots_sdk::trade::RadrootsTradeEvidenceCoverageV1) -> Self {
            match value {
                radroots_sdk::trade::RadrootsTradeEvidenceCoverageV1::Missing => Self::Missing,
                radroots_sdk::trade::RadrootsTradeEvidenceCoverageV1::Partial => Self::Partial,
                radroots_sdk::trade::RadrootsTradeEvidenceCoverageV1::ScopeSatisfied => {
                    Self::ScopeSatisfied
                }
                radroots_sdk::trade::RadrootsTradeEvidenceCoverageV1::Unsupported => {
                    Self::Unsupported
                }
            }
        }
    }

    impl From<radroots_sdk::trade::RadrootsTradeEvidenceOutcomeV1> for TradeEvidenceOutcome {
        fn from(value: radroots_sdk::trade::RadrootsTradeEvidenceOutcomeV1) -> Self {
            match value {
                radroots_sdk::trade::RadrootsTradeEvidenceOutcomeV1::Valid => Self::Valid,
                radroots_sdk::trade::RadrootsTradeEvidenceOutcomeV1::Invalid => Self::Invalid,
                radroots_sdk::trade::RadrootsTradeEvidenceOutcomeV1::Indeterminate => {
                    Self::Indeterminate
                }
            }
        }
    }

    impl From<&radroots_sdk::trade::RadrootsRhiEvidenceReportV1> for RhiEvidenceReport {
        fn from(value: &radroots_sdk::trade::RadrootsRhiEvidenceReportV1) -> Self {
            Self {
                contract_id: value.contract_id().to_owned(),
                contract_version: value.contract_version(),
                issuer_pubkey: value.issuer_public_key().to_hex(),
                trade_id: value.trade_id().to_string(),
                claim_mutation_id: value.claim_mutation_id().to_string(),
                outcome: value.outcome().into(),
                reason_codes: value
                    .reason_codes()
                    .iter()
                    .map(|code| code.as_str().to_owned())
                    .collect(),
                projection_digest: value.projection_digest().to_hex(),
                evidence_manifest_digest: value.evidence_manifest_digest().to_hex(),
                evidence_policy_digest: value.evidence_policy_digest().to_hex(),
                observed_at_unix_s: value.observed_at_unix_s().to_string(),
                trade_generation: value.trade_generation().get().to_string(),
                statement_digest: value.statement_digest().to_hex(),
                supersession: value.supersession().map(RhiEvidenceSupersession::from),
                canonical_content: value.canonical_content().to_owned(),
            }
        }
    }

    impl From<radroots_sdk::trade::RadrootsRhiEvidenceSupersessionV1> for RhiEvidenceSupersession {
        fn from(value: radroots_sdk::trade::RadrootsRhiEvidenceSupersessionV1) -> Self {
            Self {
                report_id: value.report_id().to_hex(),
                event_id: value.event_id().to_hex(),
            }
        }
    }

    impl From<&radroots_event_codec::authoring::AuthoredEventPlan> for TypedEvidenceEventPlan {
        fn from(value: &radroots_event_codec::authoring::AuthoredEventPlan) -> Self {
            Self {
                contract_id: value.body().contract().contract_id().as_str().to_owned(),
                kind: value.body().kind(),
                author_pubkey: value.author().to_hex(),
                created_at_unix_s: value.created_at().to_string(),
                expected_event_id: value.expected_event_id().to_hex(),
                tags: value.body().tags().to_vec(),
                content: value.body().content().to_owned(),
            }
        }
    }

    impl From<&radroots_sdk::trade::RadrootsRhiEvidenceAttestationV1> for RhiEvidenceAttestation {
        fn from(value: &radroots_sdk::trade::RadrootsRhiEvidenceAttestationV1) -> Self {
            Self {
                issuer_pubkey: value.issuer().to_hex(),
                trade_id: value.trade_id().to_string(),
                claim_mutation_id: value.claim_mutation_id().to_string(),
                outcome: match value.outcome() {
                    radroots_sdk::trade::RadrootsRhiEvidenceAttestationOutcomeV1::Valid => {
                        TradeEvidenceOutcome::Valid
                    }
                    radroots_sdk::trade::RadrootsRhiEvidenceAttestationOutcomeV1::Invalid => {
                        TradeEvidenceOutcome::Invalid
                    }
                    radroots_sdk::trade::RadrootsRhiEvidenceAttestationOutcomeV1::Indeterminate => {
                        TradeEvidenceOutcome::Indeterminate
                    }
                },
                observed_at_unix_s: value.observed_at_unix_s().to_string(),
                trade_generation: value.trade_generation().get().to_string(),
                statement_digest: hex::encode(value.statement_digest()),
                supersession: value
                    .supersession()
                    .map(|supersession| RhiEvidenceSupersession {
                        report_id: hex::encode(supersession.report_id()),
                        event_id: supersession.event_id().to_hex(),
                    }),
                canonical_content: value.canonical_content().to_owned(),
            }
        }
    }

    /// Thread-safe mobile handle delegating lifecycle to [`radroots_sdk::Client`].
    #[derive(uniffi::Object)]
    pub struct MobileClient {
        client: Client,
    }

    #[uniffi::export]
    impl MobileClient {
        /// Creates a deterministic, local-only memory client without I/O.
        #[uniffi::constructor]
        pub fn memory() -> Result<Arc<Self>, Error> {
            let client = ClientBuilder::memory_default().build()?;
            Ok(Arc::new(Self { client }))
        }

        /// Returns stable capability DTOs without probing host resources.
        pub fn capabilities(&self) -> Vec<CapabilityStatus> {
            self.client
                .capabilities()
                .iter()
                .copied()
                .map(CapabilityStatus::from)
                .collect()
        }

        /// Returns whether explicit SDK close completed across all clones.
        pub fn is_closed(&self) -> bool {
            self.client.is_closed()
        }

        /// Explicitly closes SDK-owned resources without installing a runtime.
        pub async fn close(&self) -> Result<(), Error> {
            self.client.close().await.map_err(Error::from)
        }
    }

    impl From<radroots_sdk::capability::CapabilityStatus> for CapabilityStatus {
        fn from(status: radroots_sdk::capability::CapabilityStatus) -> Self {
            Self {
                id: status.id().as_str().to_owned(),
                compiled: status.is_compiled(),
                configured: status.is_configured(),
                availability: match status.availability() {
                    radroots_sdk::capability::Availability::Available => {
                        CapabilityAvailability::Available
                    }
                    radroots_sdk::capability::Availability::Degraded => {
                        CapabilityAvailability::Degraded
                    }
                    radroots_sdk::capability::Availability::Unavailable => {
                        CapabilityAvailability::Unavailable
                    }
                    radroots_sdk::capability::Availability::Unsupported => {
                        CapabilityAvailability::Unsupported
                    }
                },
                maturity: match status.maturity() {
                    radroots_sdk::capability::Maturity::Stable => CapabilityMaturity::Stable,
                    radroots_sdk::capability::Maturity::Preview => CapabilityMaturity::Preview,
                    radroots_sdk::capability::Maturity::Experimental => {
                        CapabilityMaturity::Experimental
                    }
                },
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use core::num::NonZeroU64;

    use radroots_event::id::TradeId;

    use super::v1::{
        CapabilityAvailability, Error, MobileClient, SignedEvent, TradeEvidenceCoverage,
        TradeEvidenceOutcome, parse_rhi_evidence_report, parse_trade_evidence_manifest,
        prepare_rhi_evidence_attestation, validate_rhi_evidence_attestation,
    };

    fn assert_send_sync<T: Send + Sync>() {}

    #[tokio::test(flavor = "current_thread")]
    async fn memory_client_delegates_capabilities_and_lifecycle() {
        assert_send_sync::<MobileClient>();
        let client = MobileClient::memory().expect("memory client");
        let capabilities = client.capabilities();
        let storage = capabilities
            .iter()
            .find(|status| status.id == "storage.canonical")
            .expect("canonical storage");
        assert!(storage.compiled);
        assert!(storage.configured);
        assert_eq!(storage.availability, CapabilityAvailability::Available);
        assert!(!client.is_closed());

        client.close().await.expect("close");
        assert!(client.is_closed());
        client.close().await.expect("idempotent close");
    }

    #[test]
    fn sdk_error_mapping_is_versioned_and_secret_safe() {
        let native = radroots_sdk::ClientBuilder::new()
            .build()
            .expect_err("missing storage");
        let error = Error::from(native);
        let Error::Sdk {
            code,
            message,
            retryable,
        } = error;
        assert_eq!(code, "missing_storage");
        assert_eq!(message, "SDK storage capability is not configured");
        assert!(!retryable);
    }

    #[test]
    fn final_report_plan_and_signed_event_cross_the_ffi_without_numeric_narrowing() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../contracts/conformance/vectors/event/authored_operations.v1.json"
        ))
        .expect("authored corpus");
        let expected = fixture["vectors"]
            .as_array()
            .expect("operations")
            .iter()
            .find(|entry| entry["id"] == "typed_rhi_evidence_attestation_017")
            .expect("RHI operation")
            .get("expected")
            .expect("expected");
        let content = expected["content"].as_str().expect("content").to_owned();

        let report = parse_rhi_evidence_report(content.clone()).expect("report");
        assert_eq!(report.outcome, TradeEvidenceOutcome::Indeterminate);
        assert_eq!(report.trade_generation, "7");
        assert_eq!(report.observed_at_unix_s, "1800000000");
        assert_eq!(report.canonical_content, content);

        let plan = prepare_rhi_evidence_attestation(content, 1_784_347_200).expect("plan");
        assert_eq!(plan.kind, 3_441);
        assert_eq!(plan.created_at_unix_s, "1784347200");
        assert_eq!(
            plan.expected_event_id,
            expected["event_id"].as_str().expect("expected event id")
        );

        let raw: serde_json::Value =
            serde_json::from_str(expected["raw_json"].as_str().expect("raw event"))
                .expect("raw event JSON");
        let signed = SignedEvent {
            id: raw["id"].as_str().expect("id").to_owned(),
            author_pubkey: raw["pubkey"].as_str().expect("pubkey").to_owned(),
            created_at_unix_s: raw["created_at"].as_u64().expect("created_at"),
            kind: u32::try_from(raw["kind"].as_u64().expect("kind")).expect("u32 kind"),
            tags: serde_json::from_value(raw["tags"].clone()).expect("tags"),
            content: raw["content"].as_str().expect("content").to_owned(),
            signature: raw["sig"].as_str().expect("signature").to_owned(),
        };
        let attestation = validate_rhi_evidence_attestation(signed).expect("signed attestation");
        assert_eq!(attestation.outcome, TradeEvidenceOutcome::Indeterminate);
        assert_eq!(attestation.trade_generation, "7");
    }

    #[test]
    fn manifest_supersession_and_closed_evidence_vocabularies_cross_the_ffi() {
        use radroots_sdk::trade::{
            RadrootsTradeEvidenceManifestSourceResultV1, RadrootsTradeEvidenceManifestV1,
            RadrootsTradeEvidencePolicyDigestV1, RadrootsTradeEvidenceScopePrerequisitesV1,
            RadrootsTradeEvidenceSourceCompletionV1, RadrootsTradeEvidenceSourceIdV1,
            RadrootsTradeEvidenceSourceRequirementV1, RadrootsTradeEvidenceSourceResultDigestV1,
            RadrootsTradeEvidenceSourceResultV1,
        };

        for (source, expected) in [
            (
                radroots_sdk::trade::RadrootsTradeEvidenceCoverageV1::Missing,
                TradeEvidenceCoverage::Missing,
            ),
            (
                radroots_sdk::trade::RadrootsTradeEvidenceCoverageV1::Partial,
                TradeEvidenceCoverage::Partial,
            ),
            (
                radroots_sdk::trade::RadrootsTradeEvidenceCoverageV1::ScopeSatisfied,
                TradeEvidenceCoverage::ScopeSatisfied,
            ),
            (
                radroots_sdk::trade::RadrootsTradeEvidenceCoverageV1::Unsupported,
                TradeEvidenceCoverage::Unsupported,
            ),
        ] {
            assert_eq!(TradeEvidenceCoverage::from(source), expected);
        }
        for (source, expected) in [
            (
                radroots_sdk::trade::RadrootsTradeEvidenceOutcomeV1::Valid,
                TradeEvidenceOutcome::Valid,
            ),
            (
                radroots_sdk::trade::RadrootsTradeEvidenceOutcomeV1::Invalid,
                TradeEvidenceOutcome::Invalid,
            ),
            (
                radroots_sdk::trade::RadrootsTradeEvidenceOutcomeV1::Indeterminate,
                TradeEvidenceOutcome::Indeterminate,
            ),
        ] {
            assert_eq!(TradeEvidenceOutcome::from(source), expected);
        }

        let source = RadrootsTradeEvidenceManifestSourceResultV1::new(
            RadrootsTradeEvidenceSourceIdV1::parse("typed_source").expect("source id"),
            RadrootsTradeEvidenceSourceResultV1::new(
                RadrootsTradeEvidenceSourceRequirementV1::Required,
                RadrootsTradeEvidenceSourceCompletionV1::Complete,
                0,
            )
            .expect("source result"),
            RadrootsTradeEvidenceSourceResultDigestV1::from_bytes([0x33; 32]),
        );
        let manifest = RadrootsTradeEvidenceManifestV1::new(
            TradeId::from_bytes([0x11; 16]),
            NonZeroU64::new(1).expect("nonzero generation"),
            RadrootsTradeEvidencePolicyDigestV1::from_bytes([0x22; 32]),
            1_800_000_000,
            RadrootsTradeEvidenceScopePrerequisitesV1::Satisfied,
            [source],
            [],
        )
        .expect("manifest");
        let projected = parse_trade_evidence_manifest(manifest.canonical_bytes().to_vec())
            .expect("manifest projection");
        assert_eq!(projected.coverage, TradeEvidenceCoverage::ScopeSatisfied);
        assert_eq!(
            projected.canonical_bytes_hex,
            hex::encode(manifest.canonical_bytes())
        );
        assert!(parse_trade_evidence_manifest(Vec::new()).is_err());

        let decisions: serde_json::Value = serde_json::from_str(include_str!(
            "../../../contracts/conformance/vectors/rhi/evidence_attestation_decision.v1.json"
        ))
        .expect("RHI decision corpus");
        let superseding = decisions["vectors"]
            .as_array()
            .expect("RHI decision vectors")
            .iter()
            .find(|entry| entry["id"] == "rhi_evidence_attestation_superseding_002")
            .expect("superseding vector");
        let report = parse_rhi_evidence_report(
            superseding["expected"]["canonical_event_content_utf8"]
                .as_str()
                .expect("canonical event content")
                .to_owned(),
        )
        .expect("superseding report");
        assert_eq!(report.outcome, TradeEvidenceOutcome::Valid);
        let supersession = report.supersession.expect("supersession");
        assert_eq!(
            supersession.report_id,
            "7777777777777777777777777777777777777777777777777777777777777777"
        );
        assert_eq!(
            supersession.event_id,
            "8888888888888888888888888888888888888888888888888888888888888888"
        );
    }

    #[test]
    fn evidence_entry_points_classify_pre_plan_and_signed_event_failures() {
        assert!(prepare_rhi_evidence_attestation("invalid".to_owned(), 0).is_err());
        assert!(
            validate_rhi_evidence_attestation(SignedEvent {
                id: String::new(),
                author_pubkey: String::new(),
                created_at_unix_s: 0,
                kind: 0,
                tags: Vec::new(),
                content: String::new(),
                signature: String::new(),
            })
            .is_err()
        );
    }

    #[test]
    fn ffi_contract_errors_are_bounded_and_secret_safe() {
        let secret = "private-report-content";
        let error = parse_rhi_evidence_report(secret.to_owned()).expect_err("malformed report");
        let display = error.to_string();
        let debug = format!("{error:?}");
        assert!(!display.contains(secret));
        assert!(!debug.contains(secret));
        assert!(display.contains("invalid_evidence_report"));
    }
}
