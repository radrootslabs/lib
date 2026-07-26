//! Sealed media-readiness binding for Phase 1 publication artifacts.
//!
//! This module binds transport-neutral Blossom observations to an already
//! allowlisted artifact. It does not perform HTTP requests, retain BUD-11
//! authorization, or grant entitlement.

#[cfg(not(feature = "std"))]
use alloc::{
    boxed::Box,
    string::{String, ToString},
    vec,
    vec::Vec,
};
use core::{fmt, marker::PhantomData};
#[cfg(feature = "std")]
use std::{
    boxed::Box,
    string::{String, ToString},
    vec::Vec,
};

use radroots_blossom::{
    RADROOTS_BLOSSOM_PUBLICATION_READINESS_EVIDENCE_MAX_BYTES,
    RADROOTS_BLOSSOM_PUBLICATION_READINESS_POLICY_VERSION,
    RadrootsBlossomPublicationReadinessEvidence, RadrootsBlossomRasterDimensions,
    RadrootsBlossomRasterFormat,
};
use serde::{
    Deserialize, Deserializer, Serialize,
    de::{Error as _, SeqAccess, Visitor},
};
use serde_json::value::RawValue;
use sha2::{Digest, Sha256};

use crate::{
    food_availability::inbound::{
        RadrootsFoodAvailabilityProjectionOutcome,
        registry_v7::project_inbound_food_availability_parts,
    },
    post::inbound::registry_v7::project_inbound_post_parts,
};

use super::{
    RADROOTS_PHASE1_PUBLICATION_MEDIA_MAX_COUNT, RadrootsPhase1PublicationArtifact,
    RadrootsPhase1PublicationMediaReference, RadrootsPhase1PublicationSemanticVariant,
    allowlist::RadrootsPhase1AllowlistedPublicationArtifact,
};

pub const RADROOTS_PHASE1_PUBLICATION_MEDIA_READINESS_BINDING_SCHEMA_VERSION: u32 = 1;

const BINDING_WIRE_CEILING_BYTES: usize = 4 * 1024 * 1024;
const BINDING_WIRE_FIXED_BYTES: usize = br#"{"schema_version":1,"readiness_policy_version":1,"artifact_digest":"","evidence":[],"binding_digest":""}"#.len()
    + 64
    + 64;
const BINDING_WIRE_FORMULA_MAX_BYTES: usize = BINDING_WIRE_FIXED_BYTES
    + RADROOTS_PHASE1_PUBLICATION_MEDIA_MAX_COUNT
        * RADROOTS_BLOSSOM_PUBLICATION_READINESS_EVIDENCE_MAX_BYTES
    + RADROOTS_PHASE1_PUBLICATION_MEDIA_MAX_COUNT.saturating_sub(1);

pub const RADROOTS_PHASE1_PUBLICATION_MEDIA_READINESS_BINDING_MAX_BYTES: usize =
    if BINDING_WIRE_FORMULA_MAX_BYTES < BINDING_WIRE_CEILING_BYTES {
        BINDING_WIRE_FORMULA_MAX_BYTES
    } else {
        BINDING_WIRE_CEILING_BYTES
    };

const _: () = assert!(
    RADROOTS_PHASE1_PUBLICATION_MEDIA_READINESS_BINDING_MAX_BYTES <= BINDING_WIRE_CEILING_BYTES
);
const BINDING_DIGEST_DOMAIN: &[u8] = b"radroots.phase1.publication-media-readiness.v1\0";

/// Domain-separated identity for an artifact's exact ordered readiness set.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RadrootsPhase1PublicationMediaReadinessBindingDigest([u8; 32]);

impl RadrootsPhase1PublicationMediaReadinessBindingDigest {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub fn to_hex(self) -> String {
        hex::encode(self.0)
    }

