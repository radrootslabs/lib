#![forbid(unsafe_code)]

use crate::error::{EventEncodeError, EventParseError};

pub fn is_d_tag_base64url(value: &str) -> bool {
    if value.is_empty() {
        return false;
    }
    value.as_bytes().iter().all(|byte| {
        matches!(
            byte,
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_'
        )
    })
}

pub(crate) fn validate_d_tag(value: &str, field: &'static str) -> Result<(), EventEncodeError> {
    if is_d_tag_base64url(value) {
        Ok(())
    } else {
        Err(EventEncodeError::InvalidField(field))
    }
}

pub(crate) fn validate_d_tag_tag(value: &str, tag: &'static str) -> Result<(), EventParseError> {
    if is_d_tag_base64url(value) {
        Ok(())
    } else {
        Err(EventParseError::InvalidTag(tag))
    }
}
