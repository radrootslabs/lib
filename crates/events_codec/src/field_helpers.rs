#[cfg(not(feature = "std"))]
use alloc::{
    format,
    string::{String, ToString},
    vec,
    vec::Vec,
};

use crate::d_tag::validate_d_tag;
use crate::d_tag::validate_d_tag_tag;
use crate::error::EventEncodeError;
use crate::error::EventParseError;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RadrootsAddress {
    pub kind: u32,
    pub pubkey: String,
    pub d_tag: String,
}

pub(crate) fn address_string(
    kind: u32,
    pubkey: &str,
    d_tag: &str,
    field: &'static str,
) -> Result<String, EventEncodeError> {
    validate_non_empty_field(pubkey, field)?;
    validate_d_tag(d_tag, field)?;
    Ok(format!("{kind}:{pubkey}:{d_tag}"))
}

pub(crate) fn parse_address_tag(
    value: &str,
    tag: &'static str,
) -> Result<RadrootsAddress, EventParseError> {
    let mut parts = value.split(':');
    let kind = parts
        .next()
        .ok_or(EventParseError::InvalidTag(tag))?
        .parse::<u32>()
        .map_err(|err| EventParseError::InvalidNumber(tag, err))?;
    let pubkey = parts
        .next()
        .map(ToString::to_string)
        .ok_or(EventParseError::InvalidTag(tag))?;
    let d_tag = parts
        .next()
        .map(ToString::to_string)
        .ok_or(EventParseError::InvalidTag(tag))?;
    if parts.next().is_some() {
        return Err(EventParseError::InvalidTag(tag));
    }
    validate_non_empty_tag_value(&pubkey, tag)?;
    validate_d_tag_tag(&d_tag, tag)?;
    Ok(RadrootsAddress {
        kind,
        pubkey,
        d_tag,
    })
}

pub(crate) fn parse_address_tag_with_kind(
    value: &str,
    expected_kind: u32,
    tag: &'static str,
) -> Result<RadrootsAddress, EventParseError> {
    let address = parse_address_tag(value, tag)?;
    if address.kind != expected_kind {
        return Err(EventParseError::InvalidTag(tag));
    }
    Ok(address)
}

pub(crate) fn is_lowercase_hex_64(value: &str) -> bool {
    value.len() == 64
        && value
            .as_bytes()
            .iter()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
}

pub(crate) fn validate_lowercase_hex_64(
    value: &str,
    field: &'static str,
) -> Result<(), EventEncodeError> {
    if is_lowercase_hex_64(value) {
        Ok(())
    } else {
        Err(EventEncodeError::InvalidField(field))
    }
}

pub(crate) fn validate_lowercase_hex_64_tag(
    value: &str,
    tag: &'static str,
) -> Result<(), EventParseError> {
    if is_lowercase_hex_64(value) {
        Ok(())
    } else {
        Err(EventParseError::InvalidTag(tag))
    }
}

pub(crate) fn is_non_empty_base64url(value: &str) -> bool {
    !value.is_empty()
        && value.as_bytes().iter().all(|byte| {
            matches!(
                byte,
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_'
            )
        })
}

pub(crate) fn validate_non_empty_base64url(
    value: &str,
    field: &'static str,
) -> Result<(), EventEncodeError> {
    if is_non_empty_base64url(value) {
        Ok(())
    } else {
        Err(EventEncodeError::InvalidField(field))
    }
}

pub(crate) fn validate_non_empty_field(
    value: &str,
    field: &'static str,
) -> Result<(), EventEncodeError> {
    if value.trim().is_empty() {
        Err(EventEncodeError::EmptyRequiredField(field))
    } else {
        Ok(())
    }
}

pub(crate) fn validate_non_empty_tag_value(
    value: &str,
    tag: &'static str,
) -> Result<(), EventParseError> {
    if value.trim().is_empty() {
        Err(EventParseError::InvalidTag(tag))
    } else {
        Ok(())
    }
}

