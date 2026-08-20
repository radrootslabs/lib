#![forbid(unsafe_code)]

//! Exact RHI evidence-attestation wire construction and validation.

#[cfg(not(feature = "std"))]
use alloc::{
    boxed::Box,
    format,
    string::{String, ToString},
    vec,
    vec::Vec,
};
use core::{cmp::Ordering, fmt, num::NonZeroU64};
#[cfg(feature = "std")]
use std::{
    boxed::Box,
    format,
    string::{String, ToString},
    vec,
    vec::Vec,
};

use radroots_event::{
    admission::SignatureVerifiedEvent,
    envelope::EventEnvelope,
    envelope::kind::KIND_RHI_EVIDENCE_ATTESTATION,
    id::{EventId, MutationId, TradeId},
    trade::canonical_jcs_value,
    wire::Nip01EventWireParts,
};
use radroots_identity::PublicKey;
use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest as _, Sha256};

pub const RADROOTS_RHI_EVIDENCE_ATTESTATION_CONTRACT_ID: &str =
    "radroots.rhi.evidence_attestation.v1";
pub const RADROOTS_RHI_EVIDENCE_ATTESTATION_MAXIMUM_BYTES: usize = 16 * 1024;
const MAXIMUM_TAGS: usize = 7;
const MAXIMUM_REASON_CODES: usize = 16;
const MAXIMUM_REASON_CODE_BYTES: usize = 64;
const STATEMENT_DIGEST_DOMAIN: &[u8] = b"radroots:rhi-evidence-attestation-statement:v1\0";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadrootsRhiEvidenceAttestationOutcomeV1 {
    Valid,
    Invalid,
    Indeterminate,
}

