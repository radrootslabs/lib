//! Closed registry-v7 typed authoring conversions and historical validators.

#[cfg(not(feature = "std"))]
use alloc::string::{String, ToString};
#[cfg(all(not(feature = "std"), feature = "json"))]
use alloc::{vec, vec::Vec};
#[cfg(feature = "std")]
use std::string::String;
#[cfg(all(feature = "std", feature = "json"))]
use std::vec::Vec;

use core::fmt;
#[cfg(feature = "json")]
use radroots_event::profile::AuthoredProfile;
use radroots_event::{
    contract::{ContractIdentityError, ContractKey, EventAuthoringPolicy},
    envelope::{EventEnvelopeError, EventTags},
    food::availability::FoodAvailabilityDetails,
    post::{
        AuthoredAsk, AuthoredPhotoUpdate, AuthoredUpdate, comment::AuthoredNip22Comment,
        deletion::AuthoredNip09DeletionRequest, reply::AuthoredNip10Reply,
    },
    wire::{
        CanonicalEventIdError, Nip01EventWireParts, compute_canonical_nip01_event_id,
        v1::DEFAULT_CONTENT_MAX_BYTES,
    },
};
use radroots_identity::{Error as PublicKeyError, PublicKey};

#[cfg(feature = "json")]
use crate::profile::authored::{
    RadrootsAuthoredProfileEncodeError, authored_profile_to_wire_parts,
};
use crate::{
    comment::authored::authored_nip22_comment_to_wire_parts,
    deletion::authored::authored_nip09_deletion_request_to_wire_parts,
    food_availability::authored::{
        RadrootsFoodAvailabilityEncodeError, authored_food_availability_to_wire_parts,
    },
    post::authored::{
        authored_ask_to_wire_parts, authored_photo_update_to_wire_parts,
        authored_update_to_wire_parts,
    },
    reply::authored::authored_nip10_reply_to_wire_parts,
};
#[cfg(feature = "json")]
use crate::{
    comment::inbound::registry_v7::project_nip22_comment_parts,
    deletion::reconciliation_v1::inbound::project_nip09_deletion_request_parts,
    food_availability::inbound::registry_v7::{
        RadrootsFoodAvailabilityProjectionOutcome, project_inbound_food_availability_parts,
    },
    post::inbound::registry_v7::{RadrootsPostClassification, project_inbound_post_parts},
    reply::inbound::registry_v7::{RadrootsNip10ReplyStyle, project_nip10_reply_parts},
};

use super::{AuthoredEventBody, AuthoredEventPlan};

pub const REGISTRY_V7_TYPED_AUTHORING_CONTRACT_IDS: [&str; 8] = [
    "radroots.profile.metadata.v1",
    "radroots.social.update.v1",
    "radroots.social.photo_update.v1",
    "radroots.social.ask.v1",
    "radroots.social.reply.v1",
    "radroots.social.deletion_request.v1",
    "radroots.social.comment.v1",
    "radroots.food.availability.v1",
];

#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AuthoredPlanError {
    ContractIdentity(ContractIdentityError),
    ContractNotTyped {
        contract_id: String,
    },
    ContractKindMismatch {
        expected: u32,
        actual: u32,
    },
    InvalidAuthor(PublicKeyError),
    Envelope(EventEnvelopeError),
    ContentTooLarge {
        max: usize,
        actual: usize,
    },
    CanonicalEventId(CanonicalEventIdError),
    #[cfg(feature = "json")]
    Profile(RadrootsAuthoredProfileEncodeError),
    FoodAvailability(RadrootsFoodAvailabilityEncodeError),
}

impl AuthoredPlanError {
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::ContractIdentity(error) => error.code(),
            Self::ContractNotTyped { .. } => "contract_not_typed",
            Self::ContractKindMismatch { .. } => "contract_kind_mismatch",
            Self::InvalidAuthor(_) => "invalid_author",
            Self::Envelope(_) => "invalid_envelope_parts",
            Self::ContentTooLarge { .. } => "content_too_large",
            Self::CanonicalEventId(_) => "canonical_event_id",
            #[cfg(feature = "json")]
            Self::Profile(error) => error.code(),
            Self::FoodAvailability(error) => error.code(),
        }
    }
}

