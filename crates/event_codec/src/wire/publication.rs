//! Sealed persistence boundary for the Radroots Phase 1 publication set.
//!
//! Artifacts can only be created from strict authored models or reloaded from
//! their exact canonical JSON bytes. Reloading restores a persisted artifact;
//! it does not restore byte-verification, upload, signing, or authenticity
//! capabilities.
//!
//! The digest detects accidental corruption, payload-only modification, and
//! digest-only modification. It does not authenticate against an actor able to
//! rewrite both the payload and digest, replace this validator, compromise the
//! signer, or control the host/database. NIP-01 id and signature verification
//! remain the cryptographic authenticity boundary.

pub mod allowlist;

#[cfg(not(feature = "std"))]
use alloc::{
    format,
    string::{String, ToString},
    vec,
    vec::Vec,
};
use core::fmt;

use radroots_blossom::{
    RadrootsBlossomApprovedBlobUrl, RadrootsBlossomByteVerifiedDescriptor,
    RadrootsBlossomMediaType, RadrootsBlossomSha256, url::RadrootsBlossomBlobUrl,
};
use radroots_event::{
    RadrootsEventTags,
    calendar::{
        RadrootsAdmittedCalendarDateEvent, RadrootsAdmittedCalendarTimeEvent,
        RadrootsAuthoredCalendarDateEvent, RadrootsAuthoredCalendarTimeEvent,
        RadrootsParsedNip52CalendarCommon,
    },
    food_availability::{RADROOTS_FOOD_AVAILABILITY_CONTRACT_ID, RadrootsFoodAvailabilityDetails},
    ids::{RadrootsEventId, RadrootsPublicKey},
    kinds::{
        KIND_CALENDAR_DATE_EVENT, KIND_CALENDAR_TIME_EVENT, KIND_CLASSIFIED_LISTING, KIND_POST,
        KIND_PROFILE,
    },
    post::{RadrootsAuthoredAsk, RadrootsAuthoredPhotoUpdate, RadrootsAuthoredUpdate},
    profile::{
        RADROOTS_PROFILE_METADATA_MAX_CONTENT_BYTES, RadrootsAuthoredProfile,
        RadrootsNip05Identifier,
    },
    wire::{
        DEFAULT_CONTENT_MAX_BYTES, DEFAULT_RAW_JSON_MAX_BYTES, RadrootsNip01EventWireParts,
        compute_canonical_nip01_event_id,
    },
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    calendar::{decode, encode},
    food_availability::{authored as food_authored, inbound as food_inbound},
    post::{authored as post_authored, inbound as post_inbound},
    profile::authored as profile_authored,
};

pub const RADROOTS_PHASE1_PUBLICATION_ARTIFACT_SCHEMA_VERSION: u32 = 1;
pub const RADROOTS_PHASE1_PUBLICATION_ARTIFACT_MAX_BYTES: usize = 2 * 1024 * 1024;
pub const RADROOTS_PHASE1_PUBLICATION_MEDIA_MAX_COUNT: usize = 4096;
pub const RADROOTS_PHASE1_PUBLICATION_SIGNED_EVENT_MAX_BYTES: usize = DEFAULT_RAW_JSON_MAX_BYTES;

const ARTIFACT_DIGEST_DOMAIN: &[u8] = b"radroots.phase1.publication-artifact.v1";
const ARTIFACT_DIGEST_DOMAIN_TERMINATOR: &[u8] = b"\0";
const PROFILE_OPERATION_ID: &str = "profile.build_authored_draft";
const PROFILE_CONTRACT_ID: &str = "radroots.profile.metadata.v1";
const UPDATE_OPERATION_ID: &str = "social.update.build_authored_draft";
const UPDATE_CONTRACT_ID: &str = "radroots.social.update.v1";
const PHOTO_UPDATE_OPERATION_ID: &str = "social.photo_update.build_authored_draft";
const PHOTO_UPDATE_CONTRACT_ID: &str = "radroots.social.photo_update.v1";
const ASK_OPERATION_ID: &str = "social.ask.build_authored_draft";
const ASK_CONTRACT_ID: &str = "radroots.social.ask.v1";
const CALENDAR_DATE_OPERATION_ID: &str = "social.calendar_date_event.build_authored_draft";
const CALENDAR_DATE_CONTRACT_ID: &str = "radroots.calendar.date_event.v1";
const CALENDAR_TIME_OPERATION_ID: &str = "social.calendar_time_event.build_authored_draft";
const CALENDAR_TIME_CONTRACT_ID: &str = "radroots.calendar.time_event.v1";
const FOOD_OPERATION_ID: &str = "food_availability.build_authored_draft";

/// The two strict NIP-52 event forms admitted by the Phase 1 Event root.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadrootsPhase1PublicationEventVariant {
    Date,
    Time,
}

/// Closed semantic set represented by a Phase 1 publication artifact.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadrootsPhase1PublicationSemanticVariant {
    Profile,
    Update,
    PhotoUpdate,
    Ask,
    Event(RadrootsPhase1PublicationEventVariant),
    FoodAvailability,
}

impl RadrootsPhase1PublicationSemanticVariant {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Profile => "profile",
            Self::Update => "update",
            Self::PhotoUpdate => "photo_update",
            Self::Ask => "ask",
            Self::Event(RadrootsPhase1PublicationEventVariant::Date) => "event_date",
            Self::Event(RadrootsPhase1PublicationEventVariant::Time) => "event_time",
            Self::FoodAvailability => "food_availability",
        }
    }

    fn parse(value: &str) -> Result<Self, RadrootsPhase1PublicationArtifactError> {
        match value {
            "profile" => Ok(Self::Profile),
            "update" => Ok(Self::Update),
            "photo_update" => Ok(Self::PhotoUpdate),
            "ask" => Ok(Self::Ask),
            "event_date" => Ok(Self::Event(RadrootsPhase1PublicationEventVariant::Date)),
            "event_time" => Ok(Self::Event(RadrootsPhase1PublicationEventVariant::Time)),
            "food_availability" => Ok(Self::FoodAvailability),
            _ => Err(RadrootsPhase1PublicationArtifactError::UnknownSemanticVariant),
        }
    }

    pub const fn authored_operation_id(self) -> &'static str {
        profile_for_variant(self).operation_id
    }

    pub const fn event_contract_id(self) -> &'static str {
        profile_for_variant(self).contract_id
    }

    pub const fn kind(self) -> u32 {
        profile_for_variant(self).kind
    }
}

#[derive(Clone, Copy)]
struct PublicationProfile {
    operation_id: &'static str,
    contract_id: &'static str,
    kind: u32,
}

