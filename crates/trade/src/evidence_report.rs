#![forbid(unsafe_code)]

//! Immutable RHI evidence reports derived from governed trade evidence.
//!
//! Reports bind a separately retained evidence manifest and projection. They
//! do not retrieve evidence, validate signatures, build events, persist state,
//! or publish anything.

use alloc::{boxed::Box, string::String, vec::Vec};
use core::{fmt, num::NonZeroU64};

use radroots_event::id::{EventId, MutationId, TradeId};
use radroots_identity::PublicKey;
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};

use crate::{
    evidence::RadrootsTradeEvidenceOutcomeV1,
    evidence_manifest::{
        RadrootsTradeEvidenceManifestDigestV1, RadrootsTradeEvidenceManifestV1,
        RadrootsTradeEvidencePolicyDigestV1,
    },
    trade_contract_v1::{RADROOTS_TRADE_REDUCER_CONTRACT_ID, RADROOTS_TRADE_REDUCER_VERSION},
};

pub const RADROOTS_RHI_EVIDENCE_REPORT_CONTRACT_ID: &str = "radroots.rhi.evidence_attestation.v1";
pub const RADROOTS_RHI_EVIDENCE_REPORT_CONTRACT_VERSION: u16 = 1;
pub const RADROOTS_RHI_EVIDENCE_ATTESTATION_METHOD: &str = "signed_evidence_snapshot";
pub const RADROOTS_RHI_EVIDENCE_REPORT_MAXIMUM_REASON_CODES: usize = 16;
pub const RADROOTS_RHI_EVIDENCE_REPORT_REASON_CODE_MAXIMUM_BYTES: usize = 64;
pub const RADROOTS_RHI_EVIDENCE_REPORT_MAXIMUM_CANONICAL_BYTES: usize = 16 * 1024;

const STATEMENT_DIGEST_DOMAIN: &[u8] = b"radroots:rhi-evidence-attestation-statement:v1\0";

macro_rules! define_digest {
    ($name:ident) => {
        #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name([u8; 32]);

        impl $name {
            pub const fn from_bytes(bytes: [u8; 32]) -> Self {
                Self(bytes)
            }

            pub fn sha256(bytes: &[u8]) -> Self {
                Self(Sha256::digest(bytes).into())
            }

            pub const fn as_bytes(&self) -> &[u8; 32] {
                &self.0
            }

            pub fn to_hex(&self) -> String {
                hex::encode(self.0)
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(concat!(stringify!($name), "(<redacted>)"))
            }
        }
    };
}

define_digest!(RadrootsTradeEvidenceProjectionDigestV1);

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RadrootsRhiEvidenceStatementDigestV1([u8; 32]);

impl RadrootsRhiEvidenceStatementDigestV1 {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub fn to_hex(&self) -> String {
        hex::encode(self.0)
    }
}

impl fmt::Debug for RadrootsRhiEvidenceStatementDigestV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("RadrootsRhiEvidenceStatementDigestV1(<redacted>)")
    }
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RadrootsRhiEvidenceReasonCodeV1(String);