impl fmt::Display for AuthoredPlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ContractIdentity(error) => write!(formatter, "invalid contract key: {error}"),
            Self::ContractNotTyped { contract_id } => {
                write!(
                    formatter,
                    "event contract `{contract_id}` is not typed-authorable"
                )
            }
            Self::ContractKindMismatch { expected, actual } => write!(
                formatter,
                "typed authored kind {actual} does not match contract kind {expected}"
            ),
            Self::InvalidAuthor(error) => write!(formatter, "invalid expected author: {error}"),
            Self::Envelope(error) => write!(formatter, "invalid authored event parts: {error}"),
            Self::ContentTooLarge { max, actual } => {
                write!(
                    formatter,
                    "authored content is {actual} bytes; max is {max}"
                )
            }
            Self::CanonicalEventId(error) => write!(formatter, "{error}"),
            #[cfg(feature = "json")]
            Self::Profile(error) => write!(formatter, "{error}"),
            Self::FoodAvailability(error) => write!(formatter, "{error}"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for AuthoredPlanError {}

impl AuthoredEventBody {
    #[cfg(feature = "json")]
    pub fn from_profile(profile: &AuthoredProfile) -> Result<Self, AuthoredPlanError> {
        let wire = authored_profile_to_wire_parts(profile).map_err(AuthoredPlanError::Profile)?;
        build_typed_body("radroots.profile.metadata.v1", wire)
    }

    pub fn from_update(update: &AuthoredUpdate) -> Result<Self, AuthoredPlanError> {
        build_typed_body(
            "radroots.social.update.v1",
            authored_update_to_wire_parts(update),
        )
    }

    pub fn from_photo_update(photo: &AuthoredPhotoUpdate) -> Result<Self, AuthoredPlanError> {
        build_typed_body(
            "radroots.social.photo_update.v1",
            authored_photo_update_to_wire_parts(photo),
        )
    }

    pub fn from_ask(ask: &AuthoredAsk) -> Result<Self, AuthoredPlanError> {
        build_typed_body("radroots.social.ask.v1", authored_ask_to_wire_parts(ask))
    }

    pub fn from_nip10_reply(reply: &AuthoredNip10Reply) -> Result<Self, AuthoredPlanError> {
        build_typed_body(
            "radroots.social.reply.v1",
            authored_nip10_reply_to_wire_parts(reply),
        )
    }

    pub fn from_nip09_deletion_request(
        request: &AuthoredNip09DeletionRequest,
    ) -> Result<Self, AuthoredPlanError> {
        build_typed_body(
            "radroots.social.deletion_request.v1",
            authored_nip09_deletion_request_to_wire_parts(request),
        )
    }

    pub fn from_nip22_comment(comment: &AuthoredNip22Comment) -> Result<Self, AuthoredPlanError> {
        build_typed_body(
            "radroots.social.comment.v1",
            authored_nip22_comment_to_wire_parts(comment),
        )
    }

    pub fn from_food_availability(
        details: &FoodAvailabilityDetails,
        created_at: u64,
    ) -> Result<Self, AuthoredPlanError> {
        let wire = authored_food_availability_to_wire_parts(details, created_at)
            .map_err(AuthoredPlanError::FoodAvailability)?;
        build_typed_body("radroots.food.availability.v1", wire)
    }
}

impl AuthoredEventPlan {
    pub fn bind(
        body: AuthoredEventBody,
        created_at: u64,
        expected_author: impl AsRef<str>,
    ) -> Result<Self, AuthoredPlanError> {
        let author = PublicKey::from_hex(expected_author.as_ref())
            .map_err(AuthoredPlanError::InvalidAuthor)?;
        let expected_event_id = compute_canonical_nip01_event_id(
            &author.to_hex(),
            created_at,
            body.kind,
            &body.tags,
            &body.content,
        )
        .map_err(AuthoredPlanError::CanonicalEventId)?;
        Ok(Self::from_validated_parts(
            body,
            author,
            created_at,
            expected_event_id,
        ))
    }

    #[cfg(feature = "json")]
    pub fn from_profile(
        profile: &AuthoredProfile,
        created_at: u64,
        expected_author: impl AsRef<str>,
    ) -> Result<Self, AuthoredPlanError> {
        Self::bind(
            AuthoredEventBody::from_profile(profile)?,
            created_at,
            expected_author,
        )
    }

    pub fn from_update(
        update: &AuthoredUpdate,
        created_at: u64,
        expected_author: impl AsRef<str>,
    ) -> Result<Self, AuthoredPlanError> {
        Self::bind(
            AuthoredEventBody::from_update(update)?,
            created_at,
            expected_author,
        )
    }

    pub fn from_photo_update(
        photo: &AuthoredPhotoUpdate,
        created_at: u64,
        expected_author: impl AsRef<str>,
    ) -> Result<Self, AuthoredPlanError> {
        Self::bind(
            AuthoredEventBody::from_photo_update(photo)?,
            created_at,
            expected_author,
        )
    }

    pub fn from_ask(
        ask: &AuthoredAsk,
        created_at: u64,
        expected_author: impl AsRef<str>,
    ) -> Result<Self, AuthoredPlanError> {
        Self::bind(
            AuthoredEventBody::from_ask(ask)?,
            created_at,
            expected_author,
        )
    }

    pub fn from_nip10_reply(
        reply: &AuthoredNip10Reply,
        created_at: u64,
        expected_author: impl AsRef<str>,
    ) -> Result<Self, AuthoredPlanError> {
        Self::bind(
            AuthoredEventBody::from_nip10_reply(reply)?,
            created_at,
            expected_author,
        )
    }

    pub fn from_nip09_deletion_request(
        request: &AuthoredNip09DeletionRequest,
        created_at: u64,
        expected_author: impl AsRef<str>,
    ) -> Result<Self, AuthoredPlanError> {
        Self::bind(
            AuthoredEventBody::from_nip09_deletion_request(request)?,
            created_at,
            expected_author,
        )
    }

    pub fn from_nip22_comment(
        comment: &AuthoredNip22Comment,
        created_at: u64,
        expected_author: impl AsRef<str>,
    ) -> Result<Self, AuthoredPlanError> {
        Self::bind(
            AuthoredEventBody::from_nip22_comment(comment)?,
            created_at,
            expected_author,
        )
    }

    pub fn from_food_availability(
        details: &FoodAvailabilityDetails,
        created_at: u64,
        expected_author: impl AsRef<str>,
    ) -> Result<Self, AuthoredPlanError> {
        Self::bind(
            AuthoredEventBody::from_food_availability(details, created_at)?,
            created_at,
            expected_author,
        )
    }
}

fn build_typed_body(
    contract_id: &str,
    wire: Nip01EventWireParts,
) -> Result<AuthoredEventBody, AuthoredPlanError> {
    let contract =
        ContractKey::current(contract_id).map_err(AuthoredPlanError::ContractIdentity)?;
    let definition = contract.contract();
    if definition.authoring_policy() != EventAuthoringPolicy::TypedOnly {
        return Err(AuthoredPlanError::ContractNotTyped {
            contract_id: contract_id.to_string(),
        });
    }
    if definition.kind != wire.kind {
        return Err(AuthoredPlanError::ContractKindMismatch {
            expected: definition.kind,
            actual: wire.kind,
        });
    }
    if wire.content.len() > DEFAULT_CONTENT_MAX_BYTES {
        return Err(AuthoredPlanError::ContentTooLarge {
            max: DEFAULT_CONTENT_MAX_BYTES,
            actual: wire.content.len(),
        });
    }
    EventTags::new(wire.tags.clone()).map_err(AuthoredPlanError::Envelope)?;
    Ok(AuthoredEventBody {
        contract,
        kind: wire.kind,
        tags: wire.tags,
        content: wire.content,
    })
}

#[cfg(feature = "json")]
pub(super) fn validate_historical_typed_profile(
    contract_id: &str,
    created_at: u64,
    kind: u32,
    tags: &[Vec<String>],
    content: &str,
) -> Result<(), String> {
    match contract_id {
        #[cfg(feature = "json")]
        "radroots.profile.metadata.v1" => validate_profile(tags, content),
        "radroots.social.update.v1" => validate_update(tags, content),
        "radroots.social.photo_update.v1" => {
            validate_post_profile(RadrootsPostClassification::PhotoUpdate, tags, content)
        }
        "radroots.social.ask.v1" => {
            validate_post_profile(RadrootsPostClassification::Ask, tags, content)
        }
        "radroots.social.reply.v1" => validate_reply(created_at, kind, tags, content),
        "radroots.social.deletion_request.v1" => validate_deletion(created_at, kind, tags, content),
        "radroots.social.comment.v1" => validate_comment(created_at, kind, tags, content),
        "radroots.food.availability.v1" => validate_food(created_at, kind, tags, content),
        _ => Err("historical_typed_profile_unavailable".to_string()),
    }
}

#[cfg(feature = "json")]
fn validate_profile(tags: &[Vec<String>], content: &str) -> Result<(), String> {
    use serde::Serialize;

    if !tags.is_empty() {
        return Err("profile_tags_forbidden".to_string());
    }
    let profile =
        crate::profile::inbound::registry_v7::parse_inbound_profile_metadata_registry_v7(content)
            .map_err(|error| error.code().to_string())?;
    if !profile.residual_fields().is_empty() {
        return Err("profile_residual_fields".to_string());
    }
    let name = profile
        .name()
        .ok_or_else(|| "profile_name_missing".to_string())?;
    AuthoredProfile::new(name).map_err(|error| error.code().to_string())?;
    if profile
        .picture()
        .into_iter()
        .chain(profile.banner())
        .any(|media| radroots_blossom::BlobUrl::parse(media.as_str()).is_err())
    {
        return Err("profile_media_url_invalid".to_string());
    }

    #[derive(Serialize)]
    struct CanonicalProfile<'a> {
        name: &'a str,
        #[serde(skip_serializing_if = "Option::is_none")]
        display_name: Option<&'a str>,
        #[serde(skip_serializing_if = "Option::is_none")]
        about: Option<&'a str>,
        #[serde(skip_serializing_if = "Option::is_none")]
        picture: Option<&'a str>,
        #[serde(skip_serializing_if = "Option::is_none")]
        banner: Option<&'a str>,
        #[serde(skip_serializing_if = "Option::is_none")]
        nip05: Option<&'a str>,
        #[serde(skip_serializing_if = "Option::is_none")]
        bot: Option<bool>,
    }
    let canonical = serde_json::to_string(&CanonicalProfile {
        name,
        display_name: profile.display_name(),
        about: profile.about(),
        picture: profile.picture().map(|media| media.as_str()),
        banner: profile.banner().map(|media| media.as_str()),
        nip05: profile.nip05().map(|identifier| identifier.as_str()),
        bot: profile.bot(),
    })
    .map_err(|_| "profile_json_encode".to_string())?;
    if canonical != content {
        return Err("profile_noncanonical_json".to_string());
    }
    Ok(())
}

