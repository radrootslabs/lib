#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

use radroots_event::{
    social::relay_auth::{KIND_RELAY_AUTH, RadrootsRelayAuth},
    tag::name::{TAG_CHALLENGE, TAG_RELAY},
};

use crate::error::EventEncodeError;
use crate::field_helpers::{push_tag, validate_non_empty_field};
use radroots_event::wire::RadrootsNip01EventWireParts;

pub fn relay_auth_build_tags(
    auth: &RadrootsRelayAuth,
) -> Result<Vec<Vec<String>>, EventEncodeError> {
    validate_non_empty_field(&auth.relay, "relay")?;
    validate_non_empty_field(&auth.challenge, "challenge")?;
    let mut tags = Vec::new();
    push_tag(&mut tags, TAG_RELAY, auth.relay.as_str());
    push_tag(&mut tags, TAG_CHALLENGE, auth.challenge.as_str());
    Ok(tags)
}

pub fn to_wire_parts(
    auth: &RadrootsRelayAuth,
) -> Result<RadrootsNip01EventWireParts, EventEncodeError> {
    to_wire_parts_with_kind(auth, KIND_RELAY_AUTH)
}

pub fn to_wire_parts_with_kind(
    auth: &RadrootsRelayAuth,
    kind: u32,
) -> Result<RadrootsNip01EventWireParts, EventEncodeError> {
    if kind != KIND_RELAY_AUTH {
        return Err(EventEncodeError::InvalidKind(kind));
    }
    let tags = relay_auth_build_tags(auth)?;
    Ok(RadrootsNip01EventWireParts {
        kind,
        content: String::new(),
        tags,
    })
}