impl RadrootsRhiEvidenceReasonCodeV1 {
    pub fn parse(value: impl AsRef<str>) -> Result<Self, RadrootsRhiEvidenceReportError> {
        let value = value.as_ref();
        let bytes = value.as_bytes();
        if bytes.is_empty()
            || bytes.len() > RADROOTS_RHI_EVIDENCE_REPORT_REASON_CODE_MAXIMUM_BYTES
            || !bytes[0].is_ascii_lowercase()
            || !bytes[bytes.len() - 1].is_ascii_alphanumeric()
            || bytes
                .iter()
                .any(|byte| !(byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'_'))
        {
            return Err(RadrootsRhiEvidenceReportError::InvalidReasonCode);
        }
        Ok(Self(String::from(value)))
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Debug for RadrootsRhiEvidenceReasonCodeV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("RadrootsRhiEvidenceReasonCodeV1(<redacted>)")
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct RadrootsRhiEvidenceSupersessionV1 {
    report_id: RadrootsRhiEvidenceStatementDigestV1,
    event_id: EventId,
}

impl RadrootsRhiEvidenceSupersessionV1 {
    pub const fn new(report_id: RadrootsRhiEvidenceStatementDigestV1, event_id: EventId) -> Self {
        Self {
            report_id,
            event_id,
        }
    }

    pub const fn report_id(&self) -> RadrootsRhiEvidenceStatementDigestV1 {
        self.report_id
    }

    pub const fn event_id(&self) -> EventId {
        self.event_id
    }
}

impl fmt::Debug for RadrootsRhiEvidenceSupersessionV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("RadrootsRhiEvidenceSupersessionV1(<redacted>)")
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct RadrootsRhiEvidenceReportV1 {
    issuer_public_key: PublicKey,
    trade_id: TradeId,
    claim_mutation_id: MutationId,
    outcome: RadrootsTradeEvidenceOutcomeV1,
    reason_codes: Box<[RadrootsRhiEvidenceReasonCodeV1]>,
    projection_digest: RadrootsTradeEvidenceProjectionDigestV1,
    evidence_manifest_digest: RadrootsTradeEvidenceManifestDigestV1,
    evidence_policy_digest: RadrootsTradeEvidencePolicyDigestV1,
    observed_at_unix_s: u64,
    supersession: Option<RadrootsRhiEvidenceSupersessionV1>,
    trade_generation: NonZeroU64,
    statement_digest: RadrootsRhiEvidenceStatementDigestV1,
    canonical_statement_payload: Box<str>,
    canonical_content: Box<str>,
}

impl RadrootsRhiEvidenceReportV1 {
    pub fn new<R>(
        issuer_public_key: PublicKey,
        claim_mutation_id: MutationId,
        outcome: RadrootsTradeEvidenceOutcomeV1,
        reason_codes: R,
        projection_digest: RadrootsTradeEvidenceProjectionDigestV1,
        manifest: &RadrootsTradeEvidenceManifestV1,
        supersession: Option<RadrootsRhiEvidenceSupersessionV1>,
    ) -> Result<Self, RadrootsRhiEvidenceReportError>
    where
        R: IntoIterator<Item = RadrootsRhiEvidenceReasonCodeV1>,
    {
        if !manifest.coverage().permits(outcome) {
            return Err(RadrootsRhiEvidenceReportError::OutcomeNotPermitted);
        }
        let reason_codes = normalize_reason_codes(reason_codes)?;
        Self::from_fields(ReportFields {
            issuer_public_key,
            trade_id: *manifest.trade_id(),
            claim_mutation_id,
            outcome,
            reason_codes,
            projection_digest,
            evidence_manifest_digest: manifest.digest(),
            evidence_policy_digest: manifest.evidence_policy_digest(),
            observed_at_unix_s: manifest.observed_at_unix_s(),
            supersession,
            trade_generation: manifest.trade_generation(),
        })
    }

    pub fn from_canonical_content(content: &[u8]) -> Result<Self, RadrootsRhiEvidenceReportError> {
        if content.is_empty() {
            return Err(RadrootsRhiEvidenceReportError::Malformed);
        }
        if content.len() > RADROOTS_RHI_EVIDENCE_REPORT_MAXIMUM_CANONICAL_BYTES {
            return Err(RadrootsRhiEvidenceReportError::ReportTooLarge);
        }
        let raw: RawReport = serde_json::from_slice(content)
            .map_err(|_| RadrootsRhiEvidenceReportError::Malformed)?;
        if raw.contract_version != RADROOTS_RHI_EVIDENCE_REPORT_CONTRACT_VERSION {
            return Err(RadrootsRhiEvidenceReportError::UnsupportedVersion);
        }
        if raw.contract_id != RADROOTS_RHI_EVIDENCE_REPORT_CONTRACT_ID
            || raw.reducer_contract_id != RADROOTS_TRADE_REDUCER_CONTRACT_ID
            || raw.reducer_contract_version != RADROOTS_TRADE_REDUCER_VERSION
            || raw.attestation_method != RADROOTS_RHI_EVIDENCE_ATTESTATION_METHOD
        {
            return Err(RadrootsRhiEvidenceReportError::FixedFieldMismatch);
        }

        let report_id = parse_digest(&raw.report_id)?;
        let statement_digest = parse_digest(&raw.statement_digest)?;
        if report_id != statement_digest {
            return Err(RadrootsRhiEvidenceReportError::StatementDigestMismatch);
        }
        let supersession = parse_supersession(
            raw.supersedes_report_id.as_deref(),
            raw.supersedes_event_id.as_deref(),
        )?;
        let reason_codes = parse_reason_codes(raw.reason_codes)?;
        let fields = ReportFields {
            issuer_public_key: PublicKey::from_hex(&raw.issuer_pubkey)
                .map_err(|_| RadrootsRhiEvidenceReportError::InvalidIdentifier)?,
            trade_id: TradeId::parse(&raw.trade_id)
                .map_err(|_| RadrootsRhiEvidenceReportError::InvalidIdentifier)?,
            claim_mutation_id: MutationId::parse(&raw.claim_mutation_id)
                .map_err(|_| RadrootsRhiEvidenceReportError::InvalidIdentifier)?,
            outcome: parse_outcome(&raw.outcome)?,
            reason_codes,
            projection_digest: RadrootsTradeEvidenceProjectionDigestV1::from_bytes(parse_hex_32(
                &raw.projection_digest,
            )?),
            evidence_manifest_digest: RadrootsTradeEvidenceManifestDigestV1::from_bytes(
                parse_hex_32(&raw.evidence_manifest_digest)?,
            ),
            evidence_policy_digest: RadrootsTradeEvidencePolicyDigestV1::from_bytes(parse_hex_32(
                &raw.evidence_policy_digest,
            )?),
            observed_at_unix_s: raw.observed_at_unix_s,
            supersession,
            trade_generation: NonZeroU64::new(raw.trade_generation)
                .ok_or(RadrootsRhiEvidenceReportError::InvalidTradeGeneration)?,
        };
        let report = Self::from_fields(fields)?;
        if report.statement_digest != statement_digest {
            return Err(RadrootsRhiEvidenceReportError::StatementDigestMismatch);
        }
        if report.canonical_content.as_bytes() != content {
            return Err(RadrootsRhiEvidenceReportError::NonCanonical);
        }
        Ok(report)
    }

    fn from_fields(fields: ReportFields) -> Result<Self, RadrootsRhiEvidenceReportError> {
        let canonical_statement_payload = canonical_statement_payload(&fields)?;
        let statement_digest = statement_digest(canonical_statement_payload.as_bytes());
        let canonical_content = canonical_report_content(&fields, statement_digest)?;
        if canonical_content.len() > RADROOTS_RHI_EVIDENCE_REPORT_MAXIMUM_CANONICAL_BYTES {
            return Err(RadrootsRhiEvidenceReportError::ReportTooLarge);
        }
        Ok(Self {
            issuer_public_key: fields.issuer_public_key,
            trade_id: fields.trade_id,
            claim_mutation_id: fields.claim_mutation_id,
            outcome: fields.outcome,
            reason_codes: fields.reason_codes,
            projection_digest: fields.projection_digest,
            evidence_manifest_digest: fields.evidence_manifest_digest,
            evidence_policy_digest: fields.evidence_policy_digest,
            observed_at_unix_s: fields.observed_at_unix_s,
            supersession: fields.supersession,
            trade_generation: fields.trade_generation,
            statement_digest,
            canonical_statement_payload: canonical_statement_payload.into_boxed_str(),
            canonical_content: canonical_content.into_boxed_str(),
        })
    }

    pub fn validate_against_manifest(
        &self,
        manifest: &RadrootsTradeEvidenceManifestV1,
    ) -> Result<(), RadrootsRhiEvidenceReportError> {
        if self.trade_id != *manifest.trade_id()
            || self.trade_generation != manifest.trade_generation()
            || self.evidence_manifest_digest != manifest.digest()
            || self.evidence_policy_digest != manifest.evidence_policy_digest()
            || self.observed_at_unix_s != manifest.observed_at_unix_s()
        {
            return Err(RadrootsRhiEvidenceReportError::ManifestMismatch);
        }
        if !manifest.coverage().permits(self.outcome) {
            return Err(RadrootsRhiEvidenceReportError::OutcomeNotPermitted);
        }
        Ok(())
    }

    pub const fn contract_id(&self) -> &'static str {
        RADROOTS_RHI_EVIDENCE_REPORT_CONTRACT_ID
    }

    pub const fn contract_version(&self) -> u16 {
        RADROOTS_RHI_EVIDENCE_REPORT_CONTRACT_VERSION
    }

    pub const fn issuer_public_key(&self) -> PublicKey {
        self.issuer_public_key
    }

    pub const fn trade_id(&self) -> &TradeId {
        &self.trade_id
    }

    pub const fn claim_mutation_id(&self) -> MutationId {
        self.claim_mutation_id
    }

    pub const fn outcome(&self) -> RadrootsTradeEvidenceOutcomeV1 {
        self.outcome
    }

    pub fn reason_codes(&self) -> &[RadrootsRhiEvidenceReasonCodeV1] {
        &self.reason_codes
    }

    pub const fn projection_digest(&self) -> RadrootsTradeEvidenceProjectionDigestV1 {
        self.projection_digest
    }

    pub const fn evidence_manifest_digest(&self) -> RadrootsTradeEvidenceManifestDigestV1 {
        self.evidence_manifest_digest
    }

    pub const fn evidence_policy_digest(&self) -> RadrootsTradeEvidencePolicyDigestV1 {
        self.evidence_policy_digest
    }

    pub const fn observed_at_unix_s(&self) -> u64 {
        self.observed_at_unix_s
    }

    pub const fn supersession(&self) -> Option<RadrootsRhiEvidenceSupersessionV1> {
        self.supersession
    }

    pub const fn trade_generation(&self) -> NonZeroU64 {
        self.trade_generation
    }

    pub const fn reducer_contract_id(&self) -> &'static str {
        RADROOTS_TRADE_REDUCER_CONTRACT_ID
    }