#[cfg(feature = "json")]
fn validate_update(tags: &[Vec<String>], content: &str) -> Result<(), String> {
    if !tags.is_empty() {
        return Err("update_tags_forbidden".to_string());
    }
    AuthoredUpdate::new(content)
        .map(|_| ())
        .map_err(|error| error.code().to_string())
}

#[cfg(feature = "json")]
fn validate_post_profile(
    expected: RadrootsPostClassification,
    tags: &[Vec<String>],
    content: &str,
) -> Result<(), String> {
    let projection =
        project_inbound_post_parts(1, tags, content).map_err(|error| error.code().to_string())?;
    if projection.classification() != expected || !projection.diagnostics().is_empty() {
        return Err("post_profile_mismatch".to_string());
    }
    let mut images = tags;
    if expected == RadrootsPostClassification::Ask {
        let Some((marker, remaining)) = images.split_first() else {
            return Err("ask_marker_missing".to_string());
        };
        if marker.as_slice() != ["t", "radroots-ask"] {
            return Err("ask_marker_noncanonical".to_string());
        }
        images = remaining;
    }
    if expected == RadrootsPostClassification::PhotoUpdate && images.is_empty() {
        return Err("photo_image_missing".to_string());
    }
    if content.trim().is_empty() {
        return Err("post_content_missing".to_string());
    }
    for tag in images {
        if !canonical_imeta_tag(tag) {
            return Err("imeta_noncanonical".to_string());
        }
    }
    validate_exact_url_occurrences(
        content,
        images.iter().map(|tag| {
            tag[1]
                .strip_prefix("url ")
                .expect("canonical imeta establishes a URL field")
        }),
    )?;
    if tags.len() != images.len() + usize::from(expected == RadrootsPostClassification::Ask) {
        return Err("post_extra_tags".to_string());
    }
    Ok(())
}