    fn parse(value: &str) -> Result<Self, RadrootsPhase1PublicationMediaReadinessError> {
        if value.len() != 64 {
            return Err(RadrootsPhase1PublicationMediaReadinessError::InvalidDigest);
        }
        let mut bytes = [0_u8; 32];
        for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
            let high = lowercase_hex_nibble(pair[0])
                .ok_or(RadrootsPhase1PublicationMediaReadinessError::InvalidDigest)?;
            let low = lowercase_hex_nibble(pair[1])
                .ok_or(RadrootsPhase1PublicationMediaReadinessError::InvalidDigest)?;
            bytes[index] = (high << 4) | low;
        }
        Ok(Self(bytes))
    }
}

const fn lowercase_hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}

impl fmt::Display for RadrootsPhase1PublicationMediaReadinessBindingDigest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.to_hex())
    }
}

/// An allowlisted artifact with one sealed readiness observation per media URL.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsPhase1MediaReadyPublicationArtifact {
    allowlisted_artifact: RadrootsPhase1AllowlistedPublicationArtifact,
    evidence: Vec<RadrootsBlossomPublicationReadinessEvidence>,
    binding_digest: RadrootsPhase1PublicationMediaReadinessBindingDigest,
    canonical_json: Vec<u8>,
}

impl RadrootsPhase1MediaReadyPublicationArtifact {
    pub fn allowlisted_artifact(&self) -> &RadrootsPhase1AllowlistedPublicationArtifact {
        &self.allowlisted_artifact
    }

    pub fn artifact(&self) -> &RadrootsPhase1PublicationArtifact {
        self.allowlisted_artifact.artifact()
    }

    pub fn evidence(&self) -> &[RadrootsBlossomPublicationReadinessEvidence] {
        &self.evidence
    }

    pub const fn binding_digest(&self) -> RadrootsPhase1PublicationMediaReadinessBindingDigest {
        self.binding_digest
    }

    pub fn canonical_json(&self) -> &[u8] {
        &self.canonical_json
    }

    pub fn to_canonical_json(&self) -> Vec<u8> {
        self.canonical_json.clone()
    }

    pub fn into_allowlisted_artifact(self) -> RadrootsPhase1AllowlistedPublicationArtifact {
        self.allowlisted_artifact
    }

    /// Reloads canonical binding bytes against a separately revalidated artifact.
    pub fn from_canonical_json(
        allowlisted_artifact: RadrootsPhase1AllowlistedPublicationArtifact,
        bytes: &[u8],
    ) -> Result<Self, RadrootsPhase1PublicationMediaReadinessError> {
        if bytes.len() > RADROOTS_PHASE1_PUBLICATION_MEDIA_READINESS_BINDING_MAX_BYTES {
            return Err(
                RadrootsPhase1PublicationMediaReadinessError::BindingTooLarge {
                    max: RADROOTS_PHASE1_PUBLICATION_MEDIA_READINESS_BINDING_MAX_BYTES,
                    actual: bytes.len(),
                },
            );
        }
        let wire: BindingWire<'_> = serde_json::from_slice(bytes)
            .map_err(|_| RadrootsPhase1PublicationMediaReadinessError::InvalidJson)?;
        if wire.schema_version != RADROOTS_PHASE1_PUBLICATION_MEDIA_READINESS_BINDING_SCHEMA_VERSION
        {
            return Err(
                RadrootsPhase1PublicationMediaReadinessError::UnsupportedSchemaVersion {
                    expected: RADROOTS_PHASE1_PUBLICATION_MEDIA_READINESS_BINDING_SCHEMA_VERSION,
                    actual: wire.schema_version,
                },
            );
        }
        if wire.readiness_policy_version != RADROOTS_BLOSSOM_PUBLICATION_READINESS_POLICY_VERSION {
            return Err(
                RadrootsPhase1PublicationMediaReadinessError::UnsupportedReadinessPolicyVersion {
                    expected: RADROOTS_BLOSSOM_PUBLICATION_READINESS_POLICY_VERSION,
                    actual: wire.readiness_policy_version,
                },
            );
        }
        let expected_artifact_digest = allowlisted_artifact.artifact().artifact_digest();
        if wire.artifact_digest != expected_artifact_digest.to_hex() {
            return Err(RadrootsPhase1PublicationMediaReadinessError::ArtifactDigestMismatch);
        }
        let claimed_digest =
            RadrootsPhase1PublicationMediaReadinessBindingDigest::parse(wire.binding_digest)?;
        let mut evidence = Vec::new();
        evidence
            .try_reserve_exact(wire.evidence.len())
            .map_err(|_| RadrootsPhase1PublicationMediaReadinessError::AllocationFailed)?;
        for raw in wire.evidence {
            evidence.push(
                RadrootsBlossomPublicationReadinessEvidence::from_canonical_json(
                    raw.get().as_bytes(),
                )
                .map_err(|_| RadrootsPhase1PublicationMediaReadinessError::EvidenceInvalid)?,
            );
        }
        let ready = build_media_readiness_binding(allowlisted_artifact, evidence)?;
        if ready.binding_digest != claimed_digest {
            return Err(RadrootsPhase1PublicationMediaReadinessError::DigestMismatch);
        }
        if ready.canonical_json != bytes {
            return Err(RadrootsPhase1PublicationMediaReadinessError::NonCanonicalJson);
        }
        Ok(ready)
    }
}

