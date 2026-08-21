#![forbid(unsafe_code)]

//! Immutable evidence inventory for one governed trade generation.
//!
//! The manifest commits to caller-supplied canonical record bytes through
//! semantically distinct SHA-256 digest types. It does not retrieve, validate,
//! persist, sign, or publish those records.

use alloc::{boxed::Box, string::String, vec, vec::Vec};
use core::{fmt, num::NonZeroU64};

use radroots_event::id::{EventId, MutationId, TradeId};
use sha2::{Digest as _, Sha256};

#[cfg(test)]
use crate::evidence::RADROOTS_TRADE_EVIDENCE_MAXIMUM_EVENTS_PER_SOURCE;
use crate::evidence::{
    RADROOTS_TRADE_EVIDENCE_MAXIMUM_SOURCE_COUNT, RadrootsTradeEvidenceCoverageError,
    RadrootsTradeEvidenceCoverageV1, RadrootsTradeEvidenceScopePrerequisitesV1,
    RadrootsTradeEvidenceSourceCompletionV1, RadrootsTradeEvidenceSourceRequirementV1,
    RadrootsTradeEvidenceSourceResultV1, classify_trade_evidence_coverage_v1,
};

pub const RADROOTS_TRADE_EVIDENCE_MANIFEST_CONTRACT_ID: &str =
    "radroots.trade.evidence-manifest.v1";
pub const RADROOTS_TRADE_EVIDENCE_MANIFEST_CONTRACT_VERSION: u16 = 1;
pub const RADROOTS_TRADE_EVIDENCE_SOURCE_ID_MAXIMUM_BYTES: usize = 64;
pub const RADROOTS_TRADE_EVIDENCE_MANIFEST_MAXIMUM_OBSERVATIONS: usize = 65_536;
pub const RADROOTS_TRADE_EVIDENCE_MANIFEST_MAXIMUM_BYTES: usize = 16 * 1024 * 1024;

const MANIFEST_PREFIX: &[u8] = b"radroots.trade.evidence-manifest.v1\0";
const MANIFEST_DIGEST_DOMAIN: &[u8] = b"radroots:trade-evidence-manifest-digest:v1\0";