const fn profile_for_variant(
    variant: RadrootsPhase1PublicationSemanticVariant,
) -> PublicationProfile {
    match variant {
        RadrootsPhase1PublicationSemanticVariant::Profile => PublicationProfile {
            operation_id: PROFILE_OPERATION_ID,
            contract_id: PROFILE_CONTRACT_ID,
            kind: KIND_PROFILE,
        },
        RadrootsPhase1PublicationSemanticVariant::Update => PublicationProfile {
            operation_id: UPDATE_OPERATION_ID,
            contract_id: UPDATE_CONTRACT_ID,
            kind: KIND_POST,
        },
        RadrootsPhase1PublicationSemanticVariant::PhotoUpdate => PublicationProfile {
            operation_id: PHOTO_UPDATE_OPERATION_ID,
            contract_id: PHOTO_UPDATE_CONTRACT_ID,
            kind: KIND_POST,
        },
        RadrootsPhase1PublicationSemanticVariant::Ask => PublicationProfile {
            operation_id: ASK_OPERATION_ID,
            contract_id: ASK_CONTRACT_ID,
            kind: KIND_POST,
        },
        RadrootsPhase1PublicationSemanticVariant::Event(
            RadrootsPhase1PublicationEventVariant::Date,
        ) => PublicationProfile {
            operation_id: CALENDAR_DATE_OPERATION_ID,
            contract_id: CALENDAR_DATE_CONTRACT_ID,
            kind: KIND_CALENDAR_DATE_EVENT,
        },
        RadrootsPhase1PublicationSemanticVariant::Event(
            RadrootsPhase1PublicationEventVariant::Time,
        ) => PublicationProfile {
            operation_id: CALENDAR_TIME_OPERATION_ID,
            contract_id: CALENDAR_TIME_CONTRACT_ID,
            kind: KIND_CALENDAR_TIME_EVENT,
        },
        RadrootsPhase1PublicationSemanticVariant::FoodAvailability => PublicationProfile {
            operation_id: FOOD_OPERATION_ID,
            contract_id: RADROOTS_FOOD_AVAILABILITY_CONTRACT_ID,
            kind: KIND_CLASSIFIED_LISTING,
        },
    }
}

/// Exact unsigned NIP-01 state frozen into a publication artifact.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsPhase1PublicationDraft {
    created_at: u64,
    kind: u32,
    tags: Vec<Vec<String>>,
    content: String,
}

impl RadrootsPhase1PublicationDraft {
    pub const fn created_at(&self) -> u64 {
        self.created_at
    }

    pub const fn kind(&self) -> u32 {
        self.kind
    }

    pub fn tags(&self) -> &[Vec<String>] {
        &self.tags
    }

    pub fn content(&self) -> &str {
        &self.content
    }
}

/// Persisted media commitment associated with one complete canonical URL.
///
/// This type preserves descriptor facts that entered through a byte-verified
/// authored model. Reloading it does not re-establish byte verification or
/// prove that the URL was uploaded or remains retrievable.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsPhase1PublicationMediaReference {
    url: RadrootsBlossomApprovedBlobUrl,
    sha256: RadrootsBlossomSha256,
    size: u64,
    media_type: RadrootsBlossomMediaType,
}

impl RadrootsPhase1PublicationMediaReference {
    pub fn url(&self) -> &RadrootsBlossomApprovedBlobUrl {
        &self.url
    }

    pub const fn sha256(&self) -> RadrootsBlossomSha256 {
        self.sha256
    }

    pub const fn size(&self) -> u64 {
        self.size
    }

    pub fn media_type(&self) -> &RadrootsBlossomMediaType {
        &self.media_type
    }

    fn from_verified(
        descriptor: &RadrootsBlossomByteVerifiedDescriptor,
        url: RadrootsBlossomApprovedBlobUrl,
    ) -> Self {
        Self {
            url,
            sha256: descriptor.sha256(),
            size: descriptor.size(),
            media_type: descriptor.media_type().clone(),
        }
    }

    fn from_wire(wire: MediaReferenceWire) -> Result<Self, RadrootsPhase1PublicationArtifactError> {
        let parsed_url = RadrootsBlossomBlobUrl::parse(&wire.url)
            .and_then(RadrootsBlossomBlobUrl::approve)
            .map_err(|_| RadrootsPhase1PublicationArtifactError::InvalidMediaReference)?;
        if parsed_url.as_str() != wire.url {
            return Err(RadrootsPhase1PublicationArtifactError::InvalidMediaReference);
        }
        let sha256 = RadrootsBlossomSha256::from_hex(&wire.sha256)
            .map_err(|_| RadrootsPhase1PublicationArtifactError::InvalidMediaReference)?;
        if parsed_url.as_blob_url().hash_path().hash() != sha256 {
            return Err(RadrootsPhase1PublicationArtifactError::InvalidMediaReference);
        }
        let media_type = RadrootsBlossomMediaType::parse(&wire.media_type)
            .map_err(|_| RadrootsPhase1PublicationArtifactError::InvalidMediaReference)?;
        if media_type.as_str() != wire.media_type || !wire.media_type.starts_with("image/") {
            return Err(RadrootsPhase1PublicationArtifactError::InvalidMediaReference);
        }
        Ok(Self {
            url: parsed_url,
            sha256,
            size: wire.size,
            media_type,
        })
    }

    fn to_wire(&self) -> MediaReferenceWire {
        MediaReferenceWire {
            url: self.url.as_str().to_string(),
            sha256: self.sha256.to_hex(),
            size: self.size,
            media_type: self.media_type.as_str().to_string(),
        }
    }
}

/// Domain-separated SHA-256 identity for the canonical artifact payload.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RadrootsPhase1PublicationArtifactDigest([u8; 32]);

impl RadrootsPhase1PublicationArtifactDigest {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub fn to_hex(self) -> String {
        hex::encode(self.0)
    }

    fn parse(value: &str) -> Result<Self, RadrootsPhase1PublicationArtifactError> {
        if value.len() != 64
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        {
            return Err(RadrootsPhase1PublicationArtifactError::InvalidDigest);
        }
        let bytes = hex::decode(value)
            .map_err(|_| RadrootsPhase1PublicationArtifactError::InvalidDigest)?;
        let value: [u8; 32] = bytes
            .try_into()
            .map_err(|_| RadrootsPhase1PublicationArtifactError::InvalidDigest)?;
        Ok(Self(value))
    }
}

impl fmt::Display for RadrootsPhase1PublicationArtifactDigest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.to_hex())
    }
}

/// A versioned artifact whose exact canonical bytes are ready for persistence.
///
/// There is deliberately no constructor from `RadrootsEventDraft`, arbitrary
/// wire parts, raw JSON, numeric kind, or a signed event.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsPhase1PublicationArtifact {
    semantic_variant: RadrootsPhase1PublicationSemanticVariant,
    expected_author: RadrootsPublicKey,
    draft: RadrootsPhase1PublicationDraft,
    expected_event_id: RadrootsEventId,
    media_references: Vec<RadrootsPhase1PublicationMediaReference>,
    artifact_digest: RadrootsPhase1PublicationArtifactDigest,
    canonical_json: Vec<u8>,
}

impl RadrootsPhase1PublicationArtifact {
    pub fn from_profile(
        profile: &RadrootsAuthoredProfile,
        created_at: u64,
        expected_author: impl AsRef<str>,
    ) -> Result<Self, RadrootsPhase1PublicationArtifactError> {
        let parts = profile_authored::authored_profile_to_wire_parts(profile)
            .map_err(|error| encoding_error(error.code()))?;
        let mut media = Vec::with_capacity(2);
        for image in [profile.picture(), profile.banner()].into_iter().flatten() {
            let descriptor = image.descriptor();
            media.push(RadrootsPhase1PublicationMediaReference::from_verified(
                descriptor,
                descriptor.url().clone(),
            ));
        }
        Self::from_authored_parts(
            RadrootsPhase1PublicationSemanticVariant::Profile,
            created_at,
            expected_author.as_ref(),
            parts,
            media,
        )
    }

    pub fn from_update(
        update: &RadrootsAuthoredUpdate,
        created_at: u64,
        expected_author: impl AsRef<str>,
    ) -> Result<Self, RadrootsPhase1PublicationArtifactError> {
        Self::from_authored_parts(
            RadrootsPhase1PublicationSemanticVariant::Update,
            created_at,
            expected_author.as_ref(),
            post_authored::authored_update_to_wire_parts(update),
            Vec::new(),
        )
    }