    pub const fn reducer_contract_version(&self) -> u16 {
        RADROOTS_TRADE_REDUCER_VERSION
    }

    pub const fn attestation_method(&self) -> &'static str {
        RADROOTS_RHI_EVIDENCE_ATTESTATION_METHOD
    }

    pub const fn statement_digest(&self) -> RadrootsRhiEvidenceStatementDigestV1 {
        self.statement_digest
    }

    pub fn canonical_statement_payload(&self) -> &str {
        &self.canonical_statement_payload
    }

    pub fn canonical_content(&self) -> &str {
        &self.canonical_content
    }
}

impl fmt::Debug for RadrootsRhiEvidenceReportV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RadrootsRhiEvidenceReportV1")
            .field("outcome", &self.outcome)
            .field("reason_code_count", &self.reason_codes.len())
            .field("has_supersession", &self.supersession.is_some())
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadrootsRhiEvidenceReportError {
    ReasonCodeCountOutOfRange,
    InvalidReasonCode,
    DuplicateReasonCode,
    OutcomeNotPermitted,
    ReportTooLarge,
    Malformed,
    UnsupportedVersion,
    FixedFieldMismatch,
    InvalidIdentifier,
    InvalidDigest,
    InvalidOutcome,
    InvalidTradeGeneration,
    IncompleteSupersession,
    StatementDigestMismatch,
    NonCanonical,
    ManifestMismatch,
}

impl fmt::Display for RadrootsRhiEvidenceReportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::ReasonCodeCountOutOfRange => "evidence report reason-code count is out of range",
            Self::InvalidReasonCode => "evidence report reason code is invalid",
            Self::DuplicateReasonCode => "evidence report contains a duplicate reason code",
            Self::OutcomeNotPermitted => "evidence report outcome is not permitted by coverage",
            Self::ReportTooLarge => "evidence report exceeds its canonical byte limit",
            Self::Malformed => "evidence report encoding is malformed",
            Self::UnsupportedVersion => "evidence report version is unsupported",
            Self::FixedFieldMismatch => "evidence report fixed field does not match the contract",
            Self::InvalidIdentifier => "evidence report identifier is invalid",
            Self::InvalidDigest => "evidence report digest is invalid",
            Self::InvalidOutcome => "evidence report outcome is invalid",
            Self::InvalidTradeGeneration => "evidence report trade generation is invalid",
            Self::IncompleteSupersession => "evidence report supersession is incomplete",
            Self::StatementDigestMismatch => "evidence report statement digest does not match",
            Self::NonCanonical => "evidence report encoding is not canonical",
            Self::ManifestMismatch => "evidence report does not match the evidence manifest",
        })
    }
}