macro_rules! define_content_digest {
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

define_content_digest!(RadrootsTradeEvidencePolicyDigestV1);
define_content_digest!(RadrootsTradeEvidenceSourceResultDigestV1);
define_content_digest!(RadrootsTradeSignedEventDigestV1);
define_content_digest!(RadrootsTradeEvidenceProvenanceDigestV1);

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RadrootsTradeEvidenceManifestDigestV1([u8; 32]);

impl RadrootsTradeEvidenceManifestDigestV1 {
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

impl fmt::Debug for RadrootsTradeEvidenceManifestDigestV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("RadrootsTradeEvidenceManifestDigestV1(<redacted>)")
    }
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RadrootsTradeEvidenceSourceIdV1(String);

impl RadrootsTradeEvidenceSourceIdV1 {
    pub fn parse(value: impl AsRef<str>) -> Result<Self, RadrootsTradeEvidenceManifestError> {
        let value = value.as_ref();
        if value.is_empty()
            || value.len() > RADROOTS_TRADE_EVIDENCE_SOURCE_ID_MAXIMUM_BYTES
            || !value.as_bytes()[0].is_ascii_lowercase()
            || value.bytes().any(|byte| {
                !(byte.is_ascii_lowercase()
                    || byte.is_ascii_digit()
                    || byte == b'_'
                    || byte == b'-')
            })
        {
            return Err(RadrootsTradeEvidenceManifestError::InvalidSourceId);
        }
        Ok(Self(String::from(value)))
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Debug for RadrootsTradeEvidenceSourceIdV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("RadrootsTradeEvidenceSourceIdV1(<redacted>)")
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct RadrootsTradeEvidenceManifestSourceResultV1 {
    source_id: RadrootsTradeEvidenceSourceIdV1,
    result: RadrootsTradeEvidenceSourceResultV1,
    result_digest: RadrootsTradeEvidenceSourceResultDigestV1,
}

impl RadrootsTradeEvidenceManifestSourceResultV1 {
    pub const fn new(
        source_id: RadrootsTradeEvidenceSourceIdV1,
        result: RadrootsTradeEvidenceSourceResultV1,
        result_digest: RadrootsTradeEvidenceSourceResultDigestV1,
    ) -> Self {
        Self {
            source_id,
            result,
            result_digest,
        }
    }

    pub const fn source_id(&self) -> &RadrootsTradeEvidenceSourceIdV1 {
        &self.source_id
    }

    pub const fn result(&self) -> RadrootsTradeEvidenceSourceResultV1 {
        self.result
    }

    pub const fn result_digest(&self) -> RadrootsTradeEvidenceSourceResultDigestV1 {
        self.result_digest
    }
}

impl fmt::Debug for RadrootsTradeEvidenceManifestSourceResultV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("RadrootsTradeEvidenceManifestSourceResultV1(<redacted>)")
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct RadrootsTradeEvidenceManifestObservationV1 {
    source_id: RadrootsTradeEvidenceSourceIdV1,
    mutation_id: MutationId,
    event_id: EventId,
    signed_event_digest: RadrootsTradeSignedEventDigestV1,
    provenance_digest: RadrootsTradeEvidenceProvenanceDigestV1,
}

impl RadrootsTradeEvidenceManifestObservationV1 {
    pub const fn new(
        source_id: RadrootsTradeEvidenceSourceIdV1,
        mutation_id: MutationId,
        event_id: EventId,
        signed_event_digest: RadrootsTradeSignedEventDigestV1,
        provenance_digest: RadrootsTradeEvidenceProvenanceDigestV1,
    ) -> Self {
        Self {
            source_id,
            mutation_id,
            event_id,
            signed_event_digest,
            provenance_digest,
        }
    }

    pub const fn source_id(&self) -> &RadrootsTradeEvidenceSourceIdV1 {
        &self.source_id
    }

    pub const fn mutation_id(&self) -> &MutationId {
        &self.mutation_id
    }

    pub const fn event_id(&self) -> &EventId {
        &self.event_id
    }

    pub const fn signed_event_digest(&self) -> RadrootsTradeSignedEventDigestV1 {
        self.signed_event_digest
    }

    pub const fn provenance_digest(&self) -> RadrootsTradeEvidenceProvenanceDigestV1 {
        self.provenance_digest
    }
}

impl fmt::Debug for RadrootsTradeEvidenceManifestObservationV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("RadrootsTradeEvidenceManifestObservationV1(<redacted>)")
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct RadrootsTradeEvidenceManifestV1 {
    trade_id: TradeId,
    trade_generation: NonZeroU64,
    evidence_policy_digest: RadrootsTradeEvidencePolicyDigestV1,
    observed_at_unix_s: u64,
    scope_prerequisites: RadrootsTradeEvidenceScopePrerequisitesV1,
    coverage: RadrootsTradeEvidenceCoverageV1,
    sources: Box<[RadrootsTradeEvidenceManifestSourceResultV1]>,
    observations: Box<[RadrootsTradeEvidenceManifestObservationV1]>,
    canonical_bytes: Box<[u8]>,
    digest: RadrootsTradeEvidenceManifestDigestV1,
}

impl RadrootsTradeEvidenceManifestV1 {
    pub fn new<S, O>(
        trade_id: TradeId,
        trade_generation: NonZeroU64,
        evidence_policy_digest: RadrootsTradeEvidencePolicyDigestV1,
        observed_at_unix_s: u64,
        scope_prerequisites: RadrootsTradeEvidenceScopePrerequisitesV1,
        sources: S,
        observations: O,
    ) -> Result<Self, RadrootsTradeEvidenceManifestError>
    where
        S: IntoIterator<Item = RadrootsTradeEvidenceManifestSourceResultV1>,
        O: IntoIterator<Item = RadrootsTradeEvidenceManifestObservationV1>,
    {
        let mut sources = collect_bounded(
            sources,
            RADROOTS_TRADE_EVIDENCE_MAXIMUM_SOURCE_COUNT,
            RadrootsTradeEvidenceManifestError::SourceCountOutOfRange,
        )?;
        if sources.is_empty() {
            return Err(RadrootsTradeEvidenceManifestError::SourceCountOutOfRange);
        }
        sources.sort_by(|left, right| left.source_id.cmp(&right.source_id));
        if sources
            .windows(2)
            .any(|pair| pair[0].source_id == pair[1].source_id)
        {
            return Err(RadrootsTradeEvidenceManifestError::DuplicateSource);
        }

        let coverage = classify_trade_evidence_coverage_v1(
            sources.iter().map(|source| source.result),
            scope_prerequisites,
        )
        .map_err(map_coverage_error)?;

        let mut observations = collect_bounded(
            observations,
            RADROOTS_TRADE_EVIDENCE_MANIFEST_MAXIMUM_OBSERVATIONS,
            RadrootsTradeEvidenceManifestError::ObservationCountOutOfRange,
        )?;
        observations.sort_by(|left, right| {
            left.event_id
                .as_bytes()
                .cmp(right.event_id.as_bytes())
                .then_with(|| left.source_id.cmp(&right.source_id))
                .then_with(|| {
                    left.mutation_id
                        .as_bytes()
                        .cmp(right.mutation_id.as_bytes())
                })
                .then_with(|| left.signed_event_digest.cmp(&right.signed_event_digest))
                .then_with(|| left.provenance_digest.cmp(&right.provenance_digest))
        });
        for pair in observations.windows(2) {
            if pair[0].event_id == pair[1].event_id
                && (pair[0].mutation_id != pair[1].mutation_id
                    || pair[0].signed_event_digest != pair[1].signed_event_digest)
            {
                return Err(RadrootsTradeEvidenceManifestError::ConflictingEvent);
            }
            if pair[0].event_id == pair[1].event_id && pair[0].source_id == pair[1].source_id {
                return Err(RadrootsTradeEvidenceManifestError::DuplicateObservation);
            }
        }

        let mut actual_counts = vec![0_u32; sources.len()];
        for observation in &observations {
            let source_index = sources
                .binary_search_by(|source| source.source_id.cmp(&observation.source_id))
                .map_err(|_| RadrootsTradeEvidenceManifestError::UnknownObservationSource)?;
            actual_counts[source_index] = actual_counts[source_index]
                .checked_add(1)
                .ok_or(RadrootsTradeEvidenceManifestError::ObservationCountOutOfRange)?;
        }
        if sources
            .iter()
            .zip(actual_counts)
            .any(|(source, actual)| source.result.admitted_event_count() != actual)
        {
            return Err(RadrootsTradeEvidenceManifestError::SourceEventCountMismatch);
        }

        let canonical_bytes = encode_manifest(
            &trade_id,
            trade_generation,
            evidence_policy_digest,
            observed_at_unix_s,
            scope_prerequisites,
            &sources,
            &observations,
        )?;
        let digest = manifest_digest(canonical_bytes.as_slice())?;

        Ok(Self {
            trade_id,
            trade_generation,
            evidence_policy_digest,
            observed_at_unix_s,
            scope_prerequisites,
            coverage,
            sources: sources.into_boxed_slice(),
            observations: observations.into_boxed_slice(),
            canonical_bytes: canonical_bytes.into_boxed_slice(),
            digest,
        })
    }

    pub fn from_canonical_bytes(bytes: &[u8]) -> Result<Self, RadrootsTradeEvidenceManifestError> {
        if bytes.is_empty() {
            return Err(RadrootsTradeEvidenceManifestError::Malformed);
        }
        if bytes.len() > RADROOTS_TRADE_EVIDENCE_MANIFEST_MAXIMUM_BYTES {
            return Err(RadrootsTradeEvidenceManifestError::ManifestTooLarge);
        }

        let mut reader = ManifestReader::new(bytes);
        if reader.take(MANIFEST_PREFIX.len())? != MANIFEST_PREFIX {
            return Err(RadrootsTradeEvidenceManifestError::Malformed);
        }
        if reader.u16()? != RADROOTS_TRADE_EVIDENCE_MANIFEST_CONTRACT_VERSION {
            return Err(RadrootsTradeEvidenceManifestError::UnsupportedVersion);
        }

        let trade_id = TradeId::from_bytes(reader.array()?);
        let trade_generation =
            NonZeroU64::new(reader.u64()?).ok_or(RadrootsTradeEvidenceManifestError::Malformed)?;
        let evidence_policy_digest =
            RadrootsTradeEvidencePolicyDigestV1::from_bytes(reader.array()?);
        let observed_at_unix_s = reader.u64()?;
        let scope_prerequisites = decode_scope_prerequisites(reader.u8()?)?;

        let source_count = usize::from(reader.u8()?);
        if source_count == 0 || source_count > RADROOTS_TRADE_EVIDENCE_MAXIMUM_SOURCE_COUNT {
            return Err(RadrootsTradeEvidenceManifestError::SourceCountOutOfRange);
        }
        let mut sources = Vec::with_capacity(source_count);
        for _ in 0..source_count {
            let source_id = decode_source_id(&mut reader)?;
            let requirement = decode_requirement(reader.u8()?)?;
            let completion = decode_completion(reader.u8()?)?;
            let admitted_event_count = reader.u32()?;
            let result = RadrootsTradeEvidenceSourceResultV1::new(
                requirement,
                completion,
                admitted_event_count,
            )
            .map_err(map_coverage_error)?;
            let result_digest =
                RadrootsTradeEvidenceSourceResultDigestV1::from_bytes(reader.array()?);
            sources.push(RadrootsTradeEvidenceManifestSourceResultV1::new(
                source_id,
                result,
                result_digest,
            ));
        }

        let observation_count = usize::try_from(reader.u32()?)
            .map_err(|_| RadrootsTradeEvidenceManifestError::ObservationCountOutOfRange)?;
        if observation_count > RADROOTS_TRADE_EVIDENCE_MANIFEST_MAXIMUM_OBSERVATIONS {
            return Err(RadrootsTradeEvidenceManifestError::ObservationCountOutOfRange);
        }
        let mut observations = Vec::with_capacity(observation_count);
        for _ in 0..observation_count {
            let source_index = usize::from(reader.u8()?);
            let source = sources
                .get(source_index)
                .ok_or(RadrootsTradeEvidenceManifestError::UnknownObservationSource)?;
            observations.push(RadrootsTradeEvidenceManifestObservationV1::new(
                source.source_id.clone(),
                MutationId::from_bytes(reader.array()?),
                EventId::from_bytes(reader.array()?),
                RadrootsTradeSignedEventDigestV1::from_bytes(reader.array()?),
                RadrootsTradeEvidenceProvenanceDigestV1::from_bytes(reader.array()?),
            ));
        }
        reader.finish()?;

        let manifest = Self::new(
            trade_id,
            trade_generation,
            evidence_policy_digest,
            observed_at_unix_s,
            scope_prerequisites,
            sources,
            observations,
        )?;
        if manifest.canonical_bytes.as_ref() != bytes {
            return Err(RadrootsTradeEvidenceManifestError::NonCanonical);
        }
        Ok(manifest)
    }

    pub const fn contract_id(&self) -> &'static str {
        RADROOTS_TRADE_EVIDENCE_MANIFEST_CONTRACT_ID
    }

    pub const fn contract_version(&self) -> u16 {
        RADROOTS_TRADE_EVIDENCE_MANIFEST_CONTRACT_VERSION
    }

    pub const fn trade_id(&self) -> &TradeId {
        &self.trade_id
    }

    pub const fn trade_generation(&self) -> NonZeroU64 {
        self.trade_generation
    }

    pub const fn evidence_policy_digest(&self) -> RadrootsTradeEvidencePolicyDigestV1 {
        self.evidence_policy_digest
    }

    pub const fn observed_at_unix_s(&self) -> u64 {
        self.observed_at_unix_s
    }

    pub const fn scope_prerequisites(&self) -> RadrootsTradeEvidenceScopePrerequisitesV1 {
        self.scope_prerequisites
    }

    pub const fn coverage(&self) -> RadrootsTradeEvidenceCoverageV1 {
        self.coverage
    }

    pub fn sources(&self) -> &[RadrootsTradeEvidenceManifestSourceResultV1] {
        &self.sources
    }

    pub fn observations(&self) -> &[RadrootsTradeEvidenceManifestObservationV1] {
        &self.observations
    }

    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    pub const fn digest(&self) -> RadrootsTradeEvidenceManifestDigestV1 {
        self.digest
    }
}

impl fmt::Debug for RadrootsTradeEvidenceManifestV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RadrootsTradeEvidenceManifestV1")
            .field("source_count", &self.sources.len())
            .field("observation_count", &self.observations.len())
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadrootsTradeEvidenceManifestError {
    InvalidSourceId,
    SourceCountOutOfRange,
    DuplicateSource,
    NoRequiredSource,
    ObservationCountOutOfRange,
    UnknownObservationSource,
    DuplicateObservation,
    ConflictingEvent,
    SourceEventCountMismatch,
    ManifestTooLarge,
    Malformed,
    UnsupportedVersion,
    NonCanonical,
}

impl fmt::Display for RadrootsTradeEvidenceManifestError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidSourceId => "evidence manifest source identity is invalid",
            Self::SourceCountOutOfRange => "evidence manifest source count is out of range",
            Self::DuplicateSource => "evidence manifest contains a duplicate source",
            Self::NoRequiredSource => "evidence manifest has no required source",
            Self::ObservationCountOutOfRange => {
                "evidence manifest observation count is out of range"
            }
            Self::UnknownObservationSource => {
                "evidence manifest observation references an unknown source"
            }
            Self::DuplicateObservation => "evidence manifest contains a duplicate observation",
            Self::ConflictingEvent => {
                "evidence manifest contains conflicting records for one event"
            }
            Self::SourceEventCountMismatch => {
                "evidence manifest source event count does not match its observations"
            }
            Self::ManifestTooLarge => "evidence manifest exceeds its canonical byte limit",
            Self::Malformed => "evidence manifest encoding is malformed",
            Self::UnsupportedVersion => "evidence manifest version is unsupported",
            Self::NonCanonical => "evidence manifest encoding is not canonical",
        })
    }
}