impl RadrootsRhiEvidenceAttestationOutcomeV1 {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Valid => "valid",
            Self::Invalid => "invalid",
            Self::Indeterminate => "indeterminate",
        }
    }

    fn topic(self) -> String {
        format!("radroots:rhi-outcome:{}", self.as_str())
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct RadrootsRhiEvidenceAttestationSupersessionV1 {
    report_id: [u8; 32],
    event_id: EventId,
}

impl RadrootsRhiEvidenceAttestationSupersessionV1 {
    #[must_use]
    pub const fn report_id(&self) -> &[u8; 32] {
        &self.report_id
    }

    #[must_use]
    pub const fn event_id(&self) -> &EventId {
        &self.event_id
    }
}

impl fmt::Debug for RadrootsRhiEvidenceAttestationSupersessionV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("RadrootsRhiEvidenceAttestationSupersessionV1(<redacted>)")
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct RadrootsRhiEvidenceAttestationV1 {
    issuer: PublicKey,
    trade_id: TradeId,
    claim_mutation_id: MutationId,
    outcome: RadrootsRhiEvidenceAttestationOutcomeV1,
    observed_at_unix_s: u64,
    trade_generation: NonZeroU64,
    statement_digest: [u8; 32],
    supersession: Option<RadrootsRhiEvidenceAttestationSupersessionV1>,
    canonical_content: Box<str>,
}

impl RadrootsRhiEvidenceAttestationV1 {
    pub fn from_canonical_content(
        content: impl AsRef<[u8]>,
    ) -> Result<Self, RadrootsRhiEvidenceAttestationError> {
        let content = content.as_ref();
        if content.is_empty() || content.len() > RADROOTS_RHI_EVIDENCE_ATTESTATION_MAXIMUM_BYTES {
            return Err(RadrootsRhiEvidenceAttestationError::NoncanonicalReportContent);
        }
        let raw: RawReport = serde_json::from_slice(content)
            .map_err(|_| RadrootsRhiEvidenceAttestationError::NoncanonicalReportContent)?;
        validate_fixed_fields(&raw)?;
        validate_reason_codes(&raw.reason_codes)?;

        let value: Value = serde_json::from_slice(content)
            .map_err(|_| RadrootsRhiEvidenceAttestationError::NoncanonicalReportContent)?;
        let object = value
            .as_object()
            .ok_or(RadrootsRhiEvidenceAttestationError::NoncanonicalReportContent)?;
        if !object.contains_key("supersedes_report_id")
            || !object.contains_key("supersedes_event_id")
        {
            return Err(RadrootsRhiEvidenceAttestationError::NoncanonicalReportContent);
        }

        let issuer = canonical_public_key(&raw.issuer_pubkey)?;
        let trade_id = canonical_trade_id(&raw.trade_id)?;
        let claim_mutation_id = canonical_mutation_id(&raw.claim_mutation_id)?;
        let outcome = parse_outcome(&raw.outcome)?;
        let trade_generation = NonZeroU64::new(raw.trade_generation)
            .ok_or(RadrootsRhiEvidenceAttestationError::InvalidReport)?;
        let report_id = parse_hex_32(&raw.report_id)?;
        let declared_statement_digest = parse_hex_32(&raw.statement_digest)?;
        if report_id != declared_statement_digest {
            return Err(RadrootsRhiEvidenceAttestationError::StatementDigestMismatch);
        }
        for digest in [
            &raw.projection_digest,
            &raw.evidence_manifest_digest,
            &raw.evidence_policy_digest,
        ] {
            parse_hex_32(digest)?;
        }
        let supersession = parse_supersession(
            raw.supersedes_report_id.as_deref(),
            raw.supersedes_event_id.as_deref(),
        )?;

        let canonical_content = canonical_jcs_value(&value)
            .map_err(|_| RadrootsRhiEvidenceAttestationError::NoncanonicalReportContent)?;
        if canonical_content.as_bytes() != content {
            return Err(RadrootsRhiEvidenceAttestationError::NoncanonicalReportContent);
        }
        let mut statement = value;
        let object = statement
            .as_object_mut()
            .ok_or(RadrootsRhiEvidenceAttestationError::NoncanonicalReportContent)?;
        object.remove("report_id");
        object.remove("statement_digest");
        let statement_payload = canonical_jcs_value(&statement)
            .map_err(|_| RadrootsRhiEvidenceAttestationError::NoncanonicalReportContent)?;
        let mut hasher = Sha256::new();
        hasher.update(STATEMENT_DIGEST_DOMAIN);
        hasher.update(statement_payload.as_bytes());
        let computed_statement_digest: [u8; 32] = hasher.finalize().into();
        if computed_statement_digest != declared_statement_digest {
            return Err(RadrootsRhiEvidenceAttestationError::StatementDigestMismatch);
        }

        Ok(Self {
            issuer,
            trade_id,
            claim_mutation_id,
            outcome,
            observed_at_unix_s: raw.observed_at_unix_s,
            trade_generation,
            statement_digest: declared_statement_digest,
            supersession,
            canonical_content: canonical_content.into_boxed_str(),
        })
    }

    #[must_use]
    pub const fn issuer(&self) -> &PublicKey {
        &self.issuer
    }

    #[must_use]
    pub const fn trade_id(&self) -> &TradeId {
        &self.trade_id
    }

    #[must_use]
    pub const fn claim_mutation_id(&self) -> &MutationId {
        &self.claim_mutation_id
    }

    #[must_use]
    pub const fn outcome(&self) -> RadrootsRhiEvidenceAttestationOutcomeV1 {
        self.outcome
    }

    #[must_use]
    pub const fn observed_at_unix_s(&self) -> u64 {
        self.observed_at_unix_s
    }

    #[must_use]
    pub const fn trade_generation(&self) -> NonZeroU64 {
        self.trade_generation
    }

    #[must_use]
    pub const fn statement_digest(&self) -> &[u8; 32] {
        &self.statement_digest
    }

    #[must_use]
    pub const fn supersession(&self) -> Option<RadrootsRhiEvidenceAttestationSupersessionV1> {
        self.supersession
    }

    #[must_use]
    pub fn canonical_content(&self) -> &str {
        &self.canonical_content
    }
}

impl fmt::Debug for RadrootsRhiEvidenceAttestationV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RadrootsRhiEvidenceAttestationV1")
            .field("outcome", &self.outcome)
            .field("has_supersession", &self.supersession.is_some())
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadrootsRhiEvidenceAttestationError {
    InvalidAttestationKind,
    IssuerAuthorMismatch,
    NoncanonicalReportContent,
    StatementDigestMismatch,
    InvalidOutcome,
    MissingClaimTag,
    DuplicateTradeTag,
    DuplicateStatementTag,
    IncompleteSupersessionReference,
    StaleTradeGeneration,
    CallerStructuralTagForbidden,
    InvalidIdentifier,
    InvalidReport,
    InvalidTagShape,
    UnexpectedTag,
    ContractTagMismatch,
    TradeTagMismatch,
    ClaimTagMismatch,
    StatementTagMismatch,
    OutcomeTagMismatch,
    SupersessionTagMismatch,
}

impl RadrootsRhiEvidenceAttestationError {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidAttestationKind => "invalid_attestation_kind",
            Self::IssuerAuthorMismatch => "issuer_author_mismatch",
            Self::NoncanonicalReportContent => "noncanonical_report_content",
            Self::StatementDigestMismatch => "statement_digest_mismatch",
            Self::InvalidOutcome => "invalid_outcome",
            Self::MissingClaimTag => "missing_claim_tag",
            Self::DuplicateTradeTag => "duplicate_trade_tag",
            Self::DuplicateStatementTag => "duplicate_statement_tag",
            Self::IncompleteSupersessionReference => "incomplete_supersession_reference",
            Self::StaleTradeGeneration => "stale_trade_generation",
            Self::CallerStructuralTagForbidden => "caller_structural_tag_forbidden",
            Self::InvalidIdentifier => "invalid_identifier",
            Self::InvalidReport => "invalid_report",
            Self::InvalidTagShape => "invalid_tag_shape",
            Self::UnexpectedTag => "unexpected_tag",
            Self::ContractTagMismatch => "contract_tag_mismatch",
            Self::TradeTagMismatch => "trade_tag_mismatch",
            Self::ClaimTagMismatch => "claim_tag_mismatch",
            Self::StatementTagMismatch => "statement_tag_mismatch",
            Self::OutcomeTagMismatch => "outcome_tag_mismatch",
            Self::SupersessionTagMismatch => "supersession_tag_mismatch",
        }
    }
}