#[cfg(feature = "std")]
impl std::error::Error for RadrootsRhiEvidenceReportError {}

struct ReportFields {
    issuer_public_key: PublicKey,
    trade_id: TradeId,
    claim_mutation_id: MutationId,
    outcome: RadrootsTradeEvidenceOutcomeV1,
    reason_codes: Box<[RadrootsRhiEvidenceReasonCodeV1]>,
    projection_digest: RadrootsTradeEvidenceProjectionDigestV1,
    evidence_manifest_digest: RadrootsTradeEvidenceManifestDigestV1,
    evidence_policy_digest: RadrootsTradeEvidencePolicyDigestV1,
    observed_at_unix_s: u64,
    supersession: Option<RadrootsRhiEvidenceSupersessionV1>,
    trade_generation: NonZeroU64,
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

fn normalize_reason_codes(
    reason_codes: impl IntoIterator<Item = RadrootsRhiEvidenceReasonCodeV1>,
) -> Result<Box<[RadrootsRhiEvidenceReasonCodeV1]>, RadrootsRhiEvidenceReportError> {
    let mut values = Vec::new();
    for value in reason_codes {
        if values.len() == RADROOTS_RHI_EVIDENCE_REPORT_MAXIMUM_REASON_CODES {
            return Err(RadrootsRhiEvidenceReportError::ReasonCodeCountOutOfRange);
        }
        values.push(value);
    }
    if values.is_empty() {
        return Err(RadrootsRhiEvidenceReportError::ReasonCodeCountOutOfRange);
    }
    values.sort();
    if values.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(RadrootsRhiEvidenceReportError::DuplicateReasonCode);
    }
    Ok(values.into_boxed_slice())
}

fn parse_reason_codes(
    values: Vec<String>,
) -> Result<Box<[RadrootsRhiEvidenceReasonCodeV1]>, RadrootsRhiEvidenceReportError> {
    if values.len() > RADROOTS_RHI_EVIDENCE_REPORT_MAXIMUM_REASON_CODES {
        return Err(RadrootsRhiEvidenceReportError::ReasonCodeCountOutOfRange);
    }
    let original = values.iter().map(String::as_str).collect::<Vec<_>>();
    let reason_codes = normalize_reason_codes(
        values
            .iter()
            .map(RadrootsRhiEvidenceReasonCodeV1::parse)
            .collect::<Result<Vec<_>, _>>()?,
    )?;
    if original.iter().copied().ne(reason_codes
        .iter()
        .map(RadrootsRhiEvidenceReasonCodeV1::as_str))
    {
        return Err(RadrootsRhiEvidenceReportError::NonCanonical);
    }
    Ok(reason_codes)
}

fn parse_supersession(
    report_id: Option<&str>,
    event_id: Option<&str>,
) -> Result<Option<RadrootsRhiEvidenceSupersessionV1>, RadrootsRhiEvidenceReportError> {
    match (report_id, event_id) {
        (None, None) => Ok(None),
        (Some(report_id), Some(event_id)) => Ok(Some(RadrootsRhiEvidenceSupersessionV1::new(
            parse_digest(report_id)?,
            EventId::parse(event_id)
                .map_err(|_| RadrootsRhiEvidenceReportError::InvalidIdentifier)?,
        ))),
        _ => Err(RadrootsRhiEvidenceReportError::IncompleteSupersession),
    }
}

fn parse_outcome(
    value: &str,
) -> Result<RadrootsTradeEvidenceOutcomeV1, RadrootsRhiEvidenceReportError> {
    match value {
        "valid" => Ok(RadrootsTradeEvidenceOutcomeV1::Valid),
        "invalid" => Ok(RadrootsTradeEvidenceOutcomeV1::Invalid),
        "indeterminate" => Ok(RadrootsTradeEvidenceOutcomeV1::Indeterminate),
        _ => Err(RadrootsRhiEvidenceReportError::InvalidOutcome),
    }
}

const fn outcome_name(outcome: RadrootsTradeEvidenceOutcomeV1) -> &'static str {
    match outcome {
        RadrootsTradeEvidenceOutcomeV1::Valid => "valid",
        RadrootsTradeEvidenceOutcomeV1::Invalid => "invalid",
        RadrootsTradeEvidenceOutcomeV1::Indeterminate => "indeterminate",
    }
}

fn parse_digest(
    value: &str,
) -> Result<RadrootsRhiEvidenceStatementDigestV1, RadrootsRhiEvidenceReportError> {
    Ok(RadrootsRhiEvidenceStatementDigestV1::from_bytes(
        parse_hex_32(value)?,
    ))
}

fn parse_hex_32(value: &str) -> Result<[u8; 32], RadrootsRhiEvidenceReportError> {
    if value.len() != 64
        || value
            .bytes()
            .any(|byte| !(byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)))
    {
        return Err(RadrootsRhiEvidenceReportError::InvalidDigest);
    }
    let decoded = hex::decode(value).map_err(|_| RadrootsRhiEvidenceReportError::InvalidDigest)?;
    decoded
        .try_into()
        .map_err(|_| RadrootsRhiEvidenceReportError::InvalidDigest)
}