#[cfg(feature = "json")]
fn canonical_imeta_tag(tag: &[String]) -> bool {
    if tag.len() < 7 || tag.first().map(String::as_str) != Some("imeta") {
        return false;
    }
    let required = ["url ", "x ", "m ", "dim ", "size ", "alt "];
    tag[1..7]
        .iter()
        .zip(required)
        .all(|(field, prefix)| field.starts_with(prefix) && field.len() > prefix.len())
        && tag[7..]
            .iter()
            .all(|field| field.starts_with("fallback ") && field.len() > "fallback ".len())
}

#[cfg(feature = "json")]
fn validate_exact_url_occurrences<'a>(
    content: &str,
    urls: impl IntoIterator<Item = &'a str>,
) -> Result<(), String> {
    let mut occurrences = Vec::new();
    for url in urls {
        let mut matches = content.match_indices(url);
        let first = matches.next();
        let actual = usize::from(first.is_some()).saturating_add(matches.count());
        if actual != 1 {
            return Err("imeta_url_occurrence_count".to_string());
        }
        let (start, matched) = first.expect("exactly one occurrence was established");
        occurrences.push((start, start.saturating_add(matched.len())));
    }
    occurrences.sort_unstable();
    if occurrences.windows(2).any(|pair| pair[0].1 > pair[1].0) {
        return Err("imeta_url_overlap".to_string());
    }
    Ok(())
}