impl fmt::Display for RadrootsRhiEvidenceAttestationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidAttestationKind => "RHI attestation kind is invalid",
            Self::IssuerAuthorMismatch => "RHI attestation issuer does not match author",
            Self::NoncanonicalReportContent => "RHI attestation report is not canonical",
            Self::StatementDigestMismatch => "RHI attestation statement digest does not match",
            Self::InvalidOutcome => "RHI attestation outcome is invalid",
            Self::MissingClaimTag => "RHI attestation claim tag is missing",
            Self::DuplicateTradeTag => "RHI attestation trade tag is duplicated",
            Self::DuplicateStatementTag => "RHI attestation statement tag is duplicated",
            Self::IncompleteSupersessionReference => "RHI attestation supersession is incomplete",
            Self::StaleTradeGeneration => "RHI attestation supersession is stale",
            Self::CallerStructuralTagForbidden => "caller supplied a governed RHI attestation tag",
            Self::InvalidIdentifier => "RHI attestation identifier is invalid",
            Self::InvalidReport => "RHI attestation report is invalid",
            Self::InvalidTagShape => "RHI attestation tag shape is invalid",
            Self::UnexpectedTag => "RHI attestation tag is not permitted",
            Self::ContractTagMismatch => "RHI attestation contract tag does not match",
            Self::TradeTagMismatch => "RHI attestation trade tag does not match",
            Self::ClaimTagMismatch => "RHI attestation claim tag does not match",
            Self::StatementTagMismatch => "RHI attestation statement tag does not match",
            Self::OutcomeTagMismatch => "RHI attestation outcome tag does not match",
            Self::SupersessionTagMismatch => "RHI attestation supersession tag does not match",
        })
    }
}