fn statement_digest(payload: &[u8]) -> RadrootsRhiEvidenceStatementDigestV1 {
    let mut digest = Sha256::new();
    digest.update(STATEMENT_DIGEST_DOMAIN);
    digest.update(payload);
    RadrootsRhiEvidenceStatementDigestV1::from_bytes(digest.finalize().into())
}

fn canonical_statement_payload(
    fields: &ReportFields,
) -> Result<String, RadrootsRhiEvidenceReportError> {
    canonical_json(statement_value(fields))
}

fn canonical_report_content(
    fields: &ReportFields,
    statement_digest: RadrootsRhiEvidenceStatementDigestV1,
) -> Result<String, RadrootsRhiEvidenceReportError> {
    let mut value = statement_value(fields);
    let object = value
        .as_object_mut()
        .ok_or(RadrootsRhiEvidenceReportError::Malformed)?;
    object.insert("report_id".into(), json!(statement_digest.to_hex()));
    object.insert("statement_digest".into(), json!(statement_digest.to_hex()));
    canonical_json(value)
}

fn statement_value(fields: &ReportFields) -> Value {
    let (supersedes_report_id, supersedes_event_id) =
        fields
            .supersession
            .map_or((Value::Null, Value::Null), |supersession| {
                (
                    json!(supersession.report_id.to_hex()),
                    json!(supersession.event_id.to_hex()),
                )
            });
    json!({
        "contract_id": RADROOTS_RHI_EVIDENCE_REPORT_CONTRACT_ID,
        "contract_version": RADROOTS_RHI_EVIDENCE_REPORT_CONTRACT_VERSION,
        "issuer_pubkey": fields.issuer_public_key.to_hex(),
        "trade_id": fields.trade_id.to_hex(),
        "claim_mutation_id": fields.claim_mutation_id.to_hex(),
        "outcome": outcome_name(fields.outcome),
        "reason_codes": fields.reason_codes.iter().map(RadrootsRhiEvidenceReasonCodeV1::as_str).collect::<Vec<_>>(),
        "reducer_contract_id": RADROOTS_TRADE_REDUCER_CONTRACT_ID,
        "reducer_contract_version": RADROOTS_TRADE_REDUCER_VERSION,
        "projection_digest": fields.projection_digest.to_hex(),
        "evidence_manifest_digest": fields.evidence_manifest_digest.to_hex(),
        "evidence_policy_digest": fields.evidence_policy_digest.to_hex(),
        "observed_at_unix_s": fields.observed_at_unix_s,
        "attestation_method": RADROOTS_RHI_EVIDENCE_ATTESTATION_METHOD,
        "supersedes_report_id": supersedes_report_id,
        "supersedes_event_id": supersedes_event_id,
        "trade_generation": fields.trade_generation.get(),
    })
}

fn canonical_json(value: Value) -> Result<String, RadrootsRhiEvidenceReportError> {
    radroots_event::trade::canonical_jcs_value(&value)
        .map_err(|_| RadrootsRhiEvidenceReportError::Malformed)
}

#[cfg(test)]
mod tests {
    use alloc::{format, vec};

    use super::*;
    use crate::evidence::{
        RadrootsTradeEvidenceCoverageV1, RadrootsTradeEvidenceManifestSourceResultV1,
        RadrootsTradeEvidenceScopePrerequisitesV1, RadrootsTradeEvidenceSourceCompletionV1,
        RadrootsTradeEvidenceSourceRequirementV1, RadrootsTradeEvidenceSourceResultDigestV1,
        RadrootsTradeEvidenceSourceResultV1,
    };

    const CURRENT_REPORT: &str = "{\"attestation_method\":\"signed_evidence_snapshot\",\"claim_mutation_id\":\"2222222222222222222222222222222222222222222222222222222222222222\",\"contract_id\":\"radroots.rhi.evidence_attestation.v1\",\"contract_version\":1,\"evidence_manifest_digest\":\"4444444444444444444444444444444444444444444444444444444444444444\",\"evidence_policy_digest\":\"5555555555555555555555555555555555555555555555555555555555555555\",\"issuer_pubkey\":\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\",\"observed_at_unix_s\":1800000000,\"outcome\":\"indeterminate\",\"projection_digest\":\"6666666666666666666666666666666666666666666666666666666666666666\",\"reason_codes\":[\"required_source_incomplete\"],\"reducer_contract_id\":\"radroots.trade.reducer.v1\",\"reducer_contract_version\":1,\"report_id\":\"461acfea579481f6aadba47c31abc07eaa700f66abc074b8926ba41266082b44\",\"statement_digest\":\"461acfea579481f6aadba47c31abc07eaa700f66abc074b8926ba41266082b44\",\"supersedes_event_id\":null,\"supersedes_report_id\":null,\"trade_generation\":7,\"trade_id\":\"11111111111111111111111111111111\"}";

