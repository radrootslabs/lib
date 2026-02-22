#![forbid(unsafe_code)]

use crate::error::EventEncodeError;
#[cfg(feature = "serde_json")]
use crate::error::EventParseError;

pub fn is_d_tag_base64url(value: &str) -> bool {
    const D_TAG_LEN: usize = 22;

    if value.len() != D_TAG_LEN {
        return false;
    }
    let bytes = value.as_bytes();
    if !bytes.iter().all(|byte| {
        matches!(
            byte,
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_'
        )
    }) {
        return false;
    }
    matches!(bytes[D_TAG_LEN - 1], b'A' | b'Q' | b'g' | b'w')
}

pub(crate) fn validate_d_tag(value: &str, field: &'static str) -> Result<(), EventEncodeError> {
    if is_d_tag_base64url(value) {
        Ok(())
    } else {
        Err(EventEncodeError::InvalidField(field))
    }
}

#[cfg(feature = "serde_json")]
pub(crate) fn validate_d_tag_tag(value: &str, tag: &'static str) -> Result<(), EventParseError> {
    if is_d_tag_base64url(value) {
        Ok(())
    } else {
        Err(EventParseError::InvalidTag(tag))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn d_tag_base64url_validation_covers_allowed_and_rejected_shapes() {
        assert!(is_d_tag_base64url("AAAAAAAAAAAAAAAAAAAAAA"));
        assert!(!is_d_tag_base64url("AAAAAAAAAAAAAAAAAAAAA!"));
        assert!(!is_d_tag_base64url("AAAAAAAAAAAAAAAAAAAAAB"));
        assert!(!is_d_tag_base64url("short"));
    }

    #[test]
    fn validate_d_tag_returns_error_for_invalid_values() {
        let err = validate_d_tag("AAAAAAAAAAAAAAAAAAAAA!", "d_tag").expect_err("invalid d_tag");
        assert!(matches!(err, EventEncodeError::InvalidField("d_tag")));
    }
}