#[cfg(feature = "std")]
impl std::error::Error for RadrootsTradeEvidenceManifestError {}

fn collect_bounded<T>(
    values: impl IntoIterator<Item = T>,
    maximum: usize,
    error: RadrootsTradeEvidenceManifestError,
) -> Result<Vec<T>, RadrootsTradeEvidenceManifestError> {
    let mut collected = Vec::new();
    for value in values {
        if collected.len() == maximum {
            return Err(error);
        }
        collected.push(value);
    }
    Ok(collected)
}

fn map_coverage_error(
    error: RadrootsTradeEvidenceCoverageError,
) -> RadrootsTradeEvidenceManifestError {
    match error {
        RadrootsTradeEvidenceCoverageError::SourceCountOutOfRange => {
            RadrootsTradeEvidenceManifestError::SourceCountOutOfRange
        }
        RadrootsTradeEvidenceCoverageError::NoRequiredSource => {
            RadrootsTradeEvidenceManifestError::NoRequiredSource
        }
        RadrootsTradeEvidenceCoverageError::AdmittedEventCountOutOfRange => {
            RadrootsTradeEvidenceManifestError::SourceEventCountMismatch
        }
    }
}

fn encode_manifest(
    trade_id: &TradeId,
    trade_generation: NonZeroU64,
    evidence_policy_digest: RadrootsTradeEvidencePolicyDigestV1,
    observed_at_unix_s: u64,
    scope_prerequisites: RadrootsTradeEvidenceScopePrerequisitesV1,
    sources: &[RadrootsTradeEvidenceManifestSourceResultV1],
    observations: &[RadrootsTradeEvidenceManifestObservationV1],
) -> Result<Vec<u8>, RadrootsTradeEvidenceManifestError> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(MANIFEST_PREFIX);
    bytes.extend_from_slice(&RADROOTS_TRADE_EVIDENCE_MANIFEST_CONTRACT_VERSION.to_be_bytes());
    bytes.extend_from_slice(trade_id.as_bytes());
    bytes.extend_from_slice(&trade_generation.get().to_be_bytes());
    bytes.extend_from_slice(evidence_policy_digest.as_bytes());
    bytes.extend_from_slice(&observed_at_unix_s.to_be_bytes());
    bytes.push(encode_scope_prerequisites(scope_prerequisites));
    bytes.push(
        u8::try_from(sources.len())
            .map_err(|_| RadrootsTradeEvidenceManifestError::SourceCountOutOfRange)?,
    );
    for source in sources {
        bytes.push(
            u8::try_from(source.source_id.as_str().len())
                .map_err(|_| RadrootsTradeEvidenceManifestError::InvalidSourceId)?,
        );
        bytes.extend_from_slice(source.source_id.as_str().as_bytes());
        bytes.push(encode_requirement(source.result.requirement()));
        bytes.push(encode_completion(source.result.completion()));
        bytes.extend_from_slice(&source.result.admitted_event_count().to_be_bytes());
        bytes.extend_from_slice(source.result_digest.as_bytes());
    }
    bytes.extend_from_slice(
        &u32::try_from(observations.len())
            .map_err(|_| RadrootsTradeEvidenceManifestError::ObservationCountOutOfRange)?
            .to_be_bytes(),
    );
    for observation in observations {
        let source_index = sources
            .binary_search_by(|source| source.source_id.cmp(&observation.source_id))
            .map_err(|_| RadrootsTradeEvidenceManifestError::UnknownObservationSource)?;
        bytes.push(
            u8::try_from(source_index)
                .map_err(|_| RadrootsTradeEvidenceManifestError::SourceCountOutOfRange)?,
        );
        bytes.extend_from_slice(observation.mutation_id.as_bytes());
        bytes.extend_from_slice(observation.event_id.as_bytes());
        bytes.extend_from_slice(observation.signed_event_digest.as_bytes());
        bytes.extend_from_slice(observation.provenance_digest.as_bytes());
    }
    if bytes.len() > RADROOTS_TRADE_EVIDENCE_MANIFEST_MAXIMUM_BYTES {
        return Err(RadrootsTradeEvidenceManifestError::ManifestTooLarge);
    }
    Ok(bytes)
}