    pub fn from_photo_update(
        photo_update: &RadrootsAuthoredPhotoUpdate,
        created_at: u64,
        expected_author: impl AsRef<str>,
    ) -> Result<Self, RadrootsPhase1PublicationArtifactError> {
        Self::from_authored_parts(
            RadrootsPhase1PublicationSemanticVariant::PhotoUpdate,
            created_at,
            expected_author.as_ref(),
            post_authored::authored_photo_update_to_wire_parts(photo_update),
            post_media_references(photo_update.images()),
        )
    }

    pub fn from_ask(
        ask: &RadrootsAuthoredAsk,
        created_at: u64,
        expected_author: impl AsRef<str>,
    ) -> Result<Self, RadrootsPhase1PublicationArtifactError> {
        Self::from_authored_parts(
            RadrootsPhase1PublicationSemanticVariant::Ask,
            created_at,
            expected_author.as_ref(),
            post_authored::authored_ask_to_wire_parts(ask),
            post_media_references(ask.images()),
        )
    }

    pub fn from_calendar_date_event(
        event: &RadrootsAuthoredCalendarDateEvent,
        created_at: u64,
        expected_author: impl AsRef<str>,
    ) -> Result<Self, RadrootsPhase1PublicationArtifactError> {
        let parts =
            encode::date_to_wire_parts(event).map_err(|error| encoding_error(error.code()))?;
        Self::from_authored_parts(
            RadrootsPhase1PublicationSemanticVariant::Event(
                RadrootsPhase1PublicationEventVariant::Date,
            ),
            created_at,
            expected_author.as_ref(),
            parts,
            authored_image_reference(event.image()),
        )
    }

    pub fn from_calendar_time_event(
        event: &RadrootsAuthoredCalendarTimeEvent,
        created_at: u64,
        expected_author: impl AsRef<str>,
    ) -> Result<Self, RadrootsPhase1PublicationArtifactError> {
        let parts =
            encode::time_to_wire_parts(event).map_err(|error| encoding_error(error.code()))?;
        Self::from_authored_parts(
            RadrootsPhase1PublicationSemanticVariant::Event(
                RadrootsPhase1PublicationEventVariant::Time,
            ),
            created_at,
            expected_author.as_ref(),
            parts,
            authored_image_reference(event.image()),
        )
    }

    pub fn from_food_availability(
        details: &RadrootsFoodAvailabilityDetails,
        created_at: u64,
        expected_author: impl AsRef<str>,
    ) -> Result<Self, RadrootsPhase1PublicationArtifactError> {
        let parts = food_authored::authored_food_availability_to_wire_parts(details, created_at)
            .map_err(|error| encoding_error(error.code()))?;
        let media = details
            .images()
            .iter()
            .map(|image| {
                let descriptor = image.image().descriptor();
                RadrootsPhase1PublicationMediaReference::from_verified(
                    descriptor,
                    descriptor.url().clone(),
                )
            })
            .collect();
        Self::from_authored_parts(
            RadrootsPhase1PublicationSemanticVariant::FoodAvailability,
            created_at,
            expected_author.as_ref(),
            parts,
            media,
        )
    }

    pub const fn schema_version(&self) -> u32 {
        RADROOTS_PHASE1_PUBLICATION_ARTIFACT_SCHEMA_VERSION
    }

    pub const fn semantic_variant(&self) -> RadrootsPhase1PublicationSemanticVariant {
        self.semantic_variant
    }

    pub const fn authored_operation_id(&self) -> &'static str {
        self.semantic_variant.authored_operation_id()
    }

    pub const fn event_contract_id(&self) -> &'static str {
        self.semantic_variant.event_contract_id()
    }

    pub fn expected_author(&self) -> &RadrootsPublicKey {
        &self.expected_author
    }

    pub fn draft(&self) -> &RadrootsPhase1PublicationDraft {
        &self.draft
    }

    pub fn expected_event_id(&self) -> &RadrootsEventId {
        &self.expected_event_id
    }

    pub fn media_references(&self) -> &[RadrootsPhase1PublicationMediaReference] {
        &self.media_references
    }

    pub const fn artifact_digest(&self) -> RadrootsPhase1PublicationArtifactDigest {
        self.artifact_digest
    }

    pub fn to_canonical_json(&self) -> Vec<u8> {
        self.canonical_json.clone()
    }

    pub fn from_canonical_json(
        bytes: &[u8],
    ) -> Result<Self, RadrootsPhase1PublicationArtifactError> {
        if bytes.len() > RADROOTS_PHASE1_PUBLICATION_ARTIFACT_MAX_BYTES {
            return Err(RadrootsPhase1PublicationArtifactError::ArtifactTooLarge {
                max: RADROOTS_PHASE1_PUBLICATION_ARTIFACT_MAX_BYTES,
                actual: bytes.len(),
            });
        }
        let wire: ArtifactWire = serde_json::from_slice(bytes)
            .map_err(|_| RadrootsPhase1PublicationArtifactError::InvalidJson)?;
        if wire.schema_version != RADROOTS_PHASE1_PUBLICATION_ARTIFACT_SCHEMA_VERSION {
            return Err(
                RadrootsPhase1PublicationArtifactError::UnsupportedSchemaVersion {
                    expected: RADROOTS_PHASE1_PUBLICATION_ARTIFACT_SCHEMA_VERSION,
                    actual: wire.schema_version,
                },
            );
        }
        let variant = RadrootsPhase1PublicationSemanticVariant::parse(&wire.semantic_variant)?;
        let profile = profile_for_variant(variant);
        if wire.authored_operation_id != profile.operation_id {
            return Err(RadrootsPhase1PublicationArtifactError::AuthoredOperationMismatch);
        }
        if wire.event_contract_id != profile.contract_id {
            return Err(RadrootsPhase1PublicationArtifactError::EventContractMismatch);
        }
        if wire.draft.kind != profile.kind {
            return Err(RadrootsPhase1PublicationArtifactError::KindMismatch {
                expected: profile.kind,
                actual: wire.draft.kind,
            });
        }
        let expected_author = parse_expected_author(&wire.expected_author)?;
        let expected_event_id = parse_expected_event_id(&wire.expected_event_id)?;
        let draft = RadrootsPhase1PublicationDraft {
            created_at: wire.draft.created_at,
            kind: wire.draft.kind,
            tags: wire.draft.tags,
            content: wire.draft.content,
        };
        validate_draft_identifier(&expected_author, &draft, &expected_event_id)?;
        validate_signed_event_wire_size(&expected_author, &draft, &expected_event_id)?;

        if wire.media_references.len() > RADROOTS_PHASE1_PUBLICATION_MEDIA_MAX_COUNT {
            return Err(
                RadrootsPhase1PublicationArtifactError::TooManyMediaReferences {
                    max: RADROOTS_PHASE1_PUBLICATION_MEDIA_MAX_COUNT,
                    actual: wire.media_references.len(),
                },
            );
        }
        let original_media_wire = wire.media_references;
        let mut media_references = original_media_wire
            .iter()
            .cloned()
            .map(RadrootsPhase1PublicationMediaReference::from_wire)
            .collect::<Result<Vec<_>, _>>()?;
        canonicalize_media_references(&mut media_references)?;
        if media_references
            .iter()
            .map(RadrootsPhase1PublicationMediaReference::to_wire)
            .ne(original_media_wire.iter().cloned())
        {
            return Err(RadrootsPhase1PublicationArtifactError::NonCanonicalMediaInventory);
        }
        validate_phase1_publication_profile(variant, &draft, &media_references)?;

        let artifact_digest =
            RadrootsPhase1PublicationArtifactDigest::parse(&wire.artifact_digest)?;
        let computed = compute_artifact_digest(
            variant,
            &expected_author,
            &draft,
            &expected_event_id,
            &media_references,
        )?;
        if computed != artifact_digest {
            return Err(RadrootsPhase1PublicationArtifactError::DigestMismatch);
        }
        let canonical_json = serialize_artifact(
            variant,
            &expected_author,
            &draft,
            &expected_event_id,
            &media_references,
            artifact_digest,
        )?;
        if canonical_json != bytes {
            return Err(RadrootsPhase1PublicationArtifactError::NonCanonicalJson);
        }
        Ok(Self {
            semantic_variant: variant,
            expected_author,
            draft,
            expected_event_id,
            media_references,
            artifact_digest,
            canonical_json,
        })
    }

    fn from_authored_parts(
        variant: RadrootsPhase1PublicationSemanticVariant,
        created_at: u64,
        expected_author: &str,
        parts: RadrootsNip01EventWireParts,
        mut media_references: Vec<RadrootsPhase1PublicationMediaReference>,
    ) -> Result<Self, RadrootsPhase1PublicationArtifactError> {
        let expected_author = parse_expected_author(expected_author)?;
        let profile = profile_for_variant(variant);
        if parts.kind != profile.kind {
            return Err(RadrootsPhase1PublicationArtifactError::KindMismatch {
                expected: profile.kind,
                actual: parts.kind,
            });
        }
        if media_references.len() > RADROOTS_PHASE1_PUBLICATION_MEDIA_MAX_COUNT {
            return Err(
                RadrootsPhase1PublicationArtifactError::TooManyMediaReferences {
                    max: RADROOTS_PHASE1_PUBLICATION_MEDIA_MAX_COUNT,
                    actual: media_references.len(),
                },
            );
        }
        canonicalize_media_references(&mut media_references)?;
        let expected_event_id = compute_canonical_nip01_event_id(
            expected_author.as_str(),
            created_at,
            parts.kind,
            &parts.tags,
            &parts.content,
        )
        .map_err(|_| RadrootsPhase1PublicationArtifactError::InvalidExpectedAuthor)?;
        let draft = RadrootsPhase1PublicationDraft {
            created_at,
            kind: parts.kind,
            tags: parts.tags,
            content: parts.content,
        };
        validate_signed_event_wire_size(&expected_author, &draft, &expected_event_id)?;
        validate_phase1_publication_profile(variant, &draft, &media_references)?;
        let artifact_digest = compute_artifact_digest(
            variant,
            &expected_author,
            &draft,
            &expected_event_id,
            &media_references,
        )?;
        let canonical_json = serialize_artifact(
            variant,
            &expected_author,
            &draft,
            &expected_event_id,
            &media_references,
            artifact_digest,
        )?;
        if canonical_json.len() > RADROOTS_PHASE1_PUBLICATION_ARTIFACT_MAX_BYTES {
            return Err(RadrootsPhase1PublicationArtifactError::ArtifactTooLarge {
                max: RADROOTS_PHASE1_PUBLICATION_ARTIFACT_MAX_BYTES,
                actual: canonical_json.len(),
            });
        }
        Ok(Self {
            semantic_variant: variant,
            expected_author,
            draft,
            expected_event_id,
            media_references,
            artifact_digest,
            canonical_json,
        })
    }
}