/// Binds one sealed observation to every canonical media URL in artifact order.
pub fn bind_phase1_publication_media_readiness<I>(
    allowlisted_artifact: RadrootsPhase1AllowlistedPublicationArtifact,
    evidence: I,
) -> Result<RadrootsPhase1MediaReadyPublicationArtifact, RadrootsPhase1PublicationMediaReadinessError>
where
    I: IntoIterator<Item = RadrootsBlossomPublicationReadinessEvidence>,
{
    let mut bounded = Vec::new();
    let mut evidence = evidence.into_iter();
    let (lower_bound, _) = evidence.size_hint();
    bounded
        .try_reserve_exact(lower_bound.min(RADROOTS_PHASE1_PUBLICATION_MEDIA_MAX_COUNT))
        .map_err(|_| RadrootsPhase1PublicationMediaReadinessError::AllocationFailed)?;
    for item in &mut evidence {
        if bounded.len() == RADROOTS_PHASE1_PUBLICATION_MEDIA_MAX_COUNT {
            return Err(
                RadrootsPhase1PublicationMediaReadinessError::EvidenceCountExceeded {
                    max: RADROOTS_PHASE1_PUBLICATION_MEDIA_MAX_COUNT,
                    actual: bounded.len() + 1,
                },
            );
        }
        bounded.push(item);
    }
    build_media_readiness_binding(allowlisted_artifact, bounded)
}

/// Revalidates an in-memory media-ready typestate through canonical reload.
pub fn validate_phase1_publication_media_readiness(
    ready: &RadrootsPhase1MediaReadyPublicationArtifact,
) -> Result<(), RadrootsPhase1PublicationMediaReadinessError> {
    let reloaded = RadrootsPhase1MediaReadyPublicationArtifact::from_canonical_json(
        ready.allowlisted_artifact.clone(),
        &ready.canonical_json,
    )?;
    if &reloaded != ready {
        return Err(RadrootsPhase1PublicationMediaReadinessError::StateMismatch);
    }
    Ok(())
}

fn build_media_readiness_binding(
    allowlisted_artifact: RadrootsPhase1AllowlistedPublicationArtifact,
    evidence: Vec<RadrootsBlossomPublicationReadinessEvidence>,
) -> Result<RadrootsPhase1MediaReadyPublicationArtifact, RadrootsPhase1PublicationMediaReadinessError>
{
    validate_evidence_parity(allowlisted_artifact.artifact(), &evidence)?;
    let binding_digest = compute_binding_digest(allowlisted_artifact.artifact(), &evidence);
    let canonical_json =
        serialize_binding(allowlisted_artifact.artifact(), &evidence, binding_digest)?;
    if canonical_json.len() > RADROOTS_PHASE1_PUBLICATION_MEDIA_READINESS_BINDING_MAX_BYTES {
        return Err(
            RadrootsPhase1PublicationMediaReadinessError::BindingTooLarge {
                max: RADROOTS_PHASE1_PUBLICATION_MEDIA_READINESS_BINDING_MAX_BYTES,
                actual: canonical_json.len(),
            },
        );
    }
    Ok(RadrootsPhase1MediaReadyPublicationArtifact {
        allowlisted_artifact,
        evidence,
        binding_digest,
        canonical_json,
    })
}