#[cfg(feature = "std")]
impl std::error::Error for RadrootsRhiEvidenceAttestationError {}

pub fn rhi_evidence_attestation_event_build(
    attestation: &RadrootsRhiEvidenceAttestationV1,
) -> Nip01EventWireParts {
    Nip01EventWireParts {
        kind: KIND_RHI_EVIDENCE_ATTESTATION,
        tags: canonical_tags(attestation),
        content: attestation.canonical_content().to_string(),
    }
}

pub fn rhi_evidence_attestation_event_build_with_extra_tags(
    attestation: &RadrootsRhiEvidenceAttestationV1,
    extra_tags: &[Vec<String>],
) -> Result<Nip01EventWireParts, RadrootsRhiEvidenceAttestationError> {
    if extra_tags.iter().any(|tag| {
        matches!(
            tag.first().map(String::as_str),
            Some("contract" | "d" | "x" | "t" | "e")
        )
    }) {
        return Err(RadrootsRhiEvidenceAttestationError::CallerStructuralTagForbidden);
    }
    if !extra_tags.is_empty() {
        return Err(RadrootsRhiEvidenceAttestationError::UnexpectedTag);
    }
    Ok(rhi_evidence_attestation_event_build(attestation))
}

/// Structurally parses an RHI attestation without claiming signature proof.
pub fn rhi_evidence_attestation_from_event(
    event: &EventEnvelope,
) -> Result<RadrootsRhiEvidenceAttestationV1, RadrootsRhiEvidenceAttestationError> {
    validate_parts(
        event.kind_u32(),
        &event.author().to_hex(),
        &event.tags_as_vec(),
        event.content(),
    )
}

/// Validates an RHI attestation whose NIP-01 signature is already verified.
pub fn rhi_evidence_attestation_from_verified_event(
    event: &SignatureVerifiedEvent,
) -> Result<RadrootsRhiEvidenceAttestationV1, RadrootsRhiEvidenceAttestationError> {
    rhi_evidence_attestation_from_event(event.event())
}

pub fn validate_rhi_evidence_attestation_tags(
    attestation: &RadrootsRhiEvidenceAttestationV1,
    tags: &[Vec<String>],
) -> Result<(), RadrootsRhiEvidenceAttestationError> {
    if tags.len() > MAXIMUM_TAGS {
        return Err(RadrootsRhiEvidenceAttestationError::InvalidTagShape);
    }
    let trade_count = count_unmarked(tags, "d");
    if trade_count > 1 {
        return Err(RadrootsRhiEvidenceAttestationError::DuplicateTradeTag);
    }
    let statement_count = count_marked(tags, "x", "statement");
    if statement_count > 1 {
        return Err(RadrootsRhiEvidenceAttestationError::DuplicateStatementTag);
    }
    let claim_count = count_marked(tags, "x", "claim");
    if claim_count == 0 {
        return Err(RadrootsRhiEvidenceAttestationError::MissingClaimTag);
    }
    let report_count = count_marked(tags, "x", "supersedes_report");
    let event_count = count_unmarked(tags, "e");
    if report_count != event_count {
        return Err(RadrootsRhiEvidenceAttestationError::IncompleteSupersessionReference);
    }
    if trade_count != 1
        || statement_count != 1
        || claim_count != 1
        || report_count > 1
        || count_unmarked(tags, "contract") != 1
        || count_unmarked(tags, "t") != 1
        || count_named(tags, "x") != claim_count + statement_count + report_count
    {
        return Err(RadrootsRhiEvidenceAttestationError::InvalidTagShape);
    }
    if tags.iter().any(|tag| {
        !matches!(
            tag.first().map(String::as_str),
            Some("contract" | "d" | "x" | "t" | "e")
        )
    }) {
        return Err(RadrootsRhiEvidenceAttestationError::UnexpectedTag);
    }

    if exact_unmarked(tags.first(), "contract")? != RADROOTS_RHI_EVIDENCE_ATTESTATION_CONTRACT_ID {
        return Err(RadrootsRhiEvidenceAttestationError::ContractTagMismatch);
    }
    if canonical_trade_id(exact_unmarked(tags.get(1), "d")?)? != attestation.trade_id {
        return Err(RadrootsRhiEvidenceAttestationError::TradeTagMismatch);
    }
    if canonical_mutation_id(exact_marked(tags.get(2), "x", "claim")?)?
        != attestation.claim_mutation_id
    {
        return Err(RadrootsRhiEvidenceAttestationError::ClaimTagMismatch);
    }
    if parse_hex_32(exact_marked(tags.get(3), "x", "statement")?)? != attestation.statement_digest {
        return Err(RadrootsRhiEvidenceAttestationError::StatementTagMismatch);
    }
    if exact_unmarked(tags.get(4), "t")? != attestation.outcome.topic() {
        return Err(RadrootsRhiEvidenceAttestationError::OutcomeTagMismatch);
    }
    match attestation.supersession {
        None if tags.len() == 5 => Ok(()),
        Some(supersession) if tags.len() == 7 => {
            if parse_hex_32(exact_marked(tags.get(5), "x", "supersedes_report")?)?
                != supersession.report_id
                || canonical_event_id(exact_unmarked(tags.get(6), "e")?)? != supersession.event_id
            {
                return Err(RadrootsRhiEvidenceAttestationError::SupersessionTagMismatch);
            }
            Ok(())
        }
        _ => Err(RadrootsRhiEvidenceAttestationError::IncompleteSupersessionReference),
    }
}