fn manifest_digest(
    bytes: &[u8],
) -> Result<RadrootsTradeEvidenceManifestDigestV1, RadrootsTradeEvidenceManifestError> {
    let length = u64::try_from(bytes.len())
        .map_err(|_| RadrootsTradeEvidenceManifestError::ManifestTooLarge)?;
    let mut hasher = Sha256::new();
    hasher.update(MANIFEST_DIGEST_DOMAIN);
    hasher.update(length.to_be_bytes());
    hasher.update(bytes);
    Ok(RadrootsTradeEvidenceManifestDigestV1(
        hasher.finalize().into(),
    ))
}

const fn encode_scope_prerequisites(value: RadrootsTradeEvidenceScopePrerequisitesV1) -> u8 {
    match value {
        RadrootsTradeEvidenceScopePrerequisitesV1::Satisfied => 0,
        RadrootsTradeEvidenceScopePrerequisitesV1::Unsatisfied => 1,
    }
}

fn decode_scope_prerequisites(
    value: u8,
) -> Result<RadrootsTradeEvidenceScopePrerequisitesV1, RadrootsTradeEvidenceManifestError> {
    match value {
        0 => Ok(RadrootsTradeEvidenceScopePrerequisitesV1::Satisfied),
        1 => Ok(RadrootsTradeEvidenceScopePrerequisitesV1::Unsatisfied),
        _ => Err(RadrootsTradeEvidenceManifestError::Malformed),
    }
}