fn validate_evidence_parity(
    artifact: &RadrootsPhase1PublicationArtifact,
    evidence: &[RadrootsBlossomPublicationReadinessEvidence],
) -> Result<(), RadrootsPhase1PublicationMediaReadinessError> {
    let media = artifact.media_references();
    if evidence.len() != media.len() {
        return Err(
            RadrootsPhase1PublicationMediaReadinessError::EvidenceCountMismatch {
                expected: media.len(),
                actual: evidence.len(),
            },
        );
    }
    let dimensions = expected_dimensions(artifact)?;
    for (index, ((reference, observation), expected_dimensions)) in
        media.iter().zip(evidence).zip(dimensions).enumerate()
    {
        if observation.url() != reference.url() {
            return Err(
                RadrootsPhase1PublicationMediaReadinessError::EvidenceOrderMismatch { index },
            );
        }
        let expected_format = RadrootsBlossomRasterFormat::from_media_type(reference.media_type())
            .map_err(
                |_| RadrootsPhase1PublicationMediaReadinessError::EvidenceFactMismatch { index },
            )?;
        if observation.sha256() != reference.sha256()
            || observation.size() != reference.size()
            || observation.media_type() != reference.media_type()
            || observation.raster_format() != expected_format
        {
            return Err(
                RadrootsPhase1PublicationMediaReadinessError::EvidenceFactMismatch { index },
            );
        }
        if expected_dimensions.is_some_and(|expected| observation.dimensions() != expected) {
            return Err(
                RadrootsPhase1PublicationMediaReadinessError::EvidenceDimensionMismatch { index },
            );
        }
    }
    Ok(())
}

fn expected_dimensions(
    artifact: &RadrootsPhase1PublicationArtifact,
) -> Result<
    Vec<Option<RadrootsBlossomRasterDimensions>>,
    RadrootsPhase1PublicationMediaReadinessError,
> {
    match artifact.semantic_variant() {
        RadrootsPhase1PublicationSemanticVariant::PhotoUpdate
        | RadrootsPhase1PublicationSemanticVariant::Ask => post_dimensions(artifact),
        RadrootsPhase1PublicationSemanticVariant::FoodAvailability => food_dimensions(artifact),
        RadrootsPhase1PublicationSemanticVariant::Profile
        | RadrootsPhase1PublicationSemanticVariant::Update
        | RadrootsPhase1PublicationSemanticVariant::Event(_) => {
            Ok(vec![None; artifact.media_references().len()])
        }
    }
}

fn post_dimensions(
    artifact: &RadrootsPhase1PublicationArtifact,
) -> Result<
    Vec<Option<RadrootsBlossomRasterDimensions>>,
    RadrootsPhase1PublicationMediaReadinessError,
> {
    let draft = artifact.draft();
    let projection = project_inbound_post_parts(draft.kind(), draft.tags(), draft.content())
        .map_err(|_| RadrootsPhase1PublicationMediaReadinessError::ArtifactProfileInvalid)?;
    if !projection.diagnostics().is_empty() {
        return Err(RadrootsPhase1PublicationMediaReadinessError::ArtifactProfileInvalid);
    }
    let mut entries = Vec::new();
    for imeta in projection.imeta() {
        let url = imeta
            .url()
            .ok_or(RadrootsPhase1PublicationMediaReadinessError::ArtifactProfileInvalid)?;
        let dimensions = imeta
            .dimensions()
            .ok_or(RadrootsPhase1PublicationMediaReadinessError::ArtifactProfileInvalid)?;
        let dimensions =
            RadrootsBlossomRasterDimensions::new(dimensions.width(), dimensions.height()).map_err(
                |_| RadrootsPhase1PublicationMediaReadinessError::ArtifactProfileInvalid,
            )?;
        entries.push((url.to_string(), dimensions));
        entries.extend(
            imeta
                .fallbacks()
                .iter()
                .cloned()
                .map(|fallback| (fallback, dimensions)),
        );
    }
    align_dimensions(artifact.media_references(), entries)
}