pub(crate) fn push_tag(tags: &mut Vec<Vec<String>>, key: &str, value: impl Into<String>) {
    tags.push(vec![key.to_string(), value.into()]);
}

pub(crate) fn push_optional_tag(tags: &mut Vec<Vec<String>>, key: &str, value: Option<&str>) {
    if let Some(value) = value {
        if !value.trim().is_empty() {
            push_tag(tags, key, value);
        }
    }
}

pub(crate) fn push_tag_values<I, S>(tags: &mut Vec<Vec<String>>, key: &str, values: I)
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let mut tag = vec![key.to_string()];
    tag.extend(values.into_iter().map(Into::into));
    tags.push(tag);
}

pub(crate) fn required_tag_value(
    tags: &[Vec<String>],
    key: &'static str,
) -> Result<String, EventParseError> {
    tags.iter()
        .find(|tag| tag.first().map(|value| value.as_str()) == Some(key))
        .ok_or(EventParseError::MissingTag(key))
        .and_then(|tag| {
            tag.get(1)
                .map(ToString::to_string)
                .ok_or(EventParseError::InvalidTag(key))
        })
        .and_then(|value| {
            validate_non_empty_tag_value(&value, key)?;
            Ok(value)
        })
}

pub(crate) fn optional_tag_value(
    tags: &[Vec<String>],
    key: &'static str,
) -> Result<Option<String>, EventParseError> {
    let Some(tag) = tags
        .iter()
        .find(|tag| tag.first().map(|value| value.as_str()) == Some(key))
    else {
        return Ok(None);
    };
    let value = tag
        .get(1)
        .map(ToString::to_string)
        .ok_or(EventParseError::InvalidTag(key))?;
    validate_non_empty_tag_value(&value, key)?;
    Ok(Some(value))
}

pub(crate) fn tag_values(
    tags: &[Vec<String>],
    key: &'static str,
) -> Result<Vec<String>, EventParseError> {
    tags.iter()
        .filter(|tag| tag.first().map(|value| value.as_str()) == Some(key))
        .map(|tag| {
            tag.get(1)
                .map(ToString::to_string)
                .ok_or(EventParseError::InvalidTag(key))
                .and_then(|value| {
                    validate_non_empty_tag_value(&value, key)?;
                    Ok(value)
                })
        })
        .collect()
}