pub fn validate_rhi_evidence_attestation_supersession(
    current: &RadrootsRhiEvidenceAttestationV1,
    current_event_id: &EventId,
    candidate: &RadrootsRhiEvidenceAttestationV1,
) -> Result<(), RadrootsRhiEvidenceAttestationError> {
    if current.trade_id != candidate.trade_id {
        return Err(RadrootsRhiEvidenceAttestationError::SupersessionTagMismatch);
    }
    let Some(supersession) = candidate.supersession else {
        return Err(RadrootsRhiEvidenceAttestationError::IncompleteSupersessionReference);
    };
    if supersession.report_id != current.statement_digest
        || supersession.event_id != *current_event_id
    {
        return Err(RadrootsRhiEvidenceAttestationError::SupersessionTagMismatch);
    }
    let ordering = candidate
        .trade_generation
        .cmp(&current.trade_generation)
        .then_with(|| {
            candidate
                .observed_at_unix_s
                .cmp(&current.observed_at_unix_s)
        })
        .then_with(|| candidate.statement_digest.cmp(&current.statement_digest));
    if ordering != Ordering::Greater {
        return Err(RadrootsRhiEvidenceAttestationError::StaleTradeGeneration);
    }
    Ok(())
}

pub(crate) fn validate_parts(
    kind: u32,
    author: &str,
    tags: &[Vec<String>],
    content: &str,
) -> Result<RadrootsRhiEvidenceAttestationV1, RadrootsRhiEvidenceAttestationError> {
    if kind != KIND_RHI_EVIDENCE_ATTESTATION {
        return Err(RadrootsRhiEvidenceAttestationError::InvalidAttestationKind);
    }
    let attestation = RadrootsRhiEvidenceAttestationV1::from_canonical_content(content)?;
    if canonical_public_key(author)? != attestation.issuer {
        return Err(RadrootsRhiEvidenceAttestationError::IssuerAuthorMismatch);
    }
    validate_rhi_evidence_attestation_tags(&attestation, tags)?;
    Ok(attestation)
}