fn food_dimensions(
    artifact: &RadrootsPhase1PublicationArtifact,
) -> Result<
    Vec<Option<RadrootsBlossomRasterDimensions>>,
    RadrootsPhase1PublicationMediaReadinessError,
> {
    let draft = artifact.draft();
    let tags = radroots_event::RadrootsEventTags::new(draft.tags().to_vec())
        .map_err(|_| RadrootsPhase1PublicationMediaReadinessError::ArtifactProfileInvalid)?;
    let projection = project_inbound_food_availability_parts(
        draft.kind(),
        draft.created_at(),
        &tags,
        draft.content(),
    )
    .map_err(|_| RadrootsPhase1PublicationMediaReadinessError::ArtifactProfileInvalid)?;
    let RadrootsFoodAvailabilityProjectionOutcome::Focused(projection) = projection else {
        return Err(RadrootsPhase1PublicationMediaReadinessError::ArtifactProfileInvalid);
    };
    if !projection.diagnostics().is_empty() {
        return Err(RadrootsPhase1PublicationMediaReadinessError::ArtifactProfileInvalid);
    }
    let mut entries = Vec::new();
    for image in projection.images() {
        let url = image
            .url()
            .ok_or(RadrootsPhase1PublicationMediaReadinessError::ArtifactProfileInvalid)?;
        let dimensions = image
            .dimensions()
            .ok_or(RadrootsPhase1PublicationMediaReadinessError::ArtifactProfileInvalid)?;
        let dimensions =
            RadrootsBlossomRasterDimensions::new(dimensions.width(), dimensions.height()).map_err(
                |_| RadrootsPhase1PublicationMediaReadinessError::ArtifactProfileInvalid,
            )?;
        entries.push((url.to_string(), dimensions));
    }
    align_dimensions(artifact.media_references(), entries)
}

fn align_dimensions(
    media: &[RadrootsPhase1PublicationMediaReference],
    mut entries: Vec<(String, RadrootsBlossomRasterDimensions)>,
) -> Result<
    Vec<Option<RadrootsBlossomRasterDimensions>>,
    RadrootsPhase1PublicationMediaReadinessError,
> {
    entries.sort_by(|left, right| left.0.cmp(&right.0));
    if entries.windows(2).any(|pair| pair[0].0 == pair[1].0) {
        return Err(RadrootsPhase1PublicationMediaReadinessError::ArtifactProfileInvalid);
    }
    if entries.len() != media.len() {
        return Err(RadrootsPhase1PublicationMediaReadinessError::ArtifactProfileInvalid);
    }
    media
        .iter()
        .zip(entries)
        .map(|(reference, (url, dimensions))| {
            if reference.url().as_str() != url {
                return Err(RadrootsPhase1PublicationMediaReadinessError::ArtifactProfileInvalid);
            }
            Ok(Some(dimensions))
        })
        .collect()
}

fn compute_binding_digest(
    artifact: &RadrootsPhase1PublicationArtifact,
    evidence: &[RadrootsBlossomPublicationReadinessEvidence],
) -> RadrootsPhase1PublicationMediaReadinessBindingDigest {
    let mut hasher = Sha256::new();
    hasher.update(BINDING_DIGEST_DOMAIN);
    hasher.update(RADROOTS_PHASE1_PUBLICATION_MEDIA_READINESS_BINDING_SCHEMA_VERSION.to_be_bytes());
    hasher.update(RADROOTS_BLOSSOM_PUBLICATION_READINESS_POLICY_VERSION.to_be_bytes());
    hasher.update(artifact.artifact_digest().as_bytes());
    hasher.update((evidence.len() as u32).to_be_bytes());
    for item in evidence {
        let url = item.url().as_str().as_bytes();
        hasher.update((url.len() as u64).to_be_bytes());
        hasher.update(url);
        hasher.update(item.evidence_digest().as_sha256().as_bytes());
    }
    RadrootsPhase1PublicationMediaReadinessBindingDigest(hasher.finalize().into())
}