const fn encode_requirement(value: RadrootsTradeEvidenceSourceRequirementV1) -> u8 {
    match value {
        RadrootsTradeEvidenceSourceRequirementV1::Required => 0,
        RadrootsTradeEvidenceSourceRequirementV1::Optional => 1,
    }
}

fn decode_requirement(
    value: u8,
) -> Result<RadrootsTradeEvidenceSourceRequirementV1, RadrootsTradeEvidenceManifestError> {
    match value {
        0 => Ok(RadrootsTradeEvidenceSourceRequirementV1::Required),
        1 => Ok(RadrootsTradeEvidenceSourceRequirementV1::Optional),
        _ => Err(RadrootsTradeEvidenceManifestError::Malformed),
    }
}

const fn encode_completion(value: RadrootsTradeEvidenceSourceCompletionV1) -> u8 {
    match value {
        RadrootsTradeEvidenceSourceCompletionV1::Complete => 0,
        RadrootsTradeEvidenceSourceCompletionV1::Incomplete => 1,
        RadrootsTradeEvidenceSourceCompletionV1::Unsupported => 2,
    }
}

fn decode_completion(
    value: u8,
) -> Result<RadrootsTradeEvidenceSourceCompletionV1, RadrootsTradeEvidenceManifestError> {
    match value {
        0 => Ok(RadrootsTradeEvidenceSourceCompletionV1::Complete),
        1 => Ok(RadrootsTradeEvidenceSourceCompletionV1::Incomplete),
        2 => Ok(RadrootsTradeEvidenceSourceCompletionV1::Unsupported),
        _ => Err(RadrootsTradeEvidenceManifestError::Malformed),
    }
}

fn decode_source_id(
    reader: &mut ManifestReader<'_>,
) -> Result<RadrootsTradeEvidenceSourceIdV1, RadrootsTradeEvidenceManifestError> {
    let length = usize::from(reader.u8()?);
    if length == 0 || length > RADROOTS_TRADE_EVIDENCE_SOURCE_ID_MAXIMUM_BYTES {
        return Err(RadrootsTradeEvidenceManifestError::InvalidSourceId);
    }
    let value = core::str::from_utf8(reader.take(length)?)
        .map_err(|_| RadrootsTradeEvidenceManifestError::InvalidSourceId)?;
    RadrootsTradeEvidenceSourceIdV1::parse(value)
}

struct ManifestReader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> ManifestReader<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn take(&mut self, length: usize) -> Result<&'a [u8], RadrootsTradeEvidenceManifestError> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or(RadrootsTradeEvidenceManifestError::Malformed)?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or(RadrootsTradeEvidenceManifestError::Malformed)?;
        self.offset = end;
        Ok(value)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], RadrootsTradeEvidenceManifestError> {
        self.take(N)?
            .try_into()
            .map_err(|_| RadrootsTradeEvidenceManifestError::Malformed)
    }

    fn u8(&mut self) -> Result<u8, RadrootsTradeEvidenceManifestError> {
        Ok(self.array::<1>()?[0])
    }

    fn u16(&mut self) -> Result<u16, RadrootsTradeEvidenceManifestError> {
        Ok(u16::from_be_bytes(self.array()?))
    }

    fn u32(&mut self) -> Result<u32, RadrootsTradeEvidenceManifestError> {
        Ok(u32::from_be_bytes(self.array()?))
    }

    fn u64(&mut self) -> Result<u64, RadrootsTradeEvidenceManifestError> {
        Ok(u64::from_be_bytes(self.array()?))
    }

    fn finish(self) -> Result<(), RadrootsTradeEvidenceManifestError> {
        if self.offset == self.bytes.len() {
            Ok(())
        } else {
            Err(RadrootsTradeEvidenceManifestError::NonCanonical)
        }
    }
}

#[cfg(test)]
mod tests {
    use alloc::{format, string::ToString, vec};

    use super::*;

    fn source_id(value: &str) -> RadrootsTradeEvidenceSourceIdV1 {
        RadrootsTradeEvidenceSourceIdV1::parse(value).unwrap()
    }

    fn policy_digest(byte: u8) -> RadrootsTradeEvidencePolicyDigestV1 {
        RadrootsTradeEvidencePolicyDigestV1::from_bytes([byte; 32])
    }

    fn source_result_digest(byte: u8) -> RadrootsTradeEvidenceSourceResultDigestV1 {
        RadrootsTradeEvidenceSourceResultDigestV1::from_bytes([byte; 32])
    }

    fn signed_event_digest(byte: u8) -> RadrootsTradeSignedEventDigestV1 {
        RadrootsTradeSignedEventDigestV1::from_bytes([byte; 32])
    }

