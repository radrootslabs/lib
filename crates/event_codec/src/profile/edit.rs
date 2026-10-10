//! Complete-profile replacement with exact verified-baseline and media fences.

#[cfg(not(feature = "std"))]
use alloc::{
    boxed::Box,
    collections::BTreeMap,
    string::{String, ToString},
    vec::Vec,
};
#[cfg(feature = "std")]
use std::{boxed::Box, collections::BTreeMap, string::String, vec::Vec};

use core::fmt;
use radroots_event::{
    contract::{
        ContractId, ContractKey, RegistryVersion, registry_v8::PROFILE_METADATA_EDIT_CONTRACT_ID,
    },
    profile::{
        AuthoredProfile, Nip05Identifier, RADROOTS_PROFILE_METADATA_MAX_CONTENT_BYTES,
        edit::{ProfileEdit, ProfileFieldChange, ProfileMediaChange},
    },
    wire::Nip01EventWireParts,
};
use radroots_identity::PublicKey;
use serde::Serialize;
use serde_json::value::RawValue;

use super::{admission::RadrootsAdmittedProfileEvent, inbound::parse_inbound_profile_metadata};
use crate::authoring::{
    AuthoredEventBody, AuthoredEventPlan, AuthoredPlanError, PLAN_WIRE_MAX_BYTES, PlanWireV1,
};

#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProfileEditError {
    StaleBaseline,
    AuthorMismatch,
    BaselineTagsUnsupported,
    TimestampNotAdvancing,
    InvalidName,
    ContentTooLarge { max: usize, actual: usize },
    DurableWireTooLarge { max: usize, actual: usize },
    MediaVerificationRequired { field: &'static str },
    MediaVerificationMismatch { field: &'static str },
    InvalidMetadata,
    Plan(AuthoredPlanError),
}

impl ProfileEditError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::StaleBaseline => "stale_profile_baseline",
            Self::AuthorMismatch => "profile_author_mismatch",
            Self::BaselineTagsUnsupported => "profile_baseline_tags_unsupported",
            Self::TimestampNotAdvancing => "profile_timestamp_not_advancing",
            Self::InvalidName => "invalid_name",
            Self::ContentTooLarge { .. } => "content_too_large",
            Self::DurableWireTooLarge { .. } => "profile_durable_wire_too_large",
            Self::MediaVerificationRequired { .. } => "profile_media_verification_required",
            Self::MediaVerificationMismatch { .. } => "profile_media_verification_mismatch",
            Self::InvalidMetadata => "profile_edit_invalid_metadata",
            Self::Plan(error) => error.code(),
        }
    }
}

impl fmt::Display for ProfileEditError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ContentTooLarge { max, actual } | Self::DurableWireTooLarge { max, actual } => {
                write!(formatter, "{}: {actual} bytes exceeds {max}", self.code())
            }
            Self::MediaVerificationRequired { field }
            | Self::MediaVerificationMismatch { field } => {
                write!(formatter, "{}: {field}", self.code())
            }
            Self::Plan(error) => write!(formatter, "{error}"),
            _ => formatter.write_str(self.code()),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for ProfileEditError {}

type RawFields = BTreeMap<String, Box<RawValue>>;

impl AuthoredEventPlan {
    /// Builds a complete replacement from the exact verified canonical head.
    ///
    /// The runtime must read the real canonical head, persist its baseline id,
    /// and recheck it before signing/side effects. This pure comparison is not
    /// a storage CAS. Kept media proof is not upload-completion evidence.
    pub fn from_profile_edit(
        baseline: &RadrootsAdmittedProfileEvent,
        current_head: &RadrootsAdmittedProfileEvent,
        edit: &ProfileEdit,
        created_at: u64,
        expected_author: impl AsRef<str>,
    ) -> Result<Self, ProfileEditError> {
        let author = PublicKey::from_hex(expected_author.as_ref())
            .map_err(|_| ProfileEditError::AuthorMismatch)?;
        if baseline.event().author() != &author || current_head.event().author() != &author {
            return Err(ProfileEditError::AuthorMismatch);
        }
        if baseline.event().id() != current_head.event().id() {
            return Err(ProfileEditError::StaleBaseline);
        }
        if !baseline.event().tags_as_vec().is_empty() {
            return Err(ProfileEditError::BaselineTagsUnsupported);
        }
        if created_at <= baseline.event().created_at_u64() {
            return Err(ProfileEditError::TimestampNotAdvancing);
        }
        for change in [&edit.name, &edit.display_name, &edit.about] {
            if let ProfileFieldChange::Set(value) = change {
                check_content_size(value.len())?;
            }
        }
        if let ProfileFieldChange::Set(name) = &edit.name {
            AuthoredProfile::new(name).map_err(|_| ProfileEditError::InvalidName)?;
        }
        let mut fields: RawFields = serde_json::from_str(baseline.metadata().raw_content())
            .map_err(|_| ProfileEditError::InvalidMetadata)?;
        apply_field(&mut fields, "name", &edit.name)?;
        apply_field(&mut fields, "display_name", &edit.display_name)?;
        apply_field(&mut fields, "about", &edit.about)?;
        apply_media(&mut fields, "picture", &edit.picture)?;
        apply_media(&mut fields, "banner", &edit.banner)?;
        apply_nip05(&mut fields, &edit.nip05)?;
        apply_field(&mut fields, "bot", &edit.bot)?;
        let content =
            serde_json::to_string(&fields).map_err(|_| ProfileEditError::InvalidMetadata)?;
        check_content_size(content.len())?;
        let contract = ContractKey::new(
            RegistryVersion::V8,
            ContractId::parse(PROFILE_METADATA_EDIT_CONTRACT_ID).map_err(|error| {
                ProfileEditError::Plan(AuthoredPlanError::ContractIdentity(error))
            })?,
        )
        .map_err(|error| ProfileEditError::Plan(AuthoredPlanError::ContractIdentity(error)))?;
        let body = AuthoredEventBody::from_profile_edit_parts(
            contract,
            Nip01EventWireParts {
                kind: 0,
                tags: Vec::new(),
                content,
            },
        )
        .map_err(ProfileEditError::Plan)?;
        let plan = Self::bind(body, created_at, author.to_hex()).map_err(ProfileEditError::Plan)?;
        let bytes = PlanWireV1::from_plan(&plan)
            .to_json()
            .map_err(|_| ProfileEditError::InvalidMetadata)?;
        if bytes.len() > PLAN_WIRE_MAX_BYTES {
            return Err(ProfileEditError::DurableWireTooLarge {
                max: PLAN_WIRE_MAX_BYTES,
                actual: bytes.len(),
            });
        }
        Ok(plan)
    }
}