    const CURRENT_STATEMENT: &str = "{\"attestation_method\":\"signed_evidence_snapshot\",\"claim_mutation_id\":\"2222222222222222222222222222222222222222222222222222222222222222\",\"contract_id\":\"radroots.rhi.evidence_attestation.v1\",\"contract_version\":1,\"evidence_manifest_digest\":\"4444444444444444444444444444444444444444444444444444444444444444\",\"evidence_policy_digest\":\"5555555555555555555555555555555555555555555555555555555555555555\",\"issuer_pubkey\":\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\",\"observed_at_unix_s\":1800000000,\"outcome\":\"indeterminate\",\"projection_digest\":\"6666666666666666666666666666666666666666666666666666666666666666\",\"reason_codes\":[\"required_source_incomplete\"],\"reducer_contract_id\":\"radroots.trade.reducer.v1\",\"reducer_contract_version\":1,\"supersedes_event_id\":null,\"supersedes_report_id\":null,\"trade_generation\":7,\"trade_id\":\"11111111111111111111111111111111\"}";

    const SUPERSEDING_REPORT: &str = "{\"attestation_method\":\"signed_evidence_snapshot\",\"claim_mutation_id\":\"2222222222222222222222222222222222222222222222222222222222222222\",\"contract_id\":\"radroots.rhi.evidence_attestation.v1\",\"contract_version\":1,\"evidence_manifest_digest\":\"4444444444444444444444444444444444444444444444444444444444444444\",\"evidence_policy_digest\":\"5555555555555555555555555555555555555555555555555555555555555555\",\"issuer_pubkey\":\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\",\"observed_at_unix_s\":1800000100,\"outcome\":\"valid\",\"projection_digest\":\"6666666666666666666666666666666666666666666666666666666666666666\",\"reason_codes\":[\"scope_satisfied\"],\"reducer_contract_id\":\"radroots.trade.reducer.v1\",\"reducer_contract_version\":1,\"report_id\":\"61af3caa6a3a14ff7bb026e9f294826d1d1957a5aff3d814156acee9cf0d5807\",\"statement_digest\":\"61af3caa6a3a14ff7bb026e9f294826d1d1957a5aff3d814156acee9cf0d5807\",\"supersedes_event_id\":\"8888888888888888888888888888888888888888888888888888888888888888\",\"supersedes_report_id\":\"7777777777777777777777777777777777777777777777777777777777777777\",\"trade_generation\":8,\"trade_id\":\"11111111111111111111111111111111\"}";

    fn manifest(coverage: RadrootsTradeEvidenceCoverageV1) -> RadrootsTradeEvidenceManifestV1 {
        let completion = match coverage {
            RadrootsTradeEvidenceCoverageV1::ScopeSatisfied => {
                RadrootsTradeEvidenceSourceCompletionV1::Complete
            }
            RadrootsTradeEvidenceCoverageV1::Unsupported => {
                RadrootsTradeEvidenceSourceCompletionV1::Unsupported
            }
            RadrootsTradeEvidenceCoverageV1::Partial => {
                RadrootsTradeEvidenceSourceCompletionV1::Complete
            }
            RadrootsTradeEvidenceCoverageV1::Missing => {
                RadrootsTradeEvidenceSourceCompletionV1::Incomplete
            }
        };
        let scope = if matches!(coverage, RadrootsTradeEvidenceCoverageV1::ScopeSatisfied) {
            RadrootsTradeEvidenceScopePrerequisitesV1::Satisfied
        } else {
            RadrootsTradeEvidenceScopePrerequisitesV1::Unsatisfied
        };
        let manifest = RadrootsTradeEvidenceManifestV1::new(
            TradeId::from_bytes([0x11; 16]),
            NonZeroU64::new(7).unwrap(),
            RadrootsTradeEvidencePolicyDigestV1::from_bytes([0x55; 32]),
            1_800_000_000,
            scope,
            [RadrootsTradeEvidenceManifestSourceResultV1::new(
                crate::evidence::RadrootsTradeEvidenceSourceIdV1::parse("relay_a").unwrap(),
                RadrootsTradeEvidenceSourceResultV1::new(
                    RadrootsTradeEvidenceSourceRequirementV1::Required,
                    completion,
                    0,
                )
                .unwrap(),
                RadrootsTradeEvidenceSourceResultDigestV1::from_bytes([0x33; 32]),
            )],
            [],
        )
        .unwrap();
        assert_eq!(manifest.coverage(), coverage);
        manifest
    }

    fn report(
        coverage: RadrootsTradeEvidenceCoverageV1,
        outcome: RadrootsTradeEvidenceOutcomeV1,
    ) -> Result<RadrootsRhiEvidenceReportV1, RadrootsRhiEvidenceReportError> {
        RadrootsRhiEvidenceReportV1::new(
            PublicKey::from_hex("585591529da0bab31b3b1b1f986611cf5f435dca84f978c89ee8a40cca7103df")
                .unwrap(),
            MutationId::from_bytes([0x22; 32]),
            outcome,
            [RadrootsRhiEvidenceReasonCodeV1::parse("scope_satisfied").unwrap()],
            RadrootsTradeEvidenceProjectionDigestV1::from_bytes([0x66; 32]),
            &manifest(coverage),
            None,
        )
    }

    fn report_with_reasons(
        manifest: &RadrootsTradeEvidenceManifestV1,
        reasons: impl IntoIterator<Item = RadrootsRhiEvidenceReasonCodeV1>,
    ) -> Result<RadrootsRhiEvidenceReportV1, RadrootsRhiEvidenceReportError> {
        RadrootsRhiEvidenceReportV1::new(
            PublicKey::from_hex("585591529da0bab31b3b1b1f986611cf5f435dca84f978c89ee8a40cca7103df")
                .unwrap(),
            MutationId::from_bytes([0x22; 32]),
            RadrootsTradeEvidenceOutcomeV1::Valid,
            reasons,
            RadrootsTradeEvidenceProjectionDigestV1::from_bytes([0x66; 32]),
            manifest,
            None,
        )
    }