#[cfg(feature = "json")]
fn validate_reply(
    created_at: u64,
    kind: u32,
    tags: &[Vec<String>],
    content: &str,
) -> Result<(), String> {
    let projection = project_nip10_reply_parts(kind, tags, content, decimal_digits(created_at))
        .map_err(|error| error.code().to_string())?;
    if projection.style() != RadrootsNip10ReplyStyle::Marked
        || !projection.citations().is_empty()
        || !projection.diagnostics().is_empty()
    {
        return Err("reply_profile_noncanonical".to_string());
    }
    let mut expected = Vec::new();
    let root = projection.root();
    if root.raw_tag().len() != 4 || root.raw_tag().get(3).map(String::as_str) != Some("root") {
        return Err("reply_root_noncanonical".to_string());
    }
    expected.push(root.raw_tag().to_vec());
    if let Some(parent) = projection.reply_reference() {
        if parent.raw_tag().len() != 4
            || parent.raw_tag().get(3).map(String::as_str) != Some("reply")
        {
            return Err("reply_parent_noncanonical".to_string());
        }
        expected.push(parent.raw_tag().to_vec());
    }
    let participants = projection.participants();
    let Some(root_author) = participants.first() else {
        return Err("reply_author_missing".to_string());
    };
    expected.push(vec!["p".to_string(), root_author.pubkey().to_hex()]);
    if projection.reply_reference().is_some() && participants.len() == 2 {
        expected.push(vec!["p".to_string(), participants[1].pubkey().to_hex()]);
    } else if participants.len() != 1 {
        return Err("reply_participants_noncanonical".to_string());
    }
    if expected != tags {
        return Err("reply_tags_noncanonical".to_string());
    }
    Ok(())
}