fn check_content_size(actual: usize) -> Result<(), ProfileEditError> {
    if actual > RADROOTS_PROFILE_METADATA_MAX_CONTENT_BYTES {
        return Err(ProfileEditError::ContentTooLarge {
            max: RADROOTS_PROFILE_METADATA_MAX_CONTENT_BYTES,
            actual,
        });
    }
    Ok(())
}

fn set_value<T: Serialize + ?Sized>(
    fields: &mut RawFields,
    name: &str,
    value: &T,
) -> Result<(), ProfileEditError> {
    let raw =
        serde_json::value::to_raw_value(value).map_err(|_| ProfileEditError::InvalidMetadata)?;
    check_content_size(raw.get().len())?;
    fields.insert(name.to_string(), raw);
    Ok(())
}

fn apply_field<T: Serialize>(
    fields: &mut RawFields,
    name: &str,
    change: &ProfileFieldChange<T>,
) -> Result<(), ProfileEditError> {
    match change {
        ProfileFieldChange::Keep => Ok(()),
        ProfileFieldChange::Set(value) => set_value(fields, name, value),
        ProfileFieldChange::Clear => {
            fields.remove(name);
            Ok(())
        }
    }
}

fn apply_nip05(
    fields: &mut RawFields,
    change: &ProfileFieldChange<Nip05Identifier>,
) -> Result<(), ProfileEditError> {
    match change {
        ProfileFieldChange::Keep => Ok(()),
        ProfileFieldChange::Set(value) => set_value(fields, "nip05", value.as_str()),
        ProfileFieldChange::Clear => {
            fields.remove("nip05");
            Ok(())
        }
    }
}

fn apply_media(
    fields: &mut RawFields,
    name: &'static str,
    change: &ProfileMediaChange,
) -> Result<(), ProfileEditError> {
    match change {
        ProfileMediaChange::Clear => {
            fields.remove(name);
            Ok(())
        }
        ProfileMediaChange::Set(image) => {
            set_value(fields, name, image.descriptor().url().as_str())
        }
        ProfileMediaChange::Keep | ProfileMediaChange::KeepVerified(_) => {
            let existing = fields
                .get(name)
                .and_then(|raw| serde_json::from_str::<String>(raw.get()).ok());
            match (existing, change) {
                (Some(value), ProfileMediaChange::KeepVerified(image))
                    if value == image.descriptor().url().as_str() =>
                {
                    Ok(())
                }
                (Some(_), ProfileMediaChange::KeepVerified(_)) => {
                    Err(ProfileEditError::MediaVerificationMismatch { field: name })
                }
                (Some(_), _) => Err(ProfileEditError::MediaVerificationRequired { field: name }),
                (None, ProfileMediaChange::KeepVerified(_)) => {
                    Err(ProfileEditError::MediaVerificationMismatch { field: name })
                }
                (None, _) => Ok(()),
            }
        }
    }
}

pub(crate) fn validate_historical(tags: &[Vec<String>], content: &str) -> Result<(), String> {
    if !tags.is_empty() {
        return Err("profile_tags_forbidden".to_string());
    }
    let profile =
        parse_inbound_profile_metadata(content).map_err(|error| error.code().to_string())?;
    if profile
        .picture()
        .into_iter()
        .chain(profile.banner())
        .any(|media| radroots_blossom::BlobUrl::parse(media.as_str()).is_err())
    {
        return Err("profile_media_url_invalid".to_string());
    }
    let fields: RawFields =
        serde_json::from_str(content).map_err(|_| "profile_json_decode".to_string())?;
    let canonical =
        serde_json::to_string(&fields).map_err(|_| "profile_json_encode".to_string())?;
    if canonical != content {
        return Err("profile_edit_noncanonical".to_string());
    }
    Ok(())
}