/// Revalidates a sealed artifact through the same strict persisted-byte path
/// used by later allowlist and outbox consumers.
pub fn validate_phase1_publication_artifact(
    artifact: &RadrootsPhase1PublicationArtifact,
) -> Result<(), RadrootsPhase1PublicationArtifactError> {
    let canonical_json = artifact.to_canonical_json();
    let reloaded = RadrootsPhase1PublicationArtifact::from_canonical_json(&canonical_json)?;
    if &reloaded != artifact {
        return Err(RadrootsPhase1PublicationArtifactError::ArtifactStateMismatch);
    }
    Ok(())
}

#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsPhase1PublicationArtifactError {
    ArtifactTooLarge { max: usize, actual: usize },
    EventWireTooLarge { max: usize, actual: usize },
    TooManyMediaReferences { max: usize, actual: usize },
    InvalidJson,
    NonCanonicalJson,
    UnsupportedSchemaVersion { expected: u32, actual: u32 },
    UnknownSemanticVariant,
    AuthoredOperationMismatch,
    EventContractMismatch,
    KindMismatch { expected: u32, actual: u32 },
    InvalidExpectedAuthor,
    InvalidExpectedEventId,
    ExpectedEventIdMismatch,
    InvalidDraft,
    InvalidProfile,
    InvalidPostProfile,
    InvalidCalendarProfile,
    InvalidFoodAvailabilityProfile,
    InvalidMediaReference,
    ConflictingMediaReference,
    NonCanonicalMediaInventory,
    MediaInventoryMismatch,
    InvalidDigest,
    DigestMismatch,
    ArtifactStateMismatch,
    AuthoredEncoding { code: &'static str },
    Serialization,
}

impl RadrootsPhase1PublicationArtifactError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::ArtifactTooLarge { .. } => "publication_artifact_too_large",
            Self::EventWireTooLarge { .. } => "publication_event_wire_too_large",
            Self::TooManyMediaReferences { .. } => "publication_media_count_exceeded",
            Self::InvalidJson => "publication_artifact_invalid_json",
            Self::NonCanonicalJson => "publication_artifact_non_canonical_json",
            Self::UnsupportedSchemaVersion { .. } => "publication_artifact_version_unsupported",
            Self::UnknownSemanticVariant => "publication_semantic_variant_unknown",
            Self::AuthoredOperationMismatch => "publication_authored_operation_mismatch",
            Self::EventContractMismatch => "publication_event_contract_mismatch",
            Self::KindMismatch { .. } => "publication_kind_mismatch",
            Self::InvalidExpectedAuthor => "publication_expected_author_invalid",
            Self::InvalidExpectedEventId => "publication_expected_event_id_invalid",
            Self::ExpectedEventIdMismatch => "publication_expected_event_id_mismatch",
            Self::InvalidDraft => "publication_draft_invalid",
            Self::InvalidProfile => "publication_profile_invalid",
            Self::InvalidPostProfile => "publication_post_profile_invalid",
            Self::InvalidCalendarProfile => "publication_calendar_profile_invalid",
            Self::InvalidFoodAvailabilityProfile => "publication_food_profile_invalid",
            Self::InvalidMediaReference => "publication_media_reference_invalid",
            Self::ConflictingMediaReference => "publication_media_reference_conflict",
            Self::NonCanonicalMediaInventory => "publication_media_inventory_non_canonical",
            Self::MediaInventoryMismatch => "publication_media_inventory_mismatch",
            Self::InvalidDigest => "publication_artifact_digest_invalid",
            Self::DigestMismatch => "publication_artifact_digest_mismatch",
            Self::ArtifactStateMismatch => "publication_artifact_state_mismatch",
            Self::AuthoredEncoding { code } => code,
            Self::Serialization => "publication_artifact_serialization",
        }
    }
}

