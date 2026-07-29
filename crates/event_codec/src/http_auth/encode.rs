#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

use radroots_event::{
    social::http_auth::{HttpAuth, KIND_HTTP_AUTH},
    tag::name::{TAG_METHOD, TAG_PAYLOAD, TAG_URL_AUTH},
};

use crate::error::EventEncodeError;
use crate::field_helpers::{
    push_optional_tag, push_tag, validate_lowercase_hex_64, validate_non_empty_field,
};
use radroots_event::wire::Nip01EventWireParts;

pub fn http_auth_build_tags(auth: &HttpAuth) -> Result<Vec<Vec<String>>, EventEncodeError> {
    validate_non_empty_field(&auth.url, "url")?;
    validate_non_empty_field(&auth.method, "method")?;
    if let Some(payload) = auth.payload_sha256.as_deref() {
        validate_lowercase_hex_64(payload, "payload_sha256")?;
    }
    let mut tags = Vec::new();
    push_tag(&mut tags, TAG_URL_AUTH, auth.url.as_str());
    push_tag(&mut tags, TAG_METHOD, auth.method.as_str());
    push_optional_tag(&mut tags, TAG_PAYLOAD, auth.payload_sha256.as_deref());
    Ok(tags)
}

pub fn to_wire_parts(auth: &HttpAuth) -> Result<Nip01EventWireParts, EventEncodeError> {
    to_wire_parts_with_kind(auth, KIND_HTTP_AUTH)
}

pub fn to_wire_parts_with_kind(
    auth: &HttpAuth,
    kind: u32,
) -> Result<Nip01EventWireParts, EventEncodeError> {
    if kind != KIND_HTTP_AUTH {
        return Err(EventEncodeError::InvalidKind(kind));
    }
    let tags = http_auth_build_tags(auth)?;
    Ok(Nip01EventWireParts {
        kind,
        content: String::new(),
        tags,
    })
}