    #[test]
    fn fixed_report_vectors_match_statement_hashes_and_supersession() {
        let current =
            RadrootsRhiEvidenceReportV1::from_canonical_content(CURRENT_REPORT.as_bytes())
                .expect("current report vector");
        assert_eq!(current.canonical_statement_payload(), CURRENT_STATEMENT);
        assert_eq!(current.canonical_content(), CURRENT_REPORT);
        assert_eq!(
            current.statement_digest().to_hex(),
            "461acfea579481f6aadba47c31abc07eaa700f66abc074b8926ba41266082b44"
        );
        assert!(current.supersession().is_none());

        let superseding =
            RadrootsRhiEvidenceReportV1::from_canonical_content(SUPERSEDING_REPORT.as_bytes())
                .expect("superseding report vector");
        assert_eq!(
            superseding.statement_digest().to_hex(),
            "61af3caa6a3a14ff7bb026e9f294826d1d1957a5aff3d814156acee9cf0d5807"
        );
        assert!(superseding.supersession().is_some());
    }

    #[test]
    fn construction_is_canonical_manifest_bound_and_permutation_invariant() {
        let manifest = manifest(RadrootsTradeEvidenceCoverageV1::ScopeSatisfied);
        let a = RadrootsRhiEvidenceReportV1::new(
            PublicKey::from_hex("585591529da0bab31b3b1b1f986611cf5f435dca84f978c89ee8a40cca7103df")
                .unwrap(),
            MutationId::from_bytes([0x22; 32]),
            RadrootsTradeEvidenceOutcomeV1::Valid,
            [
                RadrootsRhiEvidenceReasonCodeV1::parse("source_complete").unwrap(),
                RadrootsRhiEvidenceReasonCodeV1::parse("scope_satisfied").unwrap(),
            ],
            RadrootsTradeEvidenceProjectionDigestV1::from_bytes([0x66; 32]),
            &manifest,
            None,
        )
        .unwrap();
        let b = RadrootsRhiEvidenceReportV1::new(
            a.issuer_public_key(),
            a.claim_mutation_id(),
            a.outcome(),
            a.reason_codes().iter().cloned().rev(),
            a.projection_digest(),
            &manifest,
            None,
        )
        .unwrap();
        assert_eq!(a.canonical_content(), b.canonical_content());
        assert_eq!(a.statement_digest(), b.statement_digest());
        a.validate_against_manifest(&manifest).unwrap();
        assert_eq!(
            RadrootsRhiEvidenceReportV1::from_canonical_content(a.canonical_content().as_bytes()),
            Ok(a)
        );
    }

    #[test]
    fn coverage_outcome_matrix_fails_closed() {
        use RadrootsTradeEvidenceCoverageV1::{Missing, Partial, ScopeSatisfied, Unsupported};
        use RadrootsTradeEvidenceOutcomeV1::{Indeterminate, Invalid, Valid};

        for coverage in [Missing, Partial, ScopeSatisfied, Unsupported] {
            for outcome in [Valid, Invalid, Indeterminate] {
                let result = report(coverage, outcome);
                assert_eq!(result.is_ok(), coverage.permits(outcome));
            }
        }
    }

    #[test]
    fn reason_codes_are_bounded_validated_sorted_and_unique() {
        for invalid in ["", "Upper", "ends_", "has-hyphen", "has space"] {
            assert_eq!(
                RadrootsRhiEvidenceReasonCodeV1::parse(invalid),
                Err(RadrootsRhiEvidenceReportError::InvalidReasonCode)
            );
        }
        assert!(RadrootsRhiEvidenceReasonCodeV1::parse("a".repeat(64)).is_ok());
        assert_eq!(
            RadrootsRhiEvidenceReasonCodeV1::parse("a".repeat(65)),
            Err(RadrootsRhiEvidenceReportError::InvalidReasonCode)
        );

        let manifest = manifest(RadrootsTradeEvidenceCoverageV1::ScopeSatisfied);
        assert_eq!(
            report_with_reasons(&manifest, Vec::new()),
            Err(RadrootsRhiEvidenceReportError::ReasonCodeCountOutOfRange)
        );
        let maximum = (0..16)
            .map(|index| RadrootsRhiEvidenceReasonCodeV1::parse(format!("reason_{index:02}")))
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert!(report_with_reasons(&manifest, maximum.clone()).is_ok());
        assert_eq!(
            report_with_reasons(
                &manifest,
                maximum.into_iter().chain(core::iter::repeat(
                    RadrootsRhiEvidenceReasonCodeV1::parse("overflow").unwrap(),
                )),
            ),
            Err(RadrootsRhiEvidenceReportError::ReasonCodeCountOutOfRange)
        );
        let duplicate = RadrootsRhiEvidenceReasonCodeV1::parse("same").unwrap();
        assert_eq!(
            report_with_reasons(&manifest, [duplicate.clone(), duplicate]),
            Err(RadrootsRhiEvidenceReportError::DuplicateReasonCode)
        );
    }