impl fmt::Display for RadrootsPhase1PublicationArtifactError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ArtifactTooLarge { max, actual } => write!(
                formatter,
                "publication artifact is {actual} bytes; maximum is {max}"
            ),
            Self::EventWireTooLarge { max, actual } => write!(
                formatter,
                "publication event wire is {actual} bytes; maximum is {max}"
            ),
            Self::TooManyMediaReferences { max, actual } => write!(
                formatter,
                "publication artifact has {actual} media references; maximum is {max}"
            ),
            Self::UnsupportedSchemaVersion { expected, actual } => write!(
                formatter,
                "publication artifact schema version must be {expected}, got {actual}"
            ),
            Self::KindMismatch { expected, actual } => write!(
                formatter,
                "publication artifact kind must be {expected}, got {actual}"
            ),
            Self::AuthoredEncoding { code } => {
                write!(
                    formatter,
                    "strict authored encoding failed with code {code}"
                )
            }
            error => formatter.write_str(error.code()),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for RadrootsPhase1PublicationArtifactError {}

fn encoding_error(code: &'static str) -> RadrootsPhase1PublicationArtifactError {
    RadrootsPhase1PublicationArtifactError::AuthoredEncoding { code }
}

fn parse_expected_author(
    value: &str,
) -> Result<RadrootsPublicKey, RadrootsPhase1PublicationArtifactError> {
    let author = RadrootsPublicKey::parse(value)
        .map_err(|_| RadrootsPhase1PublicationArtifactError::InvalidExpectedAuthor)?;
    if author.as_str() != value {
        return Err(RadrootsPhase1PublicationArtifactError::InvalidExpectedAuthor);
    }
    Ok(author)
}

fn parse_expected_event_id(
    value: &str,
) -> Result<RadrootsEventId, RadrootsPhase1PublicationArtifactError> {
    let event_id = RadrootsEventId::parse(value)
        .map_err(|_| RadrootsPhase1PublicationArtifactError::InvalidExpectedEventId)?;
    if event_id.as_str() != value {
        return Err(RadrootsPhase1PublicationArtifactError::InvalidExpectedEventId);
    }
    Ok(event_id)
}

fn validate_draft_identifier(
    author: &RadrootsPublicKey,
    draft: &RadrootsPhase1PublicationDraft,
    expected_event_id: &RadrootsEventId,
) -> Result<(), RadrootsPhase1PublicationArtifactError> {
    let computed = compute_canonical_nip01_event_id(
        author.as_str(),
        draft.created_at,
        draft.kind,
        &draft.tags,
        &draft.content,
    )
    .map_err(|_| RadrootsPhase1PublicationArtifactError::InvalidDraft)?;
    if &computed != expected_event_id {
        return Err(RadrootsPhase1PublicationArtifactError::ExpectedEventIdMismatch);
    }
    Ok(())
}

#[derive(Serialize)]
struct SignedEventSizeWire<'a> {
    id: &'a str,
    pubkey: &'a str,
    created_at: u64,
    kind: u32,
    tags: &'a [Vec<String>],
    content: &'a str,
    sig: &'a str,
}

fn validate_signed_event_wire_size(
    author: &RadrootsPublicKey,
    draft: &RadrootsPhase1PublicationDraft,
    expected_event_id: &RadrootsEventId,
) -> Result<(), RadrootsPhase1PublicationArtifactError> {
    let actual = signed_event_wire_size(author, draft, expected_event_id)?;
    if actual > RADROOTS_PHASE1_PUBLICATION_SIGNED_EVENT_MAX_BYTES {
        return Err(RadrootsPhase1PublicationArtifactError::EventWireTooLarge {
            max: RADROOTS_PHASE1_PUBLICATION_SIGNED_EVENT_MAX_BYTES,
            actual,
        });
    }
    Ok(())
}

fn signed_event_wire_size(
    author: &RadrootsPublicKey,
    draft: &RadrootsPhase1PublicationDraft,
    expected_event_id: &RadrootsEventId,
) -> Result<usize, RadrootsPhase1PublicationArtifactError> {
    let signature = "0".repeat(128);
    Ok(serde_json::to_vec(&SignedEventSizeWire {
        id: expected_event_id.as_str(),
        pubkey: author.as_str(),
        created_at: draft.created_at,
        kind: draft.kind,
        tags: &draft.tags,
        content: &draft.content,
        sig: &signature,
    })
    .map_err(|_| RadrootsPhase1PublicationArtifactError::Serialization)?
    .len())
}

fn post_media_references(
    images: &[radroots_event::post::RadrootsAuthoredPostImage],
) -> Vec<RadrootsPhase1PublicationMediaReference> {
    let mut media = Vec::new();
    for image in images {
        let descriptor = image.image().descriptor();
        media.push(RadrootsPhase1PublicationMediaReference::from_verified(
            descriptor,
            descriptor.url().clone(),
        ));
        media.extend(image.fallbacks().iter().cloned().map(|fallback| {
            RadrootsPhase1PublicationMediaReference::from_verified(descriptor, fallback)
        }));
    }
    media
}

fn authored_image_reference(
    image: Option<&radroots_event::RadrootsAuthoredImage>,
) -> Vec<RadrootsPhase1PublicationMediaReference> {
    image
        .map(|image| {
            let descriptor = image.descriptor();
            vec![RadrootsPhase1PublicationMediaReference::from_verified(
                descriptor,
                descriptor.url().clone(),
            )]
        })
        .unwrap_or_default()
}

fn canonicalize_media_references(
    media: &mut Vec<RadrootsPhase1PublicationMediaReference>,
) -> Result<(), RadrootsPhase1PublicationArtifactError> {
    media.sort_by(|left, right| left.url.as_str().cmp(right.url.as_str()));
    let mut index = 1usize;
    while index < media.len() {
        if media[index - 1].url.as_str() == media[index].url.as_str() {
            if media[index - 1] != media[index] {
                return Err(RadrootsPhase1PublicationArtifactError::ConflictingMediaReference);
            }
            media.remove(index);
        } else {
            index += 1;
        }
    }
    Ok(())
}

