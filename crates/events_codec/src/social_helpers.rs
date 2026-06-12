#[cfg(not(feature = "std"))]
use alloc::{format, string::String, vec::Vec};

use radroots_events::social::{
    RadrootsCalendarParticipant, RadrootsSocialFarmAnchor, RadrootsSocialLocation,
    RadrootsSocialMediaDimensions, RadrootsSocialMediaThumbnail,
};

use crate::error::{EventEncodeError, EventParseError};
use crate::field_helpers::{push_tag, push_tag_values, validate_non_empty_field};

pub(crate) fn validate_http_url(value: &str, field: &'static str) -> Result<(), EventEncodeError> {
    if value.starts_with("https://") || value.starts_with("http://") {
        validate_non_empty_field(value, field)
    } else {
        Err(EventEncodeError::InvalidField(field))
    }
}

pub(crate) fn validate_date(value: &str, field: &'static str) -> Result<(), EventEncodeError> {
    if is_date(value) {
        Ok(())
    } else {
        Err(EventEncodeError::InvalidField(field))
    }
}

pub(crate) fn validate_date_tag(value: &str, tag: &'static str) -> Result<(), EventParseError> {
    if is_date(value) {
        Ok(())
    } else {
        Err(EventParseError::InvalidTag(tag))
    }
}

pub(crate) fn is_date(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 10
        && matches!(bytes[0], b'0'..=b'9')
        && matches!(bytes[1], b'0'..=b'9')
        && matches!(bytes[2], b'0'..=b'9')
        && matches!(bytes[3], b'0'..=b'9')
        && bytes[4] == b'-'
        && matches!(bytes[5], b'0'..=b'9')
        && matches!(bytes[6], b'0'..=b'9')
        && bytes[7] == b'-'
        && matches!(bytes[8], b'0'..=b'9')
        && matches!(bytes[9], b'0'..=b'9')
}

pub(crate) fn validate_end_after_start(
    start: u64,
    end: Option<u64>,
    field: &'static str,
) -> Result<(), EventEncodeError> {
    if end.is_some_and(|end| end < start) {
        Err(EventEncodeError::InvalidField(field))
    } else {
        Ok(())
    }
}

pub(crate) fn validate_date_end_after_start(
    start: &str,
    end: Option<&str>,
    field: &'static str,
) -> Result<(), EventEncodeError> {
    if end.is_some_and(|end| end < start) {
        Err(EventEncodeError::InvalidField(field))
    } else {
        Ok(())
    }
}

pub(crate) fn push_location_tags(tags: &mut Vec<Vec<String>>, location: &RadrootsSocialLocation) {
    if let Some(name) = location
        .name
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        push_tag(tags, "location", name);
    }
    if let Some(geohash) = location
        .geohash
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        push_tag(tags, "g", geohash);
    }
}

pub(crate) fn location_from_tags(tags: &[Vec<String>]) -> Option<RadrootsSocialLocation> {
    let name = first_tag_value(tags, "location");
    let geohash = first_tag_value(tags, "g");
    if name.is_none() && geohash.is_none() {
        None
    } else {
        Some(RadrootsSocialLocation { name, geohash })
    }
}

pub(crate) fn push_farm_anchor(tags: &mut Vec<Vec<String>>, farm: &RadrootsSocialFarmAnchor) {
    if farm.farm.pubkey.trim().is_empty() || farm.farm.d_tag.trim().is_empty() {
        return;
    }
    let address = format!("30340:{}:{}", farm.farm.pubkey, farm.farm.d_tag);
    push_tag(tags, "a", address);
}

pub(crate) fn participants_from_tags(
    tags: &[Vec<String>],
) -> Option<Vec<RadrootsCalendarParticipant>> {
    let participants = tags
        .iter()
        .filter(|tag| tag.first().map(|value| value.as_str()) == Some("p"))
        .filter_map(|tag| {
            let pubkey = tag.get(1)?.clone();
            if pubkey.trim().is_empty() {
                return None;
            }
            Some(RadrootsCalendarParticipant {
                pubkey,
                relay: tag.get(2).filter(|value| !value.trim().is_empty()).cloned(),
                role: tag.get(3).filter(|value| !value.trim().is_empty()).cloned(),
            })
        })
        .collect::<Vec<_>>();
    if participants.is_empty() {
        None
    } else {
        Some(participants)
    }
}