fn canonical_tags(attestation: &RadrootsRhiEvidenceAttestationV1) -> Vec<Vec<String>> {
    let mut tags = Vec::with_capacity(if attestation.supersession.is_some() {
        7
    } else {
        5
    });
    tags.push(vec![
        "contract".to_string(),
        RADROOTS_RHI_EVIDENCE_ATTESTATION_CONTRACT_ID.to_string(),
    ]);
    tags.push(vec!["d".to_string(), attestation.trade_id.to_hex()]);
    tags.push(vec![
        "x".to_string(),
        attestation.claim_mutation_id.to_hex(),
        "claim".to_string(),
    ]);
    tags.push(vec![
        "x".to_string(),
        hex::encode(attestation.statement_digest),
        "statement".to_string(),
    ]);
    tags.push(vec!["t".to_string(), attestation.outcome.topic()]);
    if let Some(supersession) = attestation.supersession {
        tags.push(vec![
            "x".to_string(),
            hex::encode(supersession.report_id),
            "supersedes_report".to_string(),
        ]);
        tags.push(vec!["e".to_string(), supersession.event_id.to_hex()]);
    }
    tags
}

fn validate_fixed_fields(raw: &RawReport) -> Result<(), RadrootsRhiEvidenceAttestationError> {
    if raw.contract_id != RADROOTS_RHI_EVIDENCE_ATTESTATION_CONTRACT_ID
        || raw.contract_version != 1
        || raw.reducer_contract_id != "radroots.trade.reducer.v1"
        || raw.reducer_contract_version != 1
        || raw.attestation_method != "signed_evidence_snapshot"
    {
        return Err(RadrootsRhiEvidenceAttestationError::InvalidReport);
    }
    Ok(())
}

fn validate_reason_codes(codes: &[String]) -> Result<(), RadrootsRhiEvidenceAttestationError> {
    if codes.is_empty() || codes.len() > MAXIMUM_REASON_CODES {
        return Err(RadrootsRhiEvidenceAttestationError::InvalidReport);
    }
    let mut previous: Option<&str> = None;
    for code in codes {
        let bytes = code.as_bytes();
        if bytes.is_empty()
            || bytes.len() > MAXIMUM_REASON_CODE_BYTES
            || !bytes[0].is_ascii_lowercase()
            || !bytes[bytes.len() - 1].is_ascii_alphanumeric()
            || bytes
                .iter()
                .any(|byte| !(byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'_'))
            || previous.is_some_and(|value| value >= code.as_str())
        {
            return Err(RadrootsRhiEvidenceAttestationError::InvalidReport);
        }
        previous = Some(code);
    }
    Ok(())
}

fn parse_outcome(
    outcome: &str,
) -> Result<RadrootsRhiEvidenceAttestationOutcomeV1, RadrootsRhiEvidenceAttestationError> {
    match outcome {
        "valid" => Ok(RadrootsRhiEvidenceAttestationOutcomeV1::Valid),
        "invalid" => Ok(RadrootsRhiEvidenceAttestationOutcomeV1::Invalid),
        "indeterminate" => Ok(RadrootsRhiEvidenceAttestationOutcomeV1::Indeterminate),
        _ => Err(RadrootsRhiEvidenceAttestationError::InvalidOutcome),
    }
}

fn parse_supersession(
    report: Option<&str>,
    event: Option<&str>,
) -> Result<Option<RadrootsRhiEvidenceAttestationSupersessionV1>, RadrootsRhiEvidenceAttestationError>
{
    match (report, event) {
        (None, None) => Ok(None),
        (Some(report), Some(event)) => Ok(Some(RadrootsRhiEvidenceAttestationSupersessionV1 {
            report_id: parse_hex_32(report)?,
            event_id: canonical_event_id(event)?,
        })),
        _ => Err(RadrootsRhiEvidenceAttestationError::IncompleteSupersessionReference),
    }
}

fn canonical_public_key(value: &str) -> Result<PublicKey, RadrootsRhiEvidenceAttestationError> {
    let key = PublicKey::from_hex(value)
        .map_err(|_| RadrootsRhiEvidenceAttestationError::InvalidIdentifier)?;
    if key.to_hex() != value {
        return Err(RadrootsRhiEvidenceAttestationError::InvalidIdentifier);
    }
    Ok(key)
}