#[cfg(feature = "json")]
fn validate_deletion(
    created_at: u64,
    kind: u32,
    tags: &[Vec<String>],
    content: &str,
) -> Result<(), String> {
    project_nip09_deletion_request_parts(kind, tags, content, created_at)
        .map_err(|error| error.code().to_string())?;
    let mut section = 0_u8;
    let mut previous_text: Option<&str> = None;
    let mut previous_kind = None;
    let mut address_kinds = Vec::new();
    let mut kind_hints = Vec::new();
    let mut targets = 0usize;
    for tag in tags {
        if tag.len() != 2 {
            return Err("deletion_tag_shape".to_string());
        }
        match tag[0].as_str() {
            "e" if section == 0 => {
                radroots_event::id::EventId::parse(&tag[1])
                    .map_err(|_| "deletion_event_id".to_string())?;
                if previous_text.is_some_and(|previous| previous >= tag[1].as_str()) {
                    return Err("deletion_event_order".to_string());
                }
                previous_text = Some(&tag[1]);
                targets += 1;
            }
            "a" if section <= 1 => {
                if section == 0 {
                    previous_text = None;
                }
                section = 1;
                let coordinate = radroots_event::id::Nip01Coordinate::parse(&tag[1])
                    .map_err(|_| "deletion_address".to_string())?;
                if previous_text.is_some_and(|previous| previous >= tag[1].as_str()) {
                    return Err("deletion_address_order".to_string());
                }
                previous_text = Some(&tag[1]);
                address_kinds.push(coordinate.kind());
                targets += 1;
            }
            "k" => {
                section = 2;
                let value = tag[1]
                    .parse::<u32>()
                    .map_err(|_| "deletion_kind_hint".to_string())?;
                if value.to_string() != tag[1]
                    || previous_kind.is_some_and(|previous| previous >= value)
                {
                    return Err("deletion_kind_order".to_string());
                }
                previous_kind = Some(value);
                kind_hints.push(value);
            }
            _ => return Err("deletion_tag_order".to_string()),
        }
    }
    if targets == 0
        || kind_hints.is_empty()
        || address_kinds.iter().any(|kind| !kind_hints.contains(kind))
    {
        return Err("deletion_target_kind_set".to_string());
    }
    Ok(())
}

#[cfg(feature = "json")]
fn validate_comment(
    created_at: u64,
    kind: u32,
    tags: &[Vec<String>],
    content: &str,
) -> Result<(), String> {
    use crate::comment::inbound::registry_v7::RadrootsInboundNip22CommentPosition;

    let projection = project_nip22_comment_parts(kind, tags, content, decimal_digits(created_at))
        .map_err(|error| error.code().to_string())?;
    if !projection.diagnostics().is_empty() || !projection.mentions().is_empty() {
        return Err("comment_profile_noncanonical".to_string());
    }
    let expected_keys: &[&str] = match projection.position() {
        RadrootsInboundNip22CommentPosition::TopLevelEvent { .. } => {
            &["E", "K", "P", "e", "k", "p"]
        }
        RadrootsInboundNip22CommentPosition::TopLevelAddress { .. } => {
            &["A", "K", "P", "a", "e", "k", "p"]
        }
        RadrootsInboundNip22CommentPosition::Nested { .. } => match projection.root() {
            crate::comment::inbound::registry_v7::RadrootsInboundNip22CommentRoot::Event(_) => {
                &["E", "K", "P", "e", "k", "p"]
            }
            crate::comment::inbound::registry_v7::RadrootsInboundNip22CommentRoot::Address(_) => {
                &["A", "K", "P", "e", "k", "p"]
            }
        },
    };
    if tags.len() != expected_keys.len()
        || tags
            .iter()
            .zip(expected_keys)
            .any(|(tag, key)| tag.first().map(String::as_str) != Some(*key))
        || tags.iter().any(|tag| tag.len() < 2 || tag.len() > 4)
    {
        return Err("comment_tags_noncanonical".to_string());
    }
    Ok(())
}