pub(crate) fn push_participants(
    tags: &mut Vec<Vec<String>>,
    participants: Option<&Vec<RadrootsCalendarParticipant>>,
) {
    let Some(participants) = participants else {
        return;
    };
    for participant in participants {
        if participant.pubkey.trim().is_empty() {
            continue;
        }
        let mut tag = vec!["p".to_string(), participant.pubkey.clone()];
        if let Some(relay) = participant.relay.as_ref() {
            tag.push(relay.clone());
        }
        if let Some(role) = participant.role.as_ref() {
            if participant.relay.is_none() {
                tag.push(String::new());
            }
            tag.push(role.clone());
        }
        tags.push(tag);
    }
}

pub(crate) fn first_tag_value(tags: &[Vec<String>], key: &str) -> Option<String> {
    tags.iter()
        .find(|tag| tag.first().map(|value| value.as_str()) == Some(key))
        .and_then(|tag| tag.get(1))
        .filter(|value| !value.trim().is_empty())
        .cloned()
}

pub(crate) fn dimensions_tag(dimensions: &RadrootsSocialMediaDimensions) -> String {
    format!("{}x{}", dimensions.width, dimensions.height)
}

pub(crate) fn parse_dimensions_tag(
    value: &str,
    tag: &'static str,
) -> Result<RadrootsSocialMediaDimensions, EventParseError> {
    let Some((width, height)) = value.split_once('x') else {
        return Err(EventParseError::InvalidTag(tag));
    };
    let width = width
        .parse::<u32>()
        .map_err(|err| EventParseError::InvalidNumber(tag, err))?;
    let height = height
        .parse::<u32>()
        .map_err(|err| EventParseError::InvalidNumber(tag, err))?;
    if width == 0 || height == 0 {
        return Err(EventParseError::InvalidTag(tag));
    }
    Ok(RadrootsSocialMediaDimensions { width, height })
}

pub(crate) fn push_thumbnail(
    tags: &mut Vec<Vec<String>>,
    thumbnail: &RadrootsSocialMediaThumbnail,
) {
    if thumbnail.url.trim().is_empty() {
        return;
    }
    if let Some(dimensions) = thumbnail.dimensions.as_ref() {
        push_tag_values(
            tags,
            "thumb",
            [thumbnail.url.clone(), dimensions_tag(dimensions)],
        );
    } else {
        push_tag(tags, "thumb", thumbnail.url.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_dates_and_ordered_time_ranges() {
        assert!(is_date("2026-06-20"));
        assert!(!is_date("2026-6-20"));
        assert!(validate_end_after_start(10, Some(10), "end").is_ok());
        assert!(matches!(
            validate_end_after_start(10, Some(9), "end"),
            Err(EventEncodeError::InvalidField("end"))
        ));
        assert!(matches!(
            validate_date_tag("bad", "start"),
            Err(EventParseError::InvalidTag("start"))
        ));
    }

    #[test]
    fn encodes_and_decodes_location_participant_and_dimensions_tags() {
        let mut tags = Vec::new();
        push_location_tags(
            &mut tags,
            &RadrootsSocialLocation {
                name: Some("Pack shed".to_string()),
                geohash: Some("c23nb62w20st".to_string()),
            },
        );
        push_participants(
            &mut tags,
            Some(&vec![RadrootsCalendarParticipant {
                pubkey: "crew_pubkey".to_string(),
                relay: None,
                role: Some("participant".to_string()),
            }]),
        );

        let location = location_from_tags(&tags).expect("location");
        assert_eq!(location.name.as_deref(), Some("Pack shed"));
        assert_eq!(location.geohash.as_deref(), Some("c23nb62w20st"));
        let participants = participants_from_tags(&tags).expect("participants");
        assert_eq!(participants[0].pubkey, "crew_pubkey");
        assert_eq!(participants[0].role.as_deref(), Some("participant"));

        let dimensions = parse_dimensions_tag("1200x800", "dim").unwrap();
        assert_eq!(dimensions_tag(&dimensions), "1200x800");
        assert!(matches!(
            parse_dimensions_tag("0x800", "dim"),
            Err(EventParseError::InvalidTag("dim"))
        ));
    }
}