pub(crate) fn require_empty_content(
    content: &str,
    field: &'static str,
) -> Result<(), EventParseError> {
    if content.is_empty() {
        Ok(())
    } else {
        Err(EventParseError::InvalidJson(field))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_D_TAG: &str = "AAAAAAAAAAAAAAAAAAAAAA";
    const VALID_HASH: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    #[test]
    fn address_string_formats_valid_radroots_address() {
        let address = address_string(30078, "workspace_pubkey", VALID_D_TAG, "workspace")
            .expect("valid address");

        assert_eq!(address, "30078:workspace_pubkey:AAAAAAAAAAAAAAAAAAAAAA");
    }

    #[test]
    fn address_string_rejects_empty_pubkey_and_bad_d_tag() {
        assert!(matches!(
            address_string(30078, "", VALID_D_TAG, "workspace"),
            Err(EventEncodeError::EmptyRequiredField("workspace"))
        ));
        assert!(matches!(
            address_string(30078, "workspace_pubkey", "bad", "workspace"),
            Err(EventEncodeError::InvalidField("workspace"))
        ));
    }

    #[test]
    fn address_parser_accepts_valid_radroots_address() {
        let address = parse_address_tag("30078:workspace_pubkey:AAAAAAAAAAAAAAAAAAAAAA", "a")
            .expect("valid address");

        assert_eq!(address.kind, 30078);
        assert_eq!(address.pubkey, "workspace_pubkey");
        assert_eq!(address.d_tag, VALID_D_TAG);
    }

    #[test]
    fn address_parser_rejects_invalid_radroots_addresses() {
        assert!(matches!(
            parse_address_tag("30078:workspace_pubkey", "a"),
            Err(EventParseError::InvalidTag("a"))
        ));
        assert!(matches!(
            parse_address_tag("30078::AAAAAAAAAAAAAAAAAAAAAA", "a"),
            Err(EventParseError::InvalidTag("a"))
        ));
        assert!(matches!(
            parse_address_tag("bad:workspace_pubkey:AAAAAAAAAAAAAAAAAAAAAA", "a"),
            Err(EventParseError::InvalidNumber("a", _))
        ));
        assert!(matches!(
            parse_address_tag("30078:workspace_pubkey:bad", "a"),
            Err(EventParseError::InvalidTag("a"))
        ));
        assert!(matches!(
            parse_address_tag_with_kind("78:workspace_pubkey:AAAAAAAAAAAAAAAAAAAAAA", 30078, "a"),
            Err(EventParseError::InvalidTag("a"))
        ));
    }

    #[test]
    fn lowercase_hex_hash_validation_accepts_only_sha256_shape() {
        assert!(is_lowercase_hex_64(VALID_HASH));
        assert!(!is_lowercase_hex_64(
            "0123456789ABCDEF0123456789abcdef0123456789abcdef0123456789abcdef"
        ));
        assert!(!is_lowercase_hex_64(
            "0123456789xyzdef0123456789abcdef0123456789abcdef0123456789abcdef"
        ));
        assert!(!is_lowercase_hex_64("0123456789abcdef"));
        assert!(matches!(
            validate_lowercase_hex_64("0123456789abcdef", "payload"),
            Err(EventEncodeError::InvalidField("payload"))
        ));
    }

    #[test]
    fn lowercase_hex_tag_validation_maps_to_parse_error() {
        assert!(validate_lowercase_hex_64_tag(VALID_HASH, "x").is_ok());
        assert!(matches!(
            validate_lowercase_hex_64_tag("0123456789abcdef", "x"),
            Err(EventParseError::InvalidTag("x"))
        ));
    }

    #[test]
    fn base64url_validation_accepts_non_empty_unpadded_payloads() {
        assert!(is_non_empty_base64url("abc-DEF_012"));
        assert!(!is_non_empty_base64url(""));
        assert!(!is_non_empty_base64url("abc="));
        assert!(!is_non_empty_base64url("abc/def"));
        assert!(matches!(
            validate_non_empty_base64url("abc=", "encoded_change"),
            Err(EventEncodeError::InvalidField("encoded_change"))
        ));
    }

    #[test]
    fn tag_helpers_parse_required_optional_and_repeated_values() {
        let tags = vec![
            vec!["h".to_string(), "group".to_string()],
            vec!["t".to_string(), "radroots:farm:crdt".to_string()],
            vec!["t".to_string(), "task".to_string()],
        ];

        assert_eq!(required_tag_value(&tags, "h").unwrap(), "group");
        assert_eq!(optional_tag_value(&tags, "missing").unwrap(), None);
        assert_eq!(
            tag_values(&tags, "t").unwrap(),
            vec!["radroots:farm:crdt".to_string(), "task".to_string()]
        );
    }

    #[test]
    fn tag_helpers_build_simple_and_repeated_tags() {
        let mut tags = Vec::new();

        push_tag(&mut tags, "h", "group");
        push_optional_tag(&mut tags, "p", Some("pubkey"));
        push_optional_tag(&mut tags, "p", Some(""));
        push_tag_values(&mut tags, "roles", ["member", "admin"]);

        assert_eq!(
            tags,
            vec![
                vec!["h".to_string(), "group".to_string()],
                vec!["p".to_string(), "pubkey".to_string()],
                vec![
                    "roles".to_string(),
                    "member".to_string(),
                    "admin".to_string()
                ],
            ]
        );
    }
}