fn validate_phase1_publication_profile(
    variant: RadrootsPhase1PublicationSemanticVariant,
    draft: &RadrootsPhase1PublicationDraft,
    media: &[RadrootsPhase1PublicationMediaReference],
) -> Result<(), RadrootsPhase1PublicationArtifactError> {
    if draft.content.len() > DEFAULT_CONTENT_MAX_BYTES
        || RadrootsEventTags::new(draft.tags.clone()).is_err()
    {
        return Err(RadrootsPhase1PublicationArtifactError::InvalidDraft);
    }
    match variant {
        RadrootsPhase1PublicationSemanticVariant::Profile => validate_profile(draft, media),
        RadrootsPhase1PublicationSemanticVariant::Update => validate_update(draft, media),
        RadrootsPhase1PublicationSemanticVariant::PhotoUpdate => validate_post(draft, media, false),
        RadrootsPhase1PublicationSemanticVariant::Ask => validate_post(draft, media, true),
        RadrootsPhase1PublicationSemanticVariant::Event(
            RadrootsPhase1PublicationEventVariant::Date,
        ) => validate_calendar_date(draft, media),
        RadrootsPhase1PublicationSemanticVariant::Event(
            RadrootsPhase1PublicationEventVariant::Time,
        ) => validate_calendar_time(draft, media),
        RadrootsPhase1PublicationSemanticVariant::FoodAvailability => {
            validate_food_availability(draft, media)
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StrictProfileContent {
    name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    display_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    about: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    picture: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    banner: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    nip05: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    bot: Option<bool>,
}

fn validate_profile(
    draft: &RadrootsPhase1PublicationDraft,
    media: &[RadrootsPhase1PublicationMediaReference],
) -> Result<(), RadrootsPhase1PublicationArtifactError> {
    if draft.kind != KIND_PROFILE
        || !draft.tags.is_empty()
        || draft.content.len() > RADROOTS_PROFILE_METADATA_MAX_CONTENT_BYTES
    {
        return Err(RadrootsPhase1PublicationArtifactError::InvalidProfile);
    }
    let content: StrictProfileContent = serde_json::from_str(&draft.content)
        .map_err(|_| RadrootsPhase1PublicationArtifactError::InvalidProfile)?;
    RadrootsAuthoredProfile::new(&content.name)
        .map_err(|_| RadrootsPhase1PublicationArtifactError::InvalidProfile)?;
    if let Some(nip05) = content.nip05.as_deref() {
        let parsed = RadrootsNip05Identifier::parse(nip05)
            .map_err(|_| RadrootsPhase1PublicationArtifactError::InvalidProfile)?;
        if parsed.as_str() != nip05 {
            return Err(RadrootsPhase1PublicationArtifactError::InvalidProfile);
        }
    }
    let canonical = serde_json::to_string(&content)
        .map_err(|_| RadrootsPhase1PublicationArtifactError::Serialization)?;
    if canonical != draft.content {
        return Err(RadrootsPhase1PublicationArtifactError::InvalidProfile);
    }
    let urls = content
        .picture
        .iter()
        .chain(content.banner.iter())
        .map(String::as_str)
        .collect::<Vec<_>>();
    for url in &urls {
        validate_primary_media_url(url, media)?;
    }
    validate_media_urls(urls.into_iter(), media)
}

fn validate_update(
    draft: &RadrootsPhase1PublicationDraft,
    media: &[RadrootsPhase1PublicationMediaReference],
) -> Result<(), RadrootsPhase1PublicationArtifactError> {
    let update = RadrootsAuthoredUpdate::new(&draft.content)
        .map_err(|_| RadrootsPhase1PublicationArtifactError::InvalidPostProfile)?;
    if post_authored::authored_update_to_wire_parts(&update) != draft_wire_parts(draft)
        || !media.is_empty()
    {
        return Err(RadrootsPhase1PublicationArtifactError::InvalidPostProfile);
    }
    Ok(())
}

fn validate_post(
    draft: &RadrootsPhase1PublicationDraft,
    media: &[RadrootsPhase1PublicationMediaReference],
    ask: bool,
) -> Result<(), RadrootsPhase1PublicationArtifactError> {
    if ask && RadrootsAuthoredAsk::new(draft.content.as_str(), Vec::new()).is_err() {
        return Err(RadrootsPhase1PublicationArtifactError::InvalidPostProfile);
    }
    let projection = post_inbound::registry_v7::project_inbound_post_parts(
        draft.kind,
        &draft.tags,
        &draft.content,
    )
    .map_err(|_| RadrootsPhase1PublicationArtifactError::InvalidPostProfile)?;
    let expected_classification = if ask {
        post_inbound::RadrootsPostClassification::Ask
    } else {
        post_inbound::RadrootsPostClassification::PhotoUpdate
    };
    if projection.classification() != expected_classification
        || !projection.diagnostics().is_empty()
        || (!ask && projection.imeta().is_empty())
    {
        return Err(RadrootsPhase1PublicationArtifactError::InvalidPostProfile);
    }
    let mut canonical_tags = Vec::with_capacity(projection.imeta().len() + usize::from(ask));
    if ask {
        canonical_tags.push(vec!["t".to_string(), "radroots-ask".to_string()]);
    }
    let mut urls = Vec::new();
    for imeta in projection.imeta() {
        let (Some(url), Some(sha256), Some(media_type), Some(dimensions), Some(size), Some(alt)) = (
            imeta.url(),
            imeta.sha256(),
            imeta.media_type(),
            imeta.dimensions(),
            imeta.size(),
            imeta.alt(),
        ) else {
            return Err(RadrootsPhase1PublicationArtifactError::InvalidPostProfile);
        };
        let mut tag = vec![
            "imeta".to_string(),
            format!("url {url}"),
            format!("x {sha256}"),
            format!("m {media_type}"),
            format!("dim {}x{}", dimensions.width(), dimensions.height()),
            format!("size {size}"),
            format!("alt {alt}"),
        ];
        tag.extend(
            imeta
                .fallbacks()
                .iter()
                .map(|fallback| format!("fallback {fallback}")),
        );
        canonical_tags.push(tag);
        urls.push(url);
        urls.extend(imeta.fallbacks().iter().map(String::as_str));
        let reference = media_reference_for_url(media, url)?;
        validate_primary_media_reference(reference)?;
        if reference.sha256.to_hex() != sha256
            || reference.media_type.as_str() != media_type
            || reference.size != size
        {
            return Err(RadrootsPhase1PublicationArtifactError::MediaInventoryMismatch);
        }
        for fallback in imeta.fallbacks() {
            let fallback_reference = media_reference_for_url(media, fallback)?;
            if fallback_reference.sha256 != reference.sha256
                || fallback_reference.media_type != reference.media_type
                || fallback_reference.size != reference.size
            {
                return Err(RadrootsPhase1PublicationArtifactError::MediaInventoryMismatch);
            }
        }
    }
    if canonical_tags != draft.tags {
        return Err(RadrootsPhase1PublicationArtifactError::InvalidPostProfile);
    }
    validate_media_urls(urls.into_iter(), media)
}

fn validate_calendar_date(
    draft: &RadrootsPhase1PublicationDraft,
    media: &[RadrootsPhase1PublicationMediaReference],
) -> Result<(), RadrootsPhase1PublicationArtifactError> {
    let parsed = decode::parse_nip52_calendar_date_event(draft.kind, &draft.tags, &draft.content)
        .map_err(|_| RadrootsPhase1PublicationArtifactError::InvalidCalendarProfile)?;
    let admitted = decode::admit_radroots_calendar_date_event(parsed)
        .map_err(|_| RadrootsPhase1PublicationArtifactError::InvalidCalendarProfile)?;
    let canonical = canonical_date_tags(&admitted);
    if canonical != draft.tags {
        return Err(RadrootsPhase1PublicationArtifactError::InvalidCalendarProfile);
    }
    validate_calendar_media(admitted.parsed().common(), media)
}

fn validate_calendar_time(
    draft: &RadrootsPhase1PublicationDraft,
    media: &[RadrootsPhase1PublicationMediaReference],
) -> Result<(), RadrootsPhase1PublicationArtifactError> {
    let parsed = decode::parse_nip52_calendar_time_event(draft.kind, &draft.tags, &draft.content)
        .map_err(|_| RadrootsPhase1PublicationArtifactError::InvalidCalendarProfile)?;
    let admitted = decode::admit_radroots_calendar_time_event(parsed)
        .map_err(|_| RadrootsPhase1PublicationArtifactError::InvalidCalendarProfile)?;
    let canonical = canonical_time_tags(&admitted);
    if canonical != draft.tags {
        return Err(RadrootsPhase1PublicationArtifactError::InvalidCalendarProfile);
    }
    validate_calendar_media(admitted.parsed().common(), media)
}

fn canonical_date_tags(event: &RadrootsAdmittedCalendarDateEvent) -> Vec<Vec<String>> {
    let parsed = event.parsed();
    let mut tags = vec![
        pair("d", parsed.common().d_tag()),
        pair("title", parsed.common().title()),
        pair("start", parsed.start().as_str()),
    ];
    if let Some(end) = parsed.end() {
        tags.push(pair("end", end.as_str()));
    }
    push_canonical_calendar_common_tags(&mut tags, parsed.common());
    tags
}

fn canonical_time_tags(event: &RadrootsAdmittedCalendarTimeEvent) -> Vec<Vec<String>> {
    let parsed = event.parsed();
    let mut tags = vec![
        pair("d", parsed.common().d_tag()),
        pair("title", parsed.common().title()),
        pair("start", parsed.start_wire()),
    ];
    if let Some(end) = parsed.end_wire() {
        tags.push(pair("end", end));
    }
    tags.extend(
        parsed
            .observed_day_indices()
            .iter()
            .map(|day| pair("D", day.wire_value())),
    );
    if let Some(tzid) = parsed.start_tzid() {
        tags.push(pair("start_tzid", tzid.as_str()));
    }
    if let Some(tzid) = parsed.end_tzid() {
        tags.push(pair("end_tzid", tzid.as_str()));
    }
    push_canonical_calendar_common_tags(&mut tags, parsed.common());
    tags
}

fn push_canonical_calendar_common_tags(
    tags: &mut Vec<Vec<String>>,
    common: &RadrootsParsedNip52CalendarCommon,
) {
    tags.extend(
        common
            .locations()
            .iter()
            .map(|value| pair("location", value)),
    );
    if let Some(value) = common.geohash() {
        tags.push(pair("g", value));
    }
    if let Some(value) = common.summary() {
        tags.push(pair("summary", value));
    }
    if let Some(value) = common.image() {
        tags.push(pair("image", value.as_str()));
    }
    for participant in common.participants() {
        let mut tag = vec!["p".to_string(), participant.pubkey.clone()];
        if let Some(relay) = &participant.relay {
            tag.push(relay.clone());
        }
        if let Some(role) = &participant.role {
            if participant.relay.is_none() {
                tag.push(String::new());
            }
            tag.push(role.clone());
        }
        tags.push(tag);
    }
    tags.extend(common.categories().iter().map(|value| pair("t", value)));
    tags.extend(
        common
            .references()
            .iter()
            .map(|value| pair("r", value.as_str())),
    );
    for request in common.calendar_requests() {
        let mut tag = vec!["a".to_string(), request.calendar().as_str().to_string()];
        if let Some(relay) = request.relay() {
            tag.push(relay.to_string());
        }
        tags.push(tag);
    }
}

fn validate_calendar_media(
    common: &RadrootsParsedNip52CalendarCommon,
    media: &[RadrootsPhase1PublicationMediaReference],
) -> Result<(), RadrootsPhase1PublicationArtifactError> {
    if common.legacy_name().is_some() {
        return Err(RadrootsPhase1PublicationArtifactError::InvalidCalendarProfile);
    }
    if let Some(image) = common.image() {
        validate_primary_media_url(image.as_str(), media)?;
    }
    validate_media_urls(common.image().into_iter().map(|url| url.as_str()), media)
}

fn validate_food_availability(
    draft: &RadrootsPhase1PublicationDraft,
    media: &[RadrootsPhase1PublicationMediaReference],
) -> Result<(), RadrootsPhase1PublicationArtifactError> {
    let tags = RadrootsEventTags::new(draft.tags.clone())
        .map_err(|_| RadrootsPhase1PublicationArtifactError::InvalidFoodAvailabilityProfile)?;
    let outcome = food_inbound::registry_v7::project_inbound_food_availability_parts(
        draft.kind,
        draft.created_at,
        &tags,
        &draft.content,
    )
    .map_err(|_| RadrootsPhase1PublicationArtifactError::InvalidFoodAvailabilityProfile)?;
    let projection = match outcome {
        food_inbound::RadrootsFoodAvailabilityProjectionOutcome::Focused(projection) => projection,
        _ => return Err(RadrootsPhase1PublicationArtifactError::InvalidFoodAvailabilityProfile),
    };
    if !projection.diagnostics().is_empty() {
        return Err(RadrootsPhase1PublicationArtifactError::InvalidFoodAvailabilityProfile);
    }
    let mut canonical = vec![
        pair("d", projection.identifier().as_str()),
        pair("title", projection.title().as_str()),
        pair("summary", projection.summary().as_str()),
        pair("published_at", &projection.published_at().to_string()),
        pair("location", projection.location().as_str()),
        vec![
            "price".to_string(),
            projection.price().amount().to_string(),
            projection.price().currency().as_str().to_string(),
        ],
        pair("radroots:price_unit", projection.price().unit().as_str()),
    ];
    if let Some(quantity) = projection.quantity() {
        canonical.push(vec![
            "radroots:quantity".to_string(),
            quantity.amount().to_string(),
            quantity.unit().as_str().to_string(),
        ]);
    }
    canonical.push(pair("status", projection.status().as_str()));
    let mut urls = Vec::new();
    for image in projection.images() {
        let (Some(url), Some(dimensions)) = (image.url(), image.dimensions()) else {
            return Err(RadrootsPhase1PublicationArtifactError::InvalidFoodAvailabilityProfile);
        };
        canonical.push(vec![
            "image".to_string(),
            url.to_string(),
            dimensions.to_string(),
        ]);
        validate_primary_media_url(url, media)?;
        urls.push(url);
    }
    if canonical != draft.tags || projection.content().as_str() != draft.content {
        return Err(RadrootsPhase1PublicationArtifactError::InvalidFoodAvailabilityProfile);
    }
    validate_media_urls(urls.into_iter(), media)
}

fn pair(key: &str, value: &str) -> Vec<String> {
    vec![key.to_string(), value.to_string()]
}

fn draft_wire_parts(draft: &RadrootsPhase1PublicationDraft) -> RadrootsNip01EventWireParts {
    RadrootsNip01EventWireParts {
        kind: draft.kind,
        content: draft.content.clone(),
        tags: draft.tags.clone(),
    }
}

fn media_reference_for_url<'a>(
    media: &'a [RadrootsPhase1PublicationMediaReference],
    url: &str,
) -> Result<&'a RadrootsPhase1PublicationMediaReference, RadrootsPhase1PublicationArtifactError> {
    media
        .binary_search_by(|candidate| candidate.url.as_str().cmp(url))
        .ok()
        .map(|index| &media[index])
        .ok_or(RadrootsPhase1PublicationArtifactError::MediaInventoryMismatch)
}

fn validate_primary_media_url(
    url: &str,
    media: &[RadrootsPhase1PublicationMediaReference],
) -> Result<(), RadrootsPhase1PublicationArtifactError> {
    validate_primary_media_reference(media_reference_for_url(media, url)?)
}

fn validate_primary_media_reference(
    reference: &RadrootsPhase1PublicationMediaReference,
) -> Result<(), RadrootsPhase1PublicationArtifactError> {
    if reference
        .url
        .as_blob_url()
        .hash_path()
        .extension()
        .is_none()
    {
        return Err(RadrootsPhase1PublicationArtifactError::InvalidMediaReference);
    }
    Ok(())
}

fn validate_media_urls<'a>(
    urls: impl Iterator<Item = &'a str>,
    media: &[RadrootsPhase1PublicationMediaReference],
) -> Result<(), RadrootsPhase1PublicationArtifactError> {
    let mut expected = urls.map(str::to_string).collect::<Vec<_>>();
    expected.sort();
    expected.dedup();
    if expected.len() != media.len()
        || expected
            .iter()
            .zip(media)
            .any(|(url, reference)| url != reference.url.as_str())
    {
        return Err(RadrootsPhase1PublicationArtifactError::MediaInventoryMismatch);
    }
    Ok(())
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct DraftWire {
    created_at: u64,
    kind: u32,
    tags: Vec<Vec<String>>,
    content: String,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct MediaReferenceWire {
    url: String,
    sha256: String,
    size: u64,
    media_type: String,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ArtifactWire {
    schema_version: u32,
    semantic_variant: String,
    authored_operation_id: String,
    event_contract_id: String,
    expected_author: String,
    draft: DraftWire,
    expected_event_id: String,
    media_references: Vec<MediaReferenceWire>,
    artifact_digest: String,
}

#[derive(Serialize)]
struct ArtifactPayloadWire<'a> {
    schema_version: u32,
    semantic_variant: &'a str,
    authored_operation_id: &'a str,
    event_contract_id: &'a str,
    expected_author: &'a str,
    draft: DraftWire,
    expected_event_id: &'a str,
    media_references: Vec<MediaReferenceWire>,
}

fn artifact_payload_wire<'a>(
    variant: RadrootsPhase1PublicationSemanticVariant,
    author: &'a RadrootsPublicKey,
    draft: &RadrootsPhase1PublicationDraft,
    expected_event_id: &'a RadrootsEventId,
    media: &[RadrootsPhase1PublicationMediaReference],
) -> ArtifactPayloadWire<'a> {
    ArtifactPayloadWire {
        schema_version: RADROOTS_PHASE1_PUBLICATION_ARTIFACT_SCHEMA_VERSION,
        semantic_variant: variant.as_str(),
        authored_operation_id: variant.authored_operation_id(),
        event_contract_id: variant.event_contract_id(),
        expected_author: author.as_str(),
        draft: DraftWire {
            created_at: draft.created_at,
            kind: draft.kind,
            tags: draft.tags.clone(),
            content: draft.content.clone(),
        },
        expected_event_id: expected_event_id.as_str(),
        media_references: media
            .iter()
            .map(RadrootsPhase1PublicationMediaReference::to_wire)
            .collect(),
    }
}