fn serialize_binding(
    artifact: &RadrootsPhase1PublicationArtifact,
    evidence: &[RadrootsBlossomPublicationReadinessEvidence],
    binding_digest: RadrootsPhase1PublicationMediaReadinessBindingDigest,
) -> Result<Vec<u8>, RadrootsPhase1PublicationMediaReadinessError> {
    let mut raw_evidence = Vec::new();
    raw_evidence
        .try_reserve_exact(evidence.len())
        .map_err(|_| RadrootsPhase1PublicationMediaReadinessError::AllocationFailed)?;
    let mut canonical_len = BINDING_WIRE_FIXED_BYTES
        .checked_add(evidence.len().saturating_sub(1))
        .ok_or(RadrootsPhase1PublicationMediaReadinessError::Serialization)?;
    let mut binding_too_large = false;
    for item in evidence {
        let canonical = item
            .to_canonical_json()
            .map_err(|_| RadrootsPhase1PublicationMediaReadinessError::EvidenceInvalid)?;
        if canonical.len() > RADROOTS_BLOSSOM_PUBLICATION_READINESS_EVIDENCE_MAX_BYTES {
            return Err(RadrootsPhase1PublicationMediaReadinessError::EvidenceInvalid);
        }
        canonical_len = canonical_len
            .checked_add(canonical.len())
            .ok_or(RadrootsPhase1PublicationMediaReadinessError::Serialization)?;
        if canonical_len <= RADROOTS_PHASE1_PUBLICATION_MEDIA_READINESS_BINDING_MAX_BYTES {
            let canonical = String::from_utf8(canonical)
                .map_err(|_| RadrootsPhase1PublicationMediaReadinessError::EvidenceInvalid)?;
            raw_evidence.push(
                RawValue::from_string(canonical)
                    .map_err(|_| RadrootsPhase1PublicationMediaReadinessError::EvidenceInvalid)?,
            );
        } else {
            binding_too_large = true;
        }
    }
    if binding_too_large {
        return Err(
            RadrootsPhase1PublicationMediaReadinessError::BindingTooLarge {
                max: RADROOTS_PHASE1_PUBLICATION_MEDIA_READINESS_BINDING_MAX_BYTES,
                actual: canonical_len,
            },
        );
    }
    serde_json::to_vec(&BindingSerializeWire {
        schema_version: RADROOTS_PHASE1_PUBLICATION_MEDIA_READINESS_BINDING_SCHEMA_VERSION,
        readiness_policy_version: RADROOTS_BLOSSOM_PUBLICATION_READINESS_POLICY_VERSION,
        artifact_digest: artifact.artifact_digest().to_hex(),
        evidence: raw_evidence.iter().map(Box::as_ref).collect(),
        binding_digest: binding_digest.to_hex(),
    })
    .map_err(|_| RadrootsPhase1PublicationMediaReadinessError::Serialization)
}

#[derive(Serialize)]
struct BindingSerializeWire<'a> {
    schema_version: u32,
    readiness_policy_version: u16,
    artifact_digest: String,
    evidence: Vec<&'a RawValue>,
    binding_digest: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BindingWire<'a> {
    schema_version: u32,
    readiness_policy_version: u16,
    artifact_digest: &'a str,
    #[serde(borrow, deserialize_with = "deserialize_bounded_evidence")]
    evidence: Vec<&'a RawValue>,
    binding_digest: &'a str,
}