fn canonical_trade_id(value: &str) -> Result<TradeId, RadrootsRhiEvidenceAttestationError> {
    let id = TradeId::parse(value)
        .map_err(|_| RadrootsRhiEvidenceAttestationError::InvalidIdentifier)?;
    if id.to_hex() != value {
        return Err(RadrootsRhiEvidenceAttestationError::InvalidIdentifier);
    }
    Ok(id)
}

fn canonical_mutation_id(value: &str) -> Result<MutationId, RadrootsRhiEvidenceAttestationError> {
    let id = MutationId::parse(value)
        .map_err(|_| RadrootsRhiEvidenceAttestationError::InvalidIdentifier)?;
    if id.to_hex() != value {
        return Err(RadrootsRhiEvidenceAttestationError::InvalidIdentifier);
    }
    Ok(id)
}

fn canonical_event_id(value: &str) -> Result<EventId, RadrootsRhiEvidenceAttestationError> {
    let id = EventId::parse(value)
        .map_err(|_| RadrootsRhiEvidenceAttestationError::InvalidIdentifier)?;
    if id.to_hex() != value {
        return Err(RadrootsRhiEvidenceAttestationError::InvalidIdentifier);
    }
    Ok(id)
}

fn parse_hex_32(value: &str) -> Result<[u8; 32], RadrootsRhiEvidenceAttestationError> {
    if value.len() != 64
        || value
            .bytes()
            .any(|byte| !(byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)))
    {
        return Err(RadrootsRhiEvidenceAttestationError::InvalidIdentifier);
    }
    let mut bytes = [0_u8; 32];
    hex::decode_to_slice(value, &mut bytes)
        .map_err(|_| RadrootsRhiEvidenceAttestationError::InvalidIdentifier)?;
    Ok(bytes)
}

fn count_named(tags: &[Vec<String>], name: &str) -> usize {
    tags.iter()
        .filter(|tag| tag.first().map(String::as_str) == Some(name))
        .count()
}

fn count_unmarked(tags: &[Vec<String>], name: &str) -> usize {
    tags.iter()
        .filter(|tag| tag.len() == 2 && tag.first().map(String::as_str) == Some(name))
        .count()
}

fn count_marked(tags: &[Vec<String>], name: &str, marker: &str) -> usize {
    tags.iter()
        .filter(|tag| {
            tag.len() == 3
                && tag.first().map(String::as_str) == Some(name)
                && tag.get(2).map(String::as_str) == Some(marker)
        })
        .count()
}

fn exact_unmarked<'a>(
    tag: Option<&'a Vec<String>>,
    name: &str,
) -> Result<&'a str, RadrootsRhiEvidenceAttestationError> {
    let tag = tag.ok_or(RadrootsRhiEvidenceAttestationError::InvalidTagShape)?;
    if tag.len() != 2 || tag.first().map(String::as_str) != Some(name) {
        return Err(RadrootsRhiEvidenceAttestationError::InvalidTagShape);
    }
    Ok(&tag[1])
}

fn exact_marked<'a>(
    tag: Option<&'a Vec<String>>,
    name: &str,
    marker: &str,
) -> Result<&'a str, RadrootsRhiEvidenceAttestationError> {
    let tag = tag.ok_or(RadrootsRhiEvidenceAttestationError::InvalidTagShape)?;
    if tag.len() != 3
        || tag.first().map(String::as_str) != Some(name)
        || tag.get(2).map(String::as_str) != Some(marker)
    {
        return Err(RadrootsRhiEvidenceAttestationError::InvalidTagShape);
    }
    Ok(&tag[1])
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawReport {
    attestation_method: String,
    claim_mutation_id: String,
    contract_id: String,
    contract_version: u16,
    evidence_manifest_digest: String,
    evidence_policy_digest: String,
    issuer_pubkey: String,
    observed_at_unix_s: u64,
    outcome: String,
    projection_digest: String,
    reason_codes: Vec<String>,
    reducer_contract_id: String,
    reducer_contract_version: u16,
    report_id: String,
    statement_digest: String,
    supersedes_event_id: Option<String>,
    supersedes_report_id: Option<String>,
    trade_generation: u64,
    trade_id: String,
}