    fn provenance_digest(byte: u8) -> RadrootsTradeEvidenceProvenanceDigestV1 {
        RadrootsTradeEvidenceProvenanceDigestV1::from_bytes([byte; 32])
    }

    fn event_id(byte: u8) -> EventId {
        EventId::from_bytes([byte; 32])
    }

    fn mutation_id(byte: u8) -> MutationId {
        MutationId::from_bytes([byte; 32])
    }

    fn ordinal_ids(ordinal: u32) -> (MutationId, EventId) {
        let mut mutation = [0_u8; 32];
        mutation[28..].copy_from_slice(&ordinal.to_be_bytes());
        let mut event = mutation;
        event[0] = 1;
        (MutationId::from_bytes(mutation), EventId::from_bytes(event))
    }

    fn source(
        id: &str,
        requirement: RadrootsTradeEvidenceSourceRequirementV1,
        completion: RadrootsTradeEvidenceSourceCompletionV1,
        count: u32,
        digest_byte: u8,
    ) -> RadrootsTradeEvidenceManifestSourceResultV1 {
        RadrootsTradeEvidenceManifestSourceResultV1::new(
            source_id(id),
            RadrootsTradeEvidenceSourceResultV1::new(requirement, completion, count).unwrap(),
            source_result_digest(digest_byte),
        )
    }

    fn observation(
        source: &str,
        mutation: u8,
        event: u8,
        signed: u8,
        provenance: u8,
    ) -> RadrootsTradeEvidenceManifestObservationV1 {
        RadrootsTradeEvidenceManifestObservationV1::new(
            source_id(source),
            mutation_id(mutation),
            event_id(event),
            signed_event_digest(signed),
            provenance_digest(provenance),
        )
    }

    fn manifest() -> RadrootsTradeEvidenceManifestV1 {
        RadrootsTradeEvidenceManifestV1::new(
            TradeId::from_bytes([0x11; 16]),
            NonZeroU64::new(7).unwrap(),
            policy_digest(0x22),
            1_700_000_000,
            RadrootsTradeEvidenceScopePrerequisitesV1::Satisfied,
            [
                source(
                    "source-b",
                    RadrootsTradeEvidenceSourceRequirementV1::Optional,
                    RadrootsTradeEvidenceSourceCompletionV1::Incomplete,
                    1,
                    0x42,
                ),
                source(
                    "source-a",
                    RadrootsTradeEvidenceSourceRequirementV1::Required,
                    RadrootsTradeEvidenceSourceCompletionV1::Complete,
                    1,
                    0x41,
                ),
            ],
            [
                observation("source-b", 0x32, 0x52, 0x62, 0x72),
                observation("source-a", 0x31, 0x51, 0x61, 0x71),
            ],
        )
        .unwrap()
    }

    #[test]
    fn exact_manifest_vector_and_digest_are_frozen() {
        let manifest = manifest();
        assert_eq!(
            hex::encode(manifest.canonical_bytes()),
            "726164726f6f74732e74726164652e65766964656e63652d6d616e69666573742e76310000011111111111111111111111111111111100000000000000072222222222222222222222222222222222222222222222222222222222222222000000006553f100000208736f757263652d61000000000001414141414141414141414141414141414141414141414141414141414141414108736f757263652d62010100000001424242424242424242424242424242424242424242424242424242424242424200000002003131313131313131313131313131313131313131313131313131313131313131515151515151515151515151515151515151515151515151515151515151515161616161616161616161616161616161616161616161616161616161616161617171717171717171717171717171717171717171717171717171717171717171013232323232323232323232323232323232323232323232323232323232323232525252525252525252525252525252525252525252525252525252525252525262626262626262626262626262626262626262626262626262626262626262627272727272727272727272727272727272727272727272727272727272727272"
        );
        assert_eq!(
            manifest.digest().to_hex(),
            "ec5e2cdc85107afe43c90925b695fe9cd8605970886f1d2a2d431b602adeb3a9"
        );
        assert_eq!(
            manifest.contract_id(),
            RADROOTS_TRADE_EVIDENCE_MANIFEST_CONTRACT_ID
        );
        assert_eq!(manifest.contract_version(), 1);
        assert_eq!(
            manifest.coverage(),
            RadrootsTradeEvidenceCoverageV1::ScopeSatisfied
        );
    }

    #[test]
    fn manifest_components_expose_exact_typed_evidence() {
        let manifest = manifest();
        let source = &manifest.sources()[0];
        let observation = &manifest.observations()[0];

        assert_eq!(source.source_id().as_str(), "source-a");
        assert_eq!(
            source.result().requirement(),
            RadrootsTradeEvidenceSourceRequirementV1::Required
        );
        assert_eq!(
            source.result().completion(),
            RadrootsTradeEvidenceSourceCompletionV1::Complete
        );
        assert_eq!(source.result().admitted_event_count(), 1);
        assert_eq!(source.result_digest().as_bytes(), &[0x41; 32]);

        assert_eq!(observation.source_id().as_str(), "source-a");
        assert_eq!(observation.mutation_id().as_bytes(), &[0x31; 32]);
        assert_eq!(observation.event_id().as_bytes(), &[0x51; 32]);
        assert_eq!(observation.signed_event_digest().as_bytes(), &[0x61; 32]);
        assert_eq!(observation.provenance_digest().as_bytes(), &[0x71; 32]);
        assert_eq!(manifest.digest().as_bytes().len(), 32);

        assert_eq!(
            RadrootsTradeEvidencePolicyDigestV1::sha256(b"policy").as_bytes(),
            RadrootsTradeEvidencePolicyDigestV1::from_bytes(Sha256::digest(b"policy").into())
                .as_bytes()
        );
        assert_eq!(
            RadrootsTradeEvidenceSourceResultDigestV1::sha256(b"result").as_bytes(),
            RadrootsTradeEvidenceSourceResultDigestV1::from_bytes(Sha256::digest(b"result").into())
                .as_bytes()
        );
        assert_eq!(
            RadrootsTradeSignedEventDigestV1::sha256(b"event").as_bytes(),
            RadrootsTradeSignedEventDigestV1::from_bytes(Sha256::digest(b"event").into())
                .as_bytes()
        );
        assert_eq!(
            RadrootsTradeEvidenceProvenanceDigestV1::sha256(b"provenance").as_bytes(),
            RadrootsTradeEvidenceProvenanceDigestV1::from_bytes(
                Sha256::digest(b"provenance").into()
            )
            .as_bytes()
        );
    }