#[cfg(feature = "json")]
fn validate_food(
    created_at: u64,
    kind: u32,
    tags: &[Vec<String>],
    content: &str,
) -> Result<(), String> {
    let event_tags = EventTags::new(tags.to_vec()).map_err(|error| error.to_string())?;
    let outcome = project_inbound_food_availability_parts(kind, created_at, &event_tags, content)
        .map_err(|error| error.code().to_string())?;
    let RadrootsFoodAvailabilityProjectionOutcome::Focused(projection) = outcome else {
        return Err("food_profile_excluded".to_string());
    };
    if !projection.diagnostics().is_empty()
        || projection.images().iter().any(|image| !image.qualifies())
    {
        return Err("food_image_profile".to_string());
    }
    let mut expected = vec![
        vec![
            "d".to_string(),
            projection.identifier().as_str().to_string(),
        ],
        vec!["title".to_string(), projection.title().as_str().to_string()],
        vec![
            "summary".to_string(),
            projection.summary().as_str().to_string(),
        ],
        vec![
            "published_at".to_string(),
            projection.published_at().to_string(),
        ],
        vec![
            "location".to_string(),
            projection.location().as_str().to_string(),
        ],
        vec![
            "price".to_string(),
            projection.price().amount().to_string(),
            projection.price().currency().as_str().to_string(),
        ],
        vec![
            "radroots:price_unit".to_string(),
            projection.price().unit().as_str().to_string(),
        ],
    ];
    if let Some(quantity) = projection.quantity() {
        expected.push(vec![
            "radroots:quantity".to_string(),
            quantity.amount().to_string(),
            quantity.unit().as_str().to_string(),
        ]);
    }
    expected.push(vec![
        "status".to_string(),
        projection.status().as_str().to_string(),
    ]);
    expected.extend(
        projection
            .images()
            .iter()
            .map(|image| image.raw_tag().to_vec()),
    );
    if expected != tags {
        return Err("food_tags_noncanonical".to_string());
    }
    Ok(())
}

#[cfg(feature = "json")]
fn decimal_digits(mut value: u64) -> usize {
    let mut digits = 1usize;
    while value >= 10 {
        value /= 10;
        digits += 1;
    }
    digits
}

#[cfg(all(test, feature = "json"))]
mod tests {
    use super::validate_exact_url_occurrences;

    #[test]
    fn historical_typed_photo_urls_are_exact_disjoint_and_utf8_safe() {
        let short = "https://media.example/abc";
        let long = "https://media.example/abc.webp";
        assert_eq!(
            validate_exact_url_occurrences("missing", [short]),
            Err("imeta_url_occurrence_count".to_owned())
        );
        assert_eq!(
            validate_exact_url_occurrences(&format!("{short} then {short}"), [short]),
            Err("imeta_url_occurrence_count".to_owned())
        );
        assert_eq!(
            validate_exact_url_occurrences(long, [short, long]),
            Err("imeta_url_overlap".to_owned())
        );
        assert!(validate_exact_url_occurrences(&format!("苗 {long} 🍓"), [long]).is_ok());
    }
}
