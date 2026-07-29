#[cfg(not(feature = "std"))]
use alloc::vec::Vec;
use core::fmt;

use radroots_event::{
    envelope::kind::KIND_PROFILE,
    profile::{AuthoredProfile, RADROOTS_PROFILE_METADATA_MAX_CONTENT_BYTES},
    wire::Nip01EventWireParts,
};
use serde::Serialize;

#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsAuthoredProfileEncodeError {
    ContentTooLarge { max: usize, actual: usize },
}

impl RadrootsAuthoredProfileEncodeError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::ContentTooLarge { .. } => "content_too_large",
        }
    }
}

impl fmt::Display for RadrootsAuthoredProfileEncodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ContentTooLarge { max, actual } => {
                write!(
                    f,
                    "authored Profile metadata is {actual} bytes; max is {max}"
                )
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for RadrootsAuthoredProfileEncodeError {}

#[derive(Serialize)]
struct AuthoredProfileMetadata<'a> {
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

/// Builds deterministic unsigned kind-0 wire parts from strict authored metadata.
///
/// The output is a complete replacement snapshot, not a merge or patch. The
/// caller must not use it for an existing Profile edit unless omitted fields
/// are intentionally removed. Retaining existing media requires the caller to
/// re-establish its byte-verified descriptor state.
///
/// The caller must separately prove successful BUD-02 upload completion before
/// passing media-bearing output to a signing boundary.
pub fn authored_profile_to_wire_parts(
    profile: &AuthoredProfile,
) -> Result<Nip01EventWireParts, RadrootsAuthoredProfileEncodeError> {
    let metadata = AuthoredProfileMetadata {
        name: profile.name(),
        display_name: profile.display_name(),
        about: profile.about(),
        picture: profile
            .picture()
            .map(|image| image.descriptor().url().as_str()),
        banner: profile
            .banner()
            .map(|image| image.descriptor().url().as_str()),
        nip05: profile.nip05().map(|identifier| identifier.as_str()),
        bot: profile.bot(),
    };
    let expected_len = authored_profile_metadata_len(&metadata);
    if expected_len > RADROOTS_PROFILE_METADATA_MAX_CONTENT_BYTES {
        return Err(RadrootsAuthoredProfileEncodeError::ContentTooLarge {
            max: RADROOTS_PROFILE_METADATA_MAX_CONTENT_BYTES,
            actual: expected_len,
        });
    }
    let content = serde_json::to_string(&metadata)
        .expect("authored Profile metadata contains only infallible JSON scalar types");
    Ok(Nip01EventWireParts {
        kind: KIND_PROFILE,
        content,
        tags: Vec::new(),
    })
}

fn authored_profile_metadata_len(metadata: &AuthoredProfileMetadata<'_>) -> usize {
    let mut len = 2usize;
    let mut fields = 0usize;
    add_string_field_len(&mut len, &mut fields, "name", metadata.name);
    if let Some(value) = metadata.display_name {
        add_string_field_len(&mut len, &mut fields, "display_name", value);
    }
    if let Some(value) = metadata.about {
        add_string_field_len(&mut len, &mut fields, "about", value);
    }
    if let Some(value) = metadata.picture {
        add_string_field_len(&mut len, &mut fields, "picture", value);
    }
    if let Some(value) = metadata.banner {
        add_string_field_len(&mut len, &mut fields, "banner", value);
    }
    if let Some(value) = metadata.nip05 {
        add_string_field_len(&mut len, &mut fields, "nip05", value);
    }
    if let Some(value) = metadata.bot {
        add_field_prefix_len(&mut len, &mut fields, "bot");
        len = len.saturating_add(if value { 4 } else { 5 });
    }
    len
}

fn add_string_field_len(len: &mut usize, fields: &mut usize, key: &str, value: &str) {
    add_field_prefix_len(len, fields, key);
    *len = len.saturating_add(json_string_encoded_len(value));
}

fn add_field_prefix_len(len: &mut usize, fields: &mut usize, key: &str) {
    if *fields > 0 {
        *len = len.saturating_add(1);
    }
    *fields = fields.saturating_add(1);
    *len = len.saturating_add(key.len().saturating_add(3));
}

fn json_string_encoded_len(value: &str) -> usize {
    value.bytes().fold(2usize, |len, byte| {
        let encoded = match byte {
            b'"' | b'\\' | b'\x08' | b'\t' | b'\n' | b'\x0c' | b'\r' => 2,
            0x00..=0x1f => 6,
            _ => 1,
        };
        len.saturating_add(encoded)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use radroots_event::profile::Nip05Identifier;

    #[test]
    fn preflight_length_matches_compact_json_escaping() {
        let escaped = "\0\u{8}\t\n\u{c}\r\"\\e\u{301}";
        assert_eq!(
            json_string_encoded_len(escaped),
            serde_json::to_string(escaped).unwrap().len()
        );

        let profile = AuthoredProfile::new("farm \\\"one\\\"")
            .unwrap()
            .with_display_name(escaped)
            .with_about("Victoria \u{e9}")
            .with_nip05(Nip05Identifier::parse("farm@example.com").unwrap())
            .with_bot(true);
        let wire = authored_profile_to_wire_parts(&profile).unwrap();
        let metadata = AuthoredProfileMetadata {
            name: profile.name(),
            display_name: profile.display_name(),
            about: profile.about(),
            picture: None,
            banner: None,
            nip05: profile.nip05().map(|identifier| identifier.as_str()),
            bot: profile.bot(),
        };
        assert_eq!(authored_profile_metadata_len(&metadata), wire.content.len());
    }
}