    #[test]
    fn construction_is_permutation_invariant_and_parser_is_strict() {
        let first = manifest();
        let second = RadrootsTradeEvidenceManifestV1::new(
            *first.trade_id(),
            first.trade_generation(),
            first.evidence_policy_digest(),
            first.observed_at_unix_s(),
            first.scope_prerequisites(),
            first.sources().iter().cloned().rev(),
            first.observations().iter().cloned().rev(),
        )
        .unwrap();
        assert_eq!(first, second);
        assert_eq!(
            RadrootsTradeEvidenceManifestV1::from_canonical_bytes(first.canonical_bytes()),
            Ok(first.clone())
        );

        let mut trailing = first.canonical_bytes().to_vec();
        trailing.push(0);
        assert_eq!(
            RadrootsTradeEvidenceManifestV1::from_canonical_bytes(&trailing),
            Err(RadrootsTradeEvidenceManifestError::NonCanonical)
        );
        let mut version = first.canonical_bytes().to_vec();
        version[MANIFEST_PREFIX.len() + 1] = 2;
        assert_eq!(
            RadrootsTradeEvidenceManifestV1::from_canonical_bytes(&version),
            Err(RadrootsTradeEvidenceManifestError::UnsupportedVersion)
        );
        for length in 0..first.canonical_bytes().len() {
            assert!(
                RadrootsTradeEvidenceManifestV1::from_canonical_bytes(
                    &first.canonical_bytes()[..length]
                )
                .is_err()
            );
        }
    }

    #[test]
    fn manifest_rejects_unknown_duplicate_and_mismatched_inventory() {
        let required = source(
            "required",
            RadrootsTradeEvidenceSourceRequirementV1::Required,
            RadrootsTradeEvidenceSourceCompletionV1::Complete,
            1,
            1,
        );
        let base = |sources: Vec<_>, observations: Vec<_>| {
            RadrootsTradeEvidenceManifestV1::new(
                TradeId::from_bytes([1; 16]),
                NonZeroU64::new(1).unwrap(),
                policy_digest(2),
                3,
                RadrootsTradeEvidenceScopePrerequisitesV1::Satisfied,
                sources,
                observations,
            )
        };
        assert_eq!(
            base(
                vec![required.clone()],
                vec![observation("other", 1, 2, 3, 4)]
            ),
            Err(RadrootsTradeEvidenceManifestError::UnknownObservationSource)
        );
        assert_eq!(
            base(vec![required.clone(), required.clone()], vec![]),
            Err(RadrootsTradeEvidenceManifestError::DuplicateSource)
        );
        let observed = observation("required", 1, 2, 3, 4);
        assert_eq!(
            base(vec![required.clone()], vec![observed.clone(), observed]),
            Err(RadrootsTradeEvidenceManifestError::DuplicateObservation)
        );
        let conflicting = RadrootsTradeEvidenceManifestObservationV1::new(
            source_id("required"),
            mutation_id(1),
            event_id(2),
            signed_event_digest(9),
            provenance_digest(10),
        );
        assert_eq!(
            base(
                vec![required.clone()],
                vec![observation("required", 1, 2, 3, 4), conflicting]
            ),
            Err(RadrootsTradeEvidenceManifestError::ConflictingEvent)
        );
        let optional = source(
            "optional",
            RadrootsTradeEvidenceSourceRequirementV1::Optional,
            RadrootsTradeEvidenceSourceCompletionV1::Complete,
            1,
            2,
        );
        assert_eq!(
            base(
                vec![required.clone(), optional],
                vec![
                    observation("required", 1, 2, 3, 4),
                    observation("optional", 9, 2, 3, 5),
                ]
            ),
            Err(RadrootsTradeEvidenceManifestError::ConflictingEvent)
        );
        assert_eq!(
            base(vec![required], vec![]),
            Err(RadrootsTradeEvidenceManifestError::SourceEventCountMismatch)
        );
    }

    #[test]
    fn source_identity_and_inventory_bounds_are_exact() {
        for invalid in ["", "Upper", "-prefix", "has space", "a/b"] {
            assert_eq!(
                RadrootsTradeEvidenceSourceIdV1::parse(invalid),
                Err(RadrootsTradeEvidenceManifestError::InvalidSourceId)
            );
        }
        let maximum = format!("a{}", "1".repeat(63));
        assert_eq!(source_id(&maximum).as_str(), maximum);
        assert_eq!(
            RadrootsTradeEvidenceSourceIdV1::parse(format!("a{}", "1".repeat(64))),
            Err(RadrootsTradeEvidenceManifestError::InvalidSourceId)
        );

        let required = source(
            "required",
            RadrootsTradeEvidenceSourceRequirementV1::Required,
            RadrootsTradeEvidenceSourceCompletionV1::Incomplete,
            0,
            1,
        );
        fn create(
            sources: impl IntoIterator<Item = RadrootsTradeEvidenceManifestSourceResultV1>,
        ) -> Result<RadrootsTradeEvidenceManifestV1, RadrootsTradeEvidenceManifestError> {
            RadrootsTradeEvidenceManifestV1::new(
                TradeId::from_bytes([1; 16]),
                NonZeroU64::new(1).unwrap(),
                policy_digest(2),
                3,
                RadrootsTradeEvidenceScopePrerequisitesV1::Unsatisfied,
                sources,
                core::iter::empty(),
            )
        }
        assert_eq!(
            create(core::iter::empty()),
            Err(RadrootsTradeEvidenceManifestError::SourceCountOutOfRange)
        );
        assert_eq!(
            create(core::iter::repeat(required)),
            Err(RadrootsTradeEvidenceManifestError::SourceCountOutOfRange)
        );
        assert_eq!(
            create([source(
                "optional",
                RadrootsTradeEvidenceSourceRequirementV1::Optional,
                RadrootsTradeEvidenceSourceCompletionV1::Incomplete,
                0,
                1,
            )]),
            Err(RadrootsTradeEvidenceManifestError::NoRequiredSource)
        );
    }