fn deserialize_bounded_evidence<'de, D>(deserializer: D) -> Result<Vec<&'de RawValue>, D::Error>
where
    D: Deserializer<'de>,
{
    struct BoundedEvidenceVisitor<'de>(PhantomData<&'de RawValue>);

    impl<'de> Visitor<'de> for BoundedEvidenceVisitor<'de> {
        type Value = Vec<&'de RawValue>;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(
                formatter,
                "at most {RADROOTS_PHASE1_PUBLICATION_MEDIA_MAX_COUNT} readiness evidence items"
            )
        }

        fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
        where
            A: SeqAccess<'de>,
        {
            let mut values = Vec::new();
            values
                .try_reserve_exact(
                    sequence
                        .size_hint()
                        .unwrap_or(0)
                        .min(RADROOTS_PHASE1_PUBLICATION_MEDIA_MAX_COUNT),
                )
                .map_err(|_| A::Error::custom("readiness evidence allocation failed"))?;
            while let Some(value) = sequence.next_element::<&'de RawValue>()? {
                if values.len() == RADROOTS_PHASE1_PUBLICATION_MEDIA_MAX_COUNT {
                    return Err(A::Error::custom("readiness evidence count exceeds maximum"));
                }
                values.push(value);
            }
            Ok(values)
        }
    }

    deserializer.deserialize_seq(BoundedEvidenceVisitor(PhantomData))
}

#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsPhase1PublicationMediaReadinessError {
    BindingTooLarge { max: usize, actual: usize },
    EvidenceCountExceeded { max: usize, actual: usize },
    EvidenceCountMismatch { expected: usize, actual: usize },
    InvalidJson,
    NonCanonicalJson,
    UnsupportedSchemaVersion { expected: u32, actual: u32 },
    UnsupportedReadinessPolicyVersion { expected: u16, actual: u16 },
    ArtifactDigestMismatch,
    ArtifactProfileInvalid,
    EvidenceInvalid,
    EvidenceOrderMismatch { index: usize },
    EvidenceFactMismatch { index: usize },
    EvidenceDimensionMismatch { index: usize },
    InvalidDigest,
    DigestMismatch,
    StateMismatch,
    AllocationFailed,
    Serialization,
}

impl RadrootsPhase1PublicationMediaReadinessError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::BindingTooLarge { .. } => "publication_media_readiness_binding_too_large",
            Self::EvidenceCountExceeded { .. } => {
                "publication_media_readiness_evidence_count_exceeded"
            }
            Self::EvidenceCountMismatch { .. } => {
                "publication_media_readiness_evidence_count_mismatch"
            }
            Self::InvalidJson => "publication_media_readiness_invalid_json",
            Self::NonCanonicalJson => "publication_media_readiness_non_canonical_json",
            Self::UnsupportedSchemaVersion { .. } => {
                "publication_media_readiness_schema_version_unsupported"
            }
            Self::UnsupportedReadinessPolicyVersion { .. } => {
                "publication_media_readiness_policy_version_unsupported"
            }
            Self::ArtifactDigestMismatch => "publication_media_readiness_artifact_digest_mismatch",
            Self::ArtifactProfileInvalid => "publication_media_readiness_artifact_profile_invalid",
            Self::EvidenceInvalid => "publication_media_readiness_evidence_invalid",
            Self::EvidenceOrderMismatch { .. } => {
                "publication_media_readiness_evidence_order_mismatch"
            }
            Self::EvidenceFactMismatch { .. } => {
                "publication_media_readiness_evidence_fact_mismatch"
            }
            Self::EvidenceDimensionMismatch { .. } => {
                "publication_media_readiness_evidence_dimension_mismatch"
            }
            Self::InvalidDigest => "publication_media_readiness_digest_invalid",
            Self::DigestMismatch => "publication_media_readiness_digest_mismatch",
            Self::StateMismatch => "publication_media_readiness_state_mismatch",
            Self::AllocationFailed => "publication_media_readiness_allocation_failed",
            Self::Serialization => "publication_media_readiness_serialization",
        }
    }
}

impl fmt::Display for RadrootsPhase1PublicationMediaReadinessError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BindingTooLarge { max, actual } => write!(
                formatter,
                "publication media-readiness binding is {actual} bytes; maximum is {max}"
            ),
            Self::EvidenceCountExceeded { max, actual } => write!(
                formatter,
                "publication media-readiness evidence count is {actual}; maximum is {max}"
            ),
            Self::EvidenceCountMismatch { expected, actual } => write!(
                formatter,
                "publication media-readiness evidence count must be {expected}, got {actual}"
            ),
            error => formatter.write_str(error.code()),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for RadrootsPhase1PublicationMediaReadinessError {}