fn compute_artifact_digest(
    variant: RadrootsPhase1PublicationSemanticVariant,
    author: &RadrootsPublicKey,
    draft: &RadrootsPhase1PublicationDraft,
    expected_event_id: &RadrootsEventId,
    media: &[RadrootsPhase1PublicationMediaReference],
) -> Result<RadrootsPhase1PublicationArtifactDigest, RadrootsPhase1PublicationArtifactError> {
    let payload = serde_json::to_vec(&artifact_payload_wire(
        variant,
        author,
        draft,
        expected_event_id,
        media,
    ))
    .map_err(|_| RadrootsPhase1PublicationArtifactError::Serialization)?;
    let mut hasher = Sha256::new();
    hasher.update(ARTIFACT_DIGEST_DOMAIN);
    hasher.update(ARTIFACT_DIGEST_DOMAIN_TERMINATOR);
    hasher.update(payload);
    Ok(RadrootsPhase1PublicationArtifactDigest(
        hasher.finalize().into(),
    ))
}

fn serialize_artifact(
    variant: RadrootsPhase1PublicationSemanticVariant,
    author: &RadrootsPublicKey,
    draft: &RadrootsPhase1PublicationDraft,
    expected_event_id: &RadrootsEventId,
    media: &[RadrootsPhase1PublicationMediaReference],
    digest: RadrootsPhase1PublicationArtifactDigest,
) -> Result<Vec<u8>, RadrootsPhase1PublicationArtifactError> {
    let payload = artifact_payload_wire(variant, author, draft, expected_event_id, media);
    serde_json::to_vec(&ArtifactWire {
        schema_version: payload.schema_version,
        semantic_variant: payload.semantic_variant.to_string(),
        authored_operation_id: payload.authored_operation_id.to_string(),
        event_contract_id: payload.event_contract_id.to_string(),
        expected_author: payload.expected_author.to_string(),
        draft: payload.draft,
        expected_event_id: payload.expected_event_id.to_string(),
        media_references: payload.media_references,
        artifact_digest: digest.to_hex(),
    })
    .map_err(|_| RadrootsPhase1PublicationArtifactError::Serialization)
}