    #[test]
    fn parser_rejects_noncanonical_malformed_and_unbound_reports() {
        let mut value: Value = serde_json::from_str(CURRENT_REPORT).unwrap();
        value["report_id"] = json!("00".repeat(32));
        assert_eq!(
            RadrootsRhiEvidenceReportV1::from_canonical_content(
                canonical_json(value).unwrap().as_bytes()
            ),
            Err(RadrootsRhiEvidenceReportError::StatementDigestMismatch)
        );
        assert_eq!(
            RadrootsRhiEvidenceReportV1::from_canonical_content(
                format!(" {CURRENT_REPORT}").as_bytes()
            ),
            Err(RadrootsRhiEvidenceReportError::NonCanonical)
        );
        assert_eq!(
            RadrootsRhiEvidenceReportV1::from_canonical_content(
                CURRENT_REPORT
                    .replacen(
                        "\"contract_version\":1",
                        "\"contract_version\":1,\"contract_version\":1",
                        1,
                    )
                    .as_bytes()
            ),
            Err(RadrootsRhiEvidenceReportError::Malformed)
        );
        assert_eq!(
            RadrootsRhiEvidenceReportV1::from_canonical_content(
                CURRENT_REPORT
                    .replacen(
                        "\"contract_version\":1",
                        "\"contract_version\":1,\"unknown\":true",
                        1,
                    )
                    .as_bytes()
            ),
            Err(RadrootsRhiEvidenceReportError::Malformed)
        );
        assert_eq!(
            RadrootsRhiEvidenceReportV1::from_canonical_content(
                CURRENT_REPORT
                    .replacen(
                        "\"supersedes_report_id\":null",
                        &format!("\"supersedes_report_id\":\"{}\"", "77".repeat(32)),
                        1,
                    )
                    .as_bytes()
            ),
            Err(RadrootsRhiEvidenceReportError::IncompleteSupersession)
        );
        assert_eq!(
            RadrootsRhiEvidenceReportV1::from_canonical_content(&vec![
                b'x';
                RADROOTS_RHI_EVIDENCE_REPORT_MAXIMUM_CANONICAL_BYTES
                    + 1
            ]),
            Err(RadrootsRhiEvidenceReportError::ReportTooLarge)
        );
        assert_eq!(
            RadrootsRhiEvidenceReportV1::from_canonical_content(&vec![
                b'x';
                RADROOTS_RHI_EVIDENCE_REPORT_MAXIMUM_CANONICAL_BYTES
            ]),
            Err(RadrootsRhiEvidenceReportError::Malformed)
        );
    }

    #[test]
    fn parser_rejects_every_fixed_identity_and_reason_drift() {
        for (from, to, error) in [
            (
                "\"contract_version\":1",
                "\"contract_version\":2",
                RadrootsRhiEvidenceReportError::UnsupportedVersion,
            ),
            (
                "radroots.rhi.evidence_attestation.v1",
                "radroots.rhi.evidence_attestation.v2",
                RadrootsRhiEvidenceReportError::FixedFieldMismatch,
            ),
            (
                "signed_evidence_snapshot",
                "unsigned_evidence_snapshot",
                RadrootsRhiEvidenceReportError::FixedFieldMismatch,
            ),
            (
                "\"outcome\":\"indeterminate\"",
                "\"outcome\":\"complete\"",
                RadrootsRhiEvidenceReportError::InvalidOutcome,
            ),
            (
                "\"trade_generation\":7",
                "\"trade_generation\":0",
                RadrootsRhiEvidenceReportError::InvalidTradeGeneration,
            ),
            (
                "\"projection_digest\":\"6666",
                "\"projection_digest\":\"GG66",
                RadrootsRhiEvidenceReportError::InvalidDigest,
            ),
        ] {
            assert_eq!(
                RadrootsRhiEvidenceReportV1::from_canonical_content(
                    CURRENT_REPORT.replacen(from, to, 1).as_bytes()
                ),
                Err(error)
            );
        }

        assert_eq!(
            RadrootsRhiEvidenceReportV1::from_canonical_content(
                CURRENT_REPORT
                    .replacen(
                        "[\"required_source_incomplete\"]",
                        "[\"z_reason\",\"a_reason\"]",
                        1,
                    )
                    .as_bytes()
            ),
            Err(RadrootsRhiEvidenceReportError::NonCanonical)
        );
        assert_eq!(
            RadrootsRhiEvidenceReportV1::from_canonical_content(
                CURRENT_REPORT
                    .replacen("[\"required_source_incomplete\"]", "[\"same\",\"same\"]", 1,)
                    .as_bytes()
            ),
            Err(RadrootsRhiEvidenceReportError::DuplicateReasonCode)
        );

        let parsed =
            RadrootsRhiEvidenceReportV1::from_canonical_content(CURRENT_REPORT.as_bytes()).unwrap();
        assert_eq!(
            parsed.validate_against_manifest(&manifest(
                RadrootsTradeEvidenceCoverageV1::ScopeSatisfied
            )),
            Err(RadrootsRhiEvidenceReportError::ManifestMismatch)
        );
    }

    #[test]
    fn diagnostics_redact_identifiers_digests_and_content() {
        let report = report(
            RadrootsTradeEvidenceCoverageV1::ScopeSatisfied,
            RadrootsTradeEvidenceOutcomeV1::Valid,
        )
        .unwrap();
        let debug = format!("{report:?}");
        for secret in [
            report.trade_id().to_hex(),
            report.claim_mutation_id().to_hex(),
            report.statement_digest().to_hex(),
        ] {
            assert!(!debug.contains(&secret));
        }
        assert_eq!(
            RadrootsRhiEvidenceReportError::ManifestMismatch.to_string(),
            "evidence report does not match the evidence manifest"
        );
        assert!(std::error::Error::source(&RadrootsRhiEvidenceReportError::Malformed).is_none());
    }
}