    #[test]
    fn observation_and_canonical_input_bounds_are_exact() {
        let sources = (0..RADROOTS_TRADE_EVIDENCE_MAXIMUM_SOURCE_COUNT).map(|index| {
            source(
                &format!("source-{index:02}"),
                if index == 0 {
                    RadrootsTradeEvidenceSourceRequirementV1::Required
                } else {
                    RadrootsTradeEvidenceSourceRequirementV1::Optional
                },
                RadrootsTradeEvidenceSourceCompletionV1::Complete,
                RADROOTS_TRADE_EVIDENCE_MAXIMUM_EVENTS_PER_SOURCE,
                u8::try_from(index).unwrap(),
            )
        });
        let observations =
            (0..RADROOTS_TRADE_EVIDENCE_MANIFEST_MAXIMUM_OBSERVATIONS).map(|index| {
                let source_index = index
                    / usize::try_from(RADROOTS_TRADE_EVIDENCE_MAXIMUM_EVENTS_PER_SOURCE).unwrap();
                let (mutation_id, event_id) = ordinal_ids(u32::try_from(index).unwrap());
                RadrootsTradeEvidenceManifestObservationV1::new(
                    source_id(&format!("source-{source_index:02}")),
                    mutation_id,
                    event_id,
                    signed_event_digest(1),
                    provenance_digest(2),
                )
            });
        let maximum = RadrootsTradeEvidenceManifestV1::new(
            TradeId::from_bytes([1; 16]),
            NonZeroU64::new(1).unwrap(),
            policy_digest(2),
            3,
            RadrootsTradeEvidenceScopePrerequisitesV1::Satisfied,
            sources,
            observations,
        )
        .unwrap();
        assert_eq!(
            maximum.observations().len(),
            RADROOTS_TRADE_EVIDENCE_MANIFEST_MAXIMUM_OBSERVATIONS
        );
        assert_eq!(
            RadrootsTradeEvidenceManifestV1::from_canonical_bytes(maximum.canonical_bytes()),
            Ok(maximum)
        );

        let required = source(
            "required",
            RadrootsTradeEvidenceSourceRequirementV1::Required,
            RadrootsTradeEvidenceSourceCompletionV1::Incomplete,
            0,
            1,
        );
        assert_eq!(
            RadrootsTradeEvidenceManifestV1::new(
                TradeId::from_bytes([1; 16]),
                NonZeroU64::new(1).unwrap(),
                policy_digest(2),
                3,
                RadrootsTradeEvidenceScopePrerequisitesV1::Unsatisfied,
                [required],
                core::iter::repeat(observation("required", 1, 2, 3, 4)),
            ),
            Err(RadrootsTradeEvidenceManifestError::ObservationCountOutOfRange)
        );
        assert_eq!(
            RadrootsTradeEvidenceManifestV1::from_canonical_bytes(&vec![
                0;
                RADROOTS_TRADE_EVIDENCE_MANIFEST_MAXIMUM_BYTES
                    + 1
            ]),
            Err(RadrootsTradeEvidenceManifestError::ManifestTooLarge)
        );
    }

    #[test]
    fn debug_and_errors_do_not_expose_evidence_identity() {
        let manifest = manifest();
        let debug = format!("{manifest:?}");
        for secret in ["source-a", "source-b", &manifest.trade_id().to_string()] {
            assert!(!debug.contains(secret));
        }
        assert_eq!(
            format!("{:?}", manifest.sources()[0]),
            "RadrootsTradeEvidenceManifestSourceResultV1(<redacted>)"
        );
        assert_eq!(
            format!("{:?}", manifest.observations()[0]),
            "RadrootsTradeEvidenceManifestObservationV1(<redacted>)"
        );
        for error in [
            RadrootsTradeEvidenceManifestError::InvalidSourceId,
            RadrootsTradeEvidenceManifestError::SourceCountOutOfRange,
            RadrootsTradeEvidenceManifestError::DuplicateSource,
            RadrootsTradeEvidenceManifestError::NoRequiredSource,
            RadrootsTradeEvidenceManifestError::ObservationCountOutOfRange,
            RadrootsTradeEvidenceManifestError::UnknownObservationSource,
            RadrootsTradeEvidenceManifestError::DuplicateObservation,
            RadrootsTradeEvidenceManifestError::ConflictingEvent,
            RadrootsTradeEvidenceManifestError::SourceEventCountMismatch,
            RadrootsTradeEvidenceManifestError::ManifestTooLarge,
            RadrootsTradeEvidenceManifestError::Malformed,
            RadrootsTradeEvidenceManifestError::UnsupportedVersion,
            RadrootsTradeEvidenceManifestError::NonCanonical,
        ] {
            assert!(!error.to_string().is_empty());
            #[cfg(feature = "std")]
            assert!(std::error::Error::source(&error).is_none());
        }
    }
}