#[cfg(test)]
mod tests {
    use super::*;

    const AUTHOR: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    #[test]
    fn signed_event_wire_size_accepts_exact_limit_and_rejects_one_over() {
        let author = RadrootsPublicKey::parse(AUTHOR).unwrap();
        let mut draft = RadrootsPhase1PublicationDraft {
            created_at: 1_784_347_200,
            kind: KIND_POST,
            tags: Vec::new(),
            content: String::new(),
        };
        let empty_id = event_id(&author, &draft);
        let base = signed_event_wire_size(&author, &draft, &empty_id).unwrap();
        let available = RADROOTS_PHASE1_PUBLICATION_SIGNED_EVENT_MAX_BYTES - base;
        draft.content = "\0".repeat(available / 6) + &"x".repeat(available % 6);
        assert!(draft.content.len() <= DEFAULT_CONTENT_MAX_BYTES);

        let exact_id = event_id(&author, &draft);
        assert_eq!(
            signed_event_wire_size(&author, &draft, &exact_id).unwrap(),
            RADROOTS_PHASE1_PUBLICATION_SIGNED_EVENT_MAX_BYTES
        );
        validate_signed_event_wire_size(&author, &draft, &exact_id).unwrap();

        draft.content.push('x');
        let oversized_id = event_id(&author, &draft);
        assert_eq!(
            validate_signed_event_wire_size(&author, &draft, &oversized_id),
            Err(RadrootsPhase1PublicationArtifactError::EventWireTooLarge {
                max: RADROOTS_PHASE1_PUBLICATION_SIGNED_EVENT_MAX_BYTES,
                actual: RADROOTS_PHASE1_PUBLICATION_SIGNED_EVENT_MAX_BYTES + 1,
            })
        );
    }

    #[test]
    fn publication_artifact_digest_has_exactly_one_nul_domain_terminator() {
        let author = RadrootsPublicKey::parse(AUTHOR).unwrap();
        let draft = RadrootsPhase1PublicationDraft {
            created_at: 1_784_347_200,
            kind: KIND_POST,
            tags: Vec::new(),
            content: "Carrots harvested today".to_string(),
        };
        let expected_event_id = event_id(&author, &draft);
        let payload = serde_json::to_vec(&artifact_payload_wire(
            RadrootsPhase1PublicationSemanticVariant::Update,
            &author,
            &draft,
            &expected_event_id,
            &[],
        ))
        .unwrap();
        let actual = compute_artifact_digest(
            RadrootsPhase1PublicationSemanticVariant::Update,
            &author,
            &draft,
            &expected_event_id,
            &[],
        )
        .unwrap();

        let mut exact = Sha256::new();
        exact.update(b"radroots.phase1.publication-artifact.v1");
        exact.update([0]);
        exact.update(&payload);
        let exact: [u8; 32] = exact.finalize().into();
        assert_eq!(actual.as_bytes(), &exact);

        for prefix in [
            b"radroots.phase1.publication-artifact.v1".as_slice(),
            b"radroots.phase1.publication-artifact.v1\0\0".as_slice(),
        ] {
            let mut alternate = Sha256::new();
            alternate.update(prefix);
            alternate.update(&payload);
            let alternate: [u8; 32] = alternate.finalize().into();
            assert_ne!(actual.as_bytes(), &alternate);
        }
    }

    fn event_id(
        author: &RadrootsPublicKey,
        draft: &RadrootsPhase1PublicationDraft,
    ) -> RadrootsEventId {
        compute_canonical_nip01_event_id(
            author.as_str(),
            draft.created_at,
            draft.kind,
            &draft.tags,
            &draft.content,
        )
        .unwrap()
    }
}
